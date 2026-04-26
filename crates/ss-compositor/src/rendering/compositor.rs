//! Main rendering pipeline.
//!
//! Composites clips onto a canvas with transforms, alpha blending, and z-ordering.
//! Uses `imageproc` for affine transforms via [`Projection`].
//!
//! # Performance
//!
//! Warp and composite operations are scoped to the axis-aligned bounding box (AABB)
//! of each transformed clip, avoiding full-canvas allocations and iterations for
//! small or off-screen clips.

use std::path::Path;
use std::sync::Arc;

use error_stack::{Report, ResultExt};
use image::{Rgba, RgbaImage};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use rayon::prelude::*;
use tracing::{debug, instrument};

use ss_core::interpolation::resolve_items;
use ss_core::item::ItemContent;
use ss_core::path_resolve::resolve_path;
use ss_core::project::Project;
use ss_core::transform::ResolvedItem;

use crate::image::ImageProvider;
use crate::rendering::CompositorError;
use crate::rendering::FrameRenderer;
use crate::rendering::buffer_pool::{self, BufferPool, PooledBuffer};
use crate::sizing::{PlacedRect, compute_placement};
use crate::viewport::Viewport;

/// Pre-computed plan for warping a clip, produced during the plan phase.
///
/// Contains all information needed to decide visibility (AABB, opacity, transform class)
/// and to perform the warp (source, forward matrix, placement). Produced without
/// allocating a warp output buffer.
#[allow(dead_code)]
struct ClipPlan {
    /// The resolved item (animations interpolated, z-path computed).
    resolved: ResolvedItem,
    /// The source image (from cache, shared via Arc).
    source: Arc<RgbaImage>,
    /// Base placement rectangle (sized to viewport-space).
    placement: PlacedRect,
    /// Forward transform matrix: source pixel → canvas pixel.
    forward: Affine3x3,
    /// Axis-aligned bounding box clamped to canvas: `(x, y, w, h)`.
    aabb: (u32, u32, u32, u32),
    /// Effective opacity (composed through ancestor chain).
    opacity: f32,
    /// Whether the transform is scale+translate or general affine.
    transform_class: TransformClass,
    /// Viewport-to-project scale factors for translate animation.
    viewport_scale: (f32, f32),
}

/// The main compositor renderer.
///
/// Uses an [`ImageProvider`] to load images and renders frames by compositing
/// clips with transforms, alpha blending, and z-ordering.
pub struct CompositorRenderer {
    image_provider: Arc<dyn ImageProvider>,
    buffer_pool: BufferPool,
}

impl CompositorRenderer {
    /// Create a new renderer with the given image provider and a default buffer pool.
    pub fn new(image_provider: Arc<dyn ImageProvider>) -> Self {
        Self {
            image_provider,
            buffer_pool: BufferPool::new(8),
        }
    }
}

impl FrameRenderer for CompositorRenderer {
    fn name(&self) -> &'static str {
        "compositor"
    }

    #[instrument(name = "render", skip_all, fields(time = %format!("{time:.3}")))]
    fn render(
        &self,
        project: &Project,
        project_file: &Path,
        time: f64,
        viewport: &Viewport,
    ) -> Result<RgbaImage, Report<CompositorError>> {
        let (out_w, out_h) = viewport.output_size;

        // 1. Create output frame filled with transparent black.
        let mut canvas = RgbaImage::new(out_w, out_h);

        // 2. Fill the canvas_rect area with the project background color.
        fill_background(&mut canvas, viewport, &project.background);

        // 3. Resolve active items (filtering, sorting, group flattening).
        let resolved = resolve_items(&project.items, time).change_context(CompositorError)?;

        // Phase 1: Plan (parallel) — compute AABBs and transform classes.
        let plan_results: Vec<Option<ClipPlan>> = resolved
            .par_iter()
            .map(|resolved_item| {
                plan_clip(
                    &self.image_provider,
                    resolved_item,
                    project_file,
                    viewport,
                    &project.resolution,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let plans: Vec<ClipPlan> = plan_results.into_iter().flatten().collect();

        // Phase 2: Cull (sequential) — skip occluded clips.
        let visible = occlusion_cull(&plans);
        let _visible_count = visible.iter().filter(|&&v| v).count();

        // Phase 3: Warp visible clips only (parallel).
        let warped_clips: Vec<WarpedClip> = plans
            .into_par_iter()
            .zip(visible)
            .filter(|(_, v)| *v)
            .map(|(plan, _)| warp_from_plan(&self.buffer_pool, plan))
            .collect();

        // Phase 4: Composite visible clips only (parallel bands).
        composite_clips(&mut canvas, &warped_clips);

        // Phase 5: Return buffers to pool.
        return_buffers(&self.buffer_pool, warped_clips);

        Ok(canvas)
    }
}

/// Fill the canvas background within the viewport's canvas_rect.
fn fill_background(canvas: &mut RgbaImage, viewport: &Viewport, bg: &[u8; 4]) {
    let (cx, cy, cw, ch) = viewport.canvas_rect;
    let bg_color = Rgba(*bg);
    for y in cy..(cy + ch) {
        for x in cx..(cx + cw) {
            if x < canvas.width() && y < canvas.height() {
                canvas.put_pixel(x, y, bg_color);
            }
        }
    }
}

/// Plan a clip's warp by computing placement, transform, and AABB without allocating
/// a warp output buffer.
///
/// Returns `None` if the clip is fully transparent (`opacity <= 0`) or entirely
/// off-screen (`aabb` is zero).
///
/// # Errors
///
/// Returns an error if the image cannot be loaded.
#[instrument(name = "plan_clip", skip_all, fields(item_id = %resolved.item.id))]
fn plan_clip(
    image_provider: &Arc<dyn ImageProvider>,
    resolved: &ResolvedItem,
    project_file: &Path,
    viewport: &Viewport,
    project_resolution: &[u32; 2],
) -> Result<Option<ClipPlan>, Report<CompositorError>> {
    // Resolve the image path.
    let image_path = match &resolved.item.content {
        ItemContent::Image { path } => resolve_path(project_file, path)
            .change_context(CompositorError)
            .attach(resolved.item.id.clone())?,
        // Groups are flattened by `resolve_items`; they never appear here.
        ItemContent::Group { .. } => unreachable!("groups should be flattened by resolve_items"),
    };

    // Skip clips that are fully transparent — warp + composite would produce no visible output.
    if resolved.opacity <= 0.0 {
        return Ok(None);
    }

    // Load the source image.
    let source = image_provider
        .get(&image_path)
        .change_context(CompositorError)
        .attach(resolved.item.id.clone())?;

    // Compute base placement from sizing (in project-space).
    let placement = compute_placement(
        &resolved.item.sizing,
        (source.width(), source.height()),
        (project_resolution[0], project_resolution[1]),
    );

    // Scale placement from project-space to viewport-space.
    let viewport_scale = (
        viewport.output_size.0 as f32 / project_resolution[0] as f32,
        viewport.output_size.1 as f32 / project_resolution[1] as f32,
    );
    let placement = placement.scale_to_viewport(
        (project_resolution[0], project_resolution[1]),
        viewport.output_size,
    );

    let (canvas_w, canvas_h) = viewport.output_size;

    let scale_x = if source.width() > 0 {
        placement.w / source.width() as f32
    } else {
        1.0
    };
    let scale_y = if source.height() > 0 {
        placement.h / source.height() as f32
    } else {
        1.0
    };

    // Pivot in canvas space (before animation transforms).
    let pivot_x = placement.x + placement.w * resolved.item.pivot[0];
    let pivot_y = placement.y + placement.h * resolved.item.pivot[1];

    // Build the forward transform matrix: source → destination.
    let forward = Affine3x3::scale(scale_x, scale_y)
        .then(Affine3x3::translate(placement.x, placement.y))
        .then(Affine3x3::translate(-pivot_x, -pivot_y))
        .then(Affine3x3::rotate(resolved.rotation))
        .then(Affine3x3::scale(resolved.scale.x, resolved.scale.y))
        .then(Affine3x3::translate(pivot_x, pivot_y))
        .then(Affine3x3::translate(
            resolved.translate.x * viewport_scale.0,
            resolved.translate.y * viewport_scale.1,
        ))
        .then(Affine3x3::translate(
            viewport.camera_pan.0,
            viewport.camera_pan.1,
        ));

    // Compute the bounding box of the transformed clip, clamped to canvas bounds.
    let aabb = transformed_aabb(
        &forward.0,
        source.width(),
        source.height(),
        canvas_w,
        canvas_h,
    );

    // Off-screen or degenerate clip — no work to do.
    if aabb.2 == 0 || aabb.3 == 0 {
        return Ok(None);
    }

    let transform_class = classify_transform(&forward.0);

    Ok(Some(ClipPlan {
        resolved: resolved.clone(),
        source,
        placement,
        forward,
        aabb,
        opacity: resolved.opacity,
        transform_class,
        viewport_scale,
    }))
}

/// Warp a clip from its pre-computed plan into a pooled buffer.
///
/// Acquires a buffer from the pool, warps into it, and returns a [`WarpedClip`]
/// carrying the buffer and its pool receipt.
#[instrument(name = "warp_from_plan", skip_all)]
fn warp_from_plan(pool: &BufferPool, plan: ClipPlan) -> WarpedClip {
    let ClipPlan {
        resolved,
        source,
        forward,
        aabb: (aabb_x, aabb_y, aabb_w, aabb_h),
        transform_class,
        ..
    } = plan;

    debug!(
        aabb = %format!("{aabb_w}x{aabb_h}+{aabb_x}+{aabb_y}"),
        source = %format!("{}x{}", source.width(), source.height()),
        fast_path = matches!(transform_class, TransformClass::ScaleTranslate),
        "warping clip"
    );

    // Acquire a pooled buffer for the warp output.
    let byte_count = aabb_w as usize * aabb_h as usize * 4;
    let pooled = pool.acquire(byte_count);
    let (raw, receipt) = pooled.take();
    let mut output = RgbaImage::from_raw(aabb_w, aabb_h, raw)
        .expect("acquire returns exact-size buffer; dimensions match");

    // Dispatch based on transform class.
    match transform_class {
        TransformClass::ScaleTranslate => {
            warp_resize_into(
                &mut output,
                &source,
                &forward.0,
                (aabb_x, aabb_y, aabb_w, aabb_h),
            );
        }
        TransformClass::General => {
            // Existing imageproc::warp_into path for rotated/skewed clips.
            let adjusted = Affine3x3::translate(-(aabb_x as f32), -(aabb_y as f32)).then(forward);
            let projection = Projection::from_matrix(adjusted.0)
                .expect("transform matrix composed of scale/translate/rotate is invertible");
            warp_into(
                &source,
                &projection,
                Interpolation::Bilinear,
                Rgba([0u8, 0u8, 0u8, 0u8]),
                &mut output,
            );
        }
    }

    // Apply opacity to alpha channel.
    if resolved.opacity < 1.0 {
        for pixel in output.pixels_mut() {
            pixel.0[3] = (pixel.0[3] as f32 * resolved.opacity) as u8;
        }
    }

    WarpedClip {
        buffer: output,
        offset: (aabb_x, aabb_y),
        receipt: Some(receipt),
    }
}

/// Determine which clips are visible after occlusion by opaque clips above them.
///
/// Walks plans in reverse z-order (topmost first). An opaque `ScaleTranslate` clip
/// with a non-zero AABB becomes an occluder. A clip is culled (marked invisible) if
/// its AABB is fully contained in any single occluder.
///
/// This is conservative: only `ScaleTranslate` clips are occluders (rotated AABBs
/// have transparent corners), and only single-occluder containment is checked
/// (union coverage is not computed). No visible clip is ever incorrectly culled.
fn occlusion_cull(plans: &[ClipPlan]) -> Vec<bool> {
    let mut visible = vec![true; plans.len()];
    let mut occluders: Vec<(u32, u32, u32, u32)> = Vec::new();

    for i in (0..plans.len()).rev() {
        let plan = &plans[i];

        // Defensive: zero AABB should have been filtered at plan time.
        if plan.aabb.2 == 0 || plan.aabb.3 == 0 {
            visible[i] = false;
            continue;
        }

        // Check containment in any existing occluder.
        if is_fully_contained(plan.aabb, &occluders) {
            visible[i] = false;
            continue;
        }

        // Add as occluder if: fully opaque AND scale+translate.
        if plan.opacity >= 1.0 && matches!(plan.transform_class, TransformClass::ScaleTranslate) {
            occluders.push(plan.aabb);
        }
    }

    visible
}

/// Check if AABB `(ax, ay, aw, ah)` is fully contained within any single occluder.
///
/// Containment means: `ox <= ax AND oy <= ay AND ax + aw <= ox + ow AND ay + ah <= oy + oh`.
fn is_fully_contained(
    (ax, ay, aw, ah): (u32, u32, u32, u32),
    occluders: &[(u32, u32, u32, u32)],
) -> bool {
    occluders
        .iter()
        .any(|&(ox, oy, ow, oh)| ox <= ax && oy <= ay && ax + aw <= ox + ow && ay + ah <= oy + oh)
}

/// A row-major 3×3 affine matrix used for bounding-box computation.
///
/// Mirrors the same transform chain built by [`Projection`] but exposes the
/// matrix values for point transformation, which [`Projection`] keeps private.
#[derive(Clone, Copy)]
struct Affine3x3([f32; 9]);

impl Affine3x3 {
    /// Scale by `(sx, sy)`.
    #[rustfmt::skip]
    fn scale(sx: f32, sy: f32) -> Self {
        Self([
            sx,  0.0, 0.0,
            0.0, sy,  0.0,
            0.0, 0.0, 1.0,
        ])
    }

    /// Translate by `(tx, ty)`.
    #[rustfmt::skip]
    fn translate(tx: f32, ty: f32) -> Self {
        Self([
            1.0, 0.0, tx,
            0.0, 1.0, ty,
            0.0, 0.0, 1.0,
        ])
    }

    /// Clockwise rotation by `theta` radians.
    #[rustfmt::skip]
    fn rotate(theta: f32) -> Self {
        let (s, c) = theta.sin_cos();
        Self([
             c, -s, 0.0,
             s,  c, 0.0,
            0.0, 0.0, 1.0,
        ])
    }

    /// Compose: `self` applied first, then `next`.
    ///
    /// Equivalent to `Projection::and_then`: `A.then(B)` = B × A.
    fn then(self, next: Self) -> Self {
        Self(mul3x3(next.0, self.0))
    }
}

/// Multiply two row-major 3×3 matrices.
fn mul3x3(a: [f32; 9], b: [f32; 9]) -> [f32; 9] {
    #[rustfmt::skip]
    let [
        a0, a1, a2,
        a3, a4, a5,
        a6, a7, a8,
    ] = a;
    #[rustfmt::skip]
    let [
        b0, b1, b2,
        b3, b4, b5,
        b6, b7, b8,
    ] = b;
    [
        a0 * b0 + a1 * b3 + a2 * b6,
        a0 * b1 + a1 * b4 + a2 * b7,
        a0 * b2 + a1 * b5 + a2 * b8,
        a3 * b0 + a4 * b3 + a5 * b6,
        a3 * b1 + a4 * b4 + a5 * b7,
        a3 * b2 + a4 * b5 + a5 * b8,
        a6 * b0 + a7 * b3 + a8 * b6,
        a6 * b1 + a7 * b4 + a8 * b7,
        a6 * b2 + a7 * b5 + a8 * b8,
    ]
}

/// The class of an affine transform, used to select the warp implementation.
///
/// Scale+translate transforms can use SIMD-accelerated resize instead of
/// general affine warping, yielding ~9× speedup.
enum TransformClass {
    /// Scale + translate only (off-diagonal elements are zero, positive scales).
    /// Can use `fast_image_resize` instead of general affine warp.
    ScaleTranslate,
    /// General affine (rotation, skew, or negative scale).
    /// Falls through to `imageproc::warp_into`.
    General,
}

/// Classify the forward transform by inspecting its matrix elements.
///
/// When rotation is zero and no negative scaling is applied, the off-diagonal
/// elements of the 2×2 sub-matrix are zero, indicating a pure scale + translate.
fn classify_transform(forward: &[f32; 9]) -> TransformClass {
    const EPSILON: f32 = 1e-4;
    let off_diag_zero = forward[1].abs() < EPSILON && forward[3].abs() < EPSILON;
    let positive_scale = forward[0] > 0.0 && forward[4] > 0.0;
    if off_diag_zero && positive_scale {
        TransformClass::ScaleTranslate
    } else {
        TransformClass::General
    }
}

/// Warp a source image using SIMD-accelerated resize for scale+translate transforms,
/// writing into a pre-allocated output buffer.
///
/// The output buffer must be sized to `(aabb_w, aabb_h)` and will be cleared before
/// resizing.
///
/// # Arguments
///
/// * `output` - Pre-allocated output buffer, sized to the AABB.
/// * `source` - Source image.
/// * `forward` - Forward transform matrix (must be scale+translate).
/// * `aabb` - The `(x, y, w, h)` bounding box clamped to canvas. Only `x` and `y` are
///   used to compute the crop box; the output size is determined by the buffer dimensions.
fn warp_resize_into(
    output: &mut RgbaImage,
    source: &RgbaImage,
    forward: &[f32; 9],
    (aabb_x, aabb_y, aabb_w, aabb_h): (u32, u32, u32, u32),
) {
    let sx = forward[0] as f64;
    let sy = forward[4] as f64;
    let tx = forward[2] as f64;
    let ty = forward[5] as f64;

    // Compute the source crop box: which source pixels map to the visible AABB.
    let crop_left = (aabb_x as f64 - tx) / sx;
    let crop_top = (aabb_y as f64 - ty) / sy;
    let crop_width = aabb_w as f64 / sx;
    let crop_height = aabb_h as f64 / sy;

    // Clamp the crop box to source image bounds.
    // The AABB includes 1px padding for bilinear interpolation, which can cause
    // the crop box to extend beyond the source. Clamping is safe — out-of-bounds
    // source pixels would sample the background anyway (transparent black).
    let src_w = source.width() as f64;
    let src_h = source.height() as f64;
    let clamped_left = crop_left.max(0.0);
    let clamped_top = crop_top.max(0.0);
    let clamped_right = (crop_left + crop_width).min(src_w);
    let clamped_bottom = (crop_top + crop_height).min(src_h);
    let clamped_width = clamped_right - clamped_left;
    let clamped_height = clamped_bottom - clamped_top;

    // If the clamped crop has zero area, fill output with transparent pixels.
    if clamped_width <= 0.0 || clamped_height <= 0.0 {
        for pixel in output.pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
        return;
    }

    // Clear output to transparent before resizing (fast_image_resize may not write all
    // pixels if the crop box is smaller than the output).
    for pixel in output.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    let mut resizer = fast_image_resize::Resizer::new();
    let options = fast_image_resize::ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(
            fast_image_resize::FilterType::Bilinear,
        ))
        .crop(clamped_left, clamped_top, clamped_width, clamped_height);

    // RgbaImage implements IntoImageView/IntoImageViewMut via fast_image_resize's
    // "image" feature — no data conversion needed.
    resizer
        .resize(source, output, &options)
        .expect("crop box is clamped to source bounds; pixel types match");
}

/// Result of warping a clip: a bounding-box-sized sub-buffer, its (x, y) offset on the
/// canvas, and an optional receipt for returning the buffer to the pool.
struct WarpedClip {
    /// The warped pixel data, sized to the bounding box.
    buffer: RgbaImage,
    /// Top-left corner of the bounding box on the canvas.
    offset: (u32, u32),
    /// Receipt for returning the buffer to the pool after compositing.
    /// `None` in tests (non-pooled buffers).
    receipt: Option<PooledBuffer<buffer_pool::Empty>>,
}

/// Return all pooled buffers from warped clips back to the pool.
///
/// Clips without receipts (tests) are skipped. If a buffer size has changed
/// (should not happen in practice), the buffer is silently dropped.
fn return_buffers(pool: &BufferPool, clips: Vec<WarpedClip>) {
    for clip in clips {
        if let Some(receipt) = clip.receipt {
            let raw = clip.buffer.into_raw();
            match receipt.replace(raw) {
                Ok(ready) => pool.release(ready),
                Err(_) => {} // wrong size — dropped
            }
        }
    }
}

/// Compute the axis-aligned bounding box of a transformed source image, clamped to canvas bounds.
///
/// Transforms the four corners of the source image through the forward matrix,
/// takes the AABB of the resulting points, clamps to `[0, canvas_w) × [0, canvas_h)`,
/// and returns `(x, y, w, h)`.
///
/// Returns `(0, 0, 0, 0)` if the transformed image is entirely off-screen.
///
/// # Arguments
///
/// * `forward` - Row-major 3×3 affine matrix mapping source → destination.
/// * `src_w`, `src_h` - Source image dimensions.
/// * `canvas_w`, `canvas_h` - Canvas (output) dimensions.
fn transformed_aabb(
    forward: &[f32; 9],
    src_w: u32,
    src_h: u32,
    canvas_w: u32,
    canvas_h: u32,
) -> (u32, u32, u32, u32) {
    let sw = src_w as f32;
    let sh = src_h as f32;

    // Transform the four source corners to destination space.
    let corners = [
        apply_affine(forward, 0.0, 0.0),
        apply_affine(forward, sw, 0.0),
        apply_affine(forward, 0.0, sh),
        apply_affine(forward, sw, sh),
    ];

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for (cx, cy) in &corners {
        min_x = min_x.min(*cx);
        min_y = min_y.min(*cy);
        max_x = max_x.max(*cx);
        max_y = max_y.max(*cy);
    }

    // Add 1-pixel padding for bilinear interpolation edge sampling.
    min_x -= 1.0;
    min_y -= 1.0;
    max_x += 1.0;
    max_y += 1.0;

    // Clamp to canvas bounds.
    let min_x = min_x.max(0.0);
    let min_y = min_y.max(0.0);
    let max_x = max_x.min(canvas_w as f32);
    let max_y = max_y.min(canvas_h as f32);

    if max_x <= min_x || max_y <= min_y {
        return (0, 0, 0, 0);
    }

    let x = min_x as u32;
    let y = min_y as u32;
    let w = (max_x as u32).saturating_sub(x);
    let h = (max_y as u32).saturating_sub(y);

    (x, y, w, h)
}

/// Apply a row-major 3×3 affine matrix to a 2D point.
///
/// For affine transforms the bottom row is `[0, 0, 1]`, so only the top 2 rows are used:
/// `x' = m[0]*x + m[1]*y + m[2]`, `y' = m[3]*x + m[4]*y + m[5]`.
fn apply_affine(m: &[f32; 9], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
}

/// Composite all warped clips onto the canvas in z-order using row-parallel compositing.
///
/// Splits the canvas into horizontal bands (one per rayon thread) and composites all clips
/// onto each band in z-order in parallel. Bands are disjoint row ranges, so no
/// synchronization is needed and z-order is preserved within each band.
#[instrument(name = "composite_clips", skip_all, fields(clip_count = warped_clips.len()))]
fn composite_clips(canvas: &mut RgbaImage, warped_clips: &[WarpedClip]) {
    let (w, h) = canvas.dimensions();
    let width = w as usize;

    // Extract the raw byte buffer to enable parallel mutable access.
    // We'll reconstitute the canvas after the parallel work.
    let mut raw: Vec<u8> = std::mem::replace(canvas, RgbaImage::new(0, 0)).into_raw();

    // Safety: Rgba<u8> is #[repr(C)] with a single [u8; 4] field.
    // Alignment of Rgba<u8> equals alignment of [u8; 4] which is 1.
    // The buffer has width * height * 4 bytes → width * height Rgba<u8> pixels.
    let pixels: &mut [Rgba<u8>] = unsafe {
        std::slice::from_raw_parts_mut(raw.as_mut_ptr().cast::<Rgba<u8>>(), raw.len() / 4)
    };

    // Split into bands of multiple rows. One band per rayon thread avoids
    // fine-grained task overhead while maximizing parallelism.
    let num_threads = rayon::current_num_threads().max(1);
    let rows_per_band = (h as usize).div_ceil(num_threads).max(1);
    let band_size = width * rows_per_band;

    pixels
        .par_chunks_mut(band_size)
        .enumerate()
        .for_each(|(band_idx, band)| {
            let band_y_start = (band_idx * rows_per_band) as u32;

            for warped in warped_clips {
                composite_onto_band(band, width, band_y_start, &warped.buffer, warped.offset);
            }
        });

    // Reconstitute the canvas from the modified buffer.
    *canvas = RgbaImage::from_raw(w, h, raw).unwrap();
}

/// Alpha composite a layer onto a horizontal band of the canvas using source-over blending.
///
/// The band is a contiguous slice of pixels representing one or more full rows of the canvas.
/// Only the rows of the layer that overlap with the band range are processed.
/// Fully transparent pixels are skipped, making it efficient for small layers on large canvases.
fn composite_onto_band(
    band: &mut [Rgba<u8>],
    band_width: usize,
    band_y_start: u32,
    layer: &RgbaImage,
    offset: (u32, u32),
) {
    if layer.width() == 0 || layer.height() == 0 {
        return;
    }

    let (off_x, off_y) = offset;
    let layer_w = layer.width();
    let layer_h = layer.height();
    let band_rows = band.len() / band_width;
    let band_y_end = band_y_start + band_rows as u32;

    // Determine which layer rows overlap with this band.
    let ly_start = band_y_start.saturating_sub(off_y);
    let ly_end = band_y_end.saturating_sub(off_y).min(layer_h);

    if ly_start >= ly_end {
        return;
    }

    for ly in ly_start..ly_end {
        let cy = off_y + ly;
        let band_row = (cy - band_y_start) as usize;
        let row_offset = band_row * band_width;

        for lx in 0..layer_w {
            let cx = off_x + lx;
            if cx >= band_width as u32 {
                break;
            }

            let layer_pixel = layer.get_pixel(lx, ly);

            // Skip fully transparent pixels — common in warped sub-buffers.
            if layer_pixel.0[3] == 0 {
                continue;
            }

            let canvas_pixel = &mut band[row_offset + cx as usize];

            let ca = canvas_pixel.0[3] as f32 / 255.0;
            let la = layer_pixel.0[3] as f32 / 255.0;

            let out_a = la + ca * (1.0 - la);
            if out_a < 1e-6 {
                continue;
            }

            let out_r =
                (layer_pixel.0[0] as f32 * la + canvas_pixel.0[0] as f32 * ca * (1.0 - la)) / out_a;
            let out_g =
                (layer_pixel.0[1] as f32 * la + canvas_pixel.0[1] as f32 * ca * (1.0 - la)) / out_a;
            let out_b =
                (layer_pixel.0[2] as f32 * la + canvas_pixel.0[2] as f32 * ca * (1.0 - la)) / out_a;

            *canvas_pixel = Rgba([out_r as u8, out_g as u8, out_b as u8, (out_a * 255.0) as u8]);
        }
    }
}

/// Alpha composite a layer onto the canvas at the given offset using source-over blending.
///
/// Only iterates pixels within the layer's bounding box and skips fully transparent
/// pixels, making it efficient for small layers on large canvases.
///
/// This is the sequential per-clip compositing function used by tests. The parallel
/// path uses [`composite_onto_band`] via [`composite_clips`].
#[cfg(test)]
fn composite_onto(canvas: &mut RgbaImage, layer: &RgbaImage, offset: (u32, u32)) {
    if layer.width() == 0 || layer.height() == 0 {
        return;
    }

    let (off_x, off_y) = offset;
    let layer_w = layer.width();
    let layer_h = layer.height();
    let canvas_w = canvas.width();
    let canvas_h = canvas.height();

    for ly in 0..layer_h {
        let cy = off_y + ly;
        if cy >= canvas_h {
            break;
        }
        for lx in 0..layer_w {
            let cx = off_x + lx;
            if cx >= canvas_w {
                break;
            }

            let layer_pixel = layer.get_pixel(lx, ly);

            // Skip fully transparent pixels — common in warped sub-buffers.
            if layer_pixel.0[3] == 0 {
                continue;
            }

            let canvas_pixel = canvas.get_pixel_mut(cx, cy);

            let ca = canvas_pixel.0[3] as f32 / 255.0;
            let la = layer_pixel.0[3] as f32 / 255.0;

            let out_a = la + ca * (1.0 - la);
            if out_a < 1e-6 {
                continue;
            }

            let out_r =
                (layer_pixel.0[0] as f32 * la + canvas_pixel.0[0] as f32 * ca * (1.0 - la)) / out_a;
            let out_g =
                (layer_pixel.0[1] as f32 * la + canvas_pixel.0[1] as f32 * ca * (1.0 - la)) / out_a;
            let out_b =
                (layer_pixel.0[2] as f32 * la + canvas_pixel.0[2] as f32 * ca * (1.0 - la)) / out_a;

            *canvas_pixel = Rgba([out_r as u8, out_g as u8, out_b as u8, (out_a * 255.0) as u8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- transformed_aabb tests ---

    fn identity_matrix() -> [f32; 9] {
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
    }

    #[test]
    fn transformed_aabb_identity_returns_source_rect_with_padding() {
        // Given an identity transform and a 100x50 source on a 200x200 canvas.
        let matrix = identity_matrix();

        // When computing the AABB.
        let (x, y, w, h) = transformed_aabb(&matrix, 100, 50, 200, 200);

        // Then the AABB covers the source with 1px padding on each side.
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        // Corners (0,0)→(100,50), padding ±1, clamped: [0, 101] × [0, 51].
        assert_eq!(w, 101);
        assert_eq!(h, 51);
    }

    #[test]
    fn transformed_aabb_scaled_projection_returns_scaled_rect() {
        // Given a 2x scale transform.
        let matrix = Affine3x3::scale(2.0, 2.0).0;

        // When computing the AABB for a 50x50 source on a 200x200 canvas.
        let (x, y, w, h) = transformed_aabb(&matrix, 50, 50, 200, 200);

        // Then the AABB is roughly 100x100 (with padding).
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        assert_eq!(w, 101); // 100 + 1 padding (only right side, left clamped to 0)
        assert_eq!(h, 101);
    }

    #[test]
    fn transformed_aabb_translated_projection_shifts_position() {
        // Given a translate(50, 30) transform.
        let matrix = Affine3x3::translate(50.0, 30.0).0;

        // When computing the AABB for a 40x20 source on a 200x200 canvas.
        let (x, y, w, h) = transformed_aabb(&matrix, 40, 20, 200, 200);

        // Then the AABB starts near (49, 29) with padding, size ~42x22.
        assert_eq!(x, 49); // 50 - 1 padding, clamped to 0
        assert_eq!(y, 29); // 30 - 1 padding, clamped to 0
        assert_eq!(w, 42); // 40 + 2 padding
        assert_eq!(h, 22); // 20 + 2 padding
    }

    #[test]
    fn transformed_aabb_offscreen_returns_zero() {
        // Given a translate that moves the image entirely off the right edge.
        let matrix = Affine3x3::translate(300.0, 0.0).0;

        // When computing the AABB for a 50x50 source on a 200x200 canvas.
        let result = transformed_aabb(&matrix, 50, 50, 200, 200);

        // Then the result is (0, 0, 0, 0).
        assert_eq!(result, (0, 0, 0, 0));
    }

    #[test]
    fn transformed_aabb_partially_offscreen_clamps_to_canvas() {
        // Given a translate that moves the image partially off the right edge.
        let matrix = Affine3x3::translate(180.0, 0.0).0;

        // When computing the AABB for a 50x50 source on a 200x200 canvas.
        let (x, _y, w, _h) = transformed_aabb(&matrix, 50, 50, 200, 200);

        // Then the AABB starts near x=179 and is clamped to canvas width 200.
        assert_eq!(x, 179); // 180 - 1 padding
        // Width = min(231, 200) - 179 = 200 - 179 = 21
        assert_eq!(w, 21);
    }

    #[test]
    fn transformed_aabb_rotated_square_produces_larger_bbox() {
        // Given a 45-degree rotation of a 100x100 square.
        let angle = std::f32::consts::FRAC_PI_4; // 45 degrees
        let matrix = Affine3x3::rotate(angle).0;

        // When computing the AABB for a 100x100 source on a 200x200 canvas.
        let (_x, _y, w, h) = transformed_aabb(&matrix, 100, 100, 200, 200);

        // Then the AABB is smaller than the source in width because rotation
        // around the origin tucks the square into a diamond. The diagonal of
        // the 100x100 square ≈ 141 pixels vertically.
        // Width: max rotated x ≈ 71, height: max rotated y ≈ 142.
        assert!((70..=73).contains(&w), "expected w ≈ 71, got {w}");
        assert!((140..=145).contains(&h), "expected h ≈ 142, got {h}");
    }

    // --- composite_onto tests ---

    #[test]
    fn composite_at_offset_blends_correctly() {
        // Given a 4x4 canvas with all red pixels (full opacity).
        let mut canvas = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255]));

        // And a 2x2 layer with semi-transparent green at offset (1, 1).
        let layer = RgbaImage::from_pixel(2, 2, Rgba([0, 255, 0, 128]));

        // When compositing the layer at offset (1, 1).
        composite_onto(&mut canvas, &layer, (1, 1));

        // Then only pixels at (1,1), (2,1), (1,2), (2,2) changed.
        // Unaffected pixels remain red.
        assert_eq!(canvas.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(3, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(0, 3), &Rgba([255, 0, 0, 255]));
        assert_eq!(canvas.get_pixel(3, 3), &Rgba([255, 0, 0, 255]));

        // Blended pixels should have changed from pure red.
        let blended = canvas.get_pixel(1, 1);
        assert_ne!(blended.0[0], 255, "red channel should have changed");
        assert_ne!(blended.0[1], 0, "green channel should have changed");
    }

    #[test]
    fn composite_empty_layer_is_noop() {
        // Given a 4x4 canvas with known content.
        let mut canvas = RgbaImage::from_pixel(4, 4, Rgba([100, 150, 200, 255]));
        let original = canvas.clone();

        // And a zero-sized layer.
        let empty = RgbaImage::new(0, 0);

        // When compositing the empty layer.
        composite_onto(&mut canvas, &empty, (0, 0));

        // Then the canvas is unchanged.
        assert_eq!(canvas.as_raw(), original.as_raw());
    }

    #[test]
    fn composite_transparent_layer_is_noop() {
        // Given a 4x4 canvas with known content.
        let mut canvas = RgbaImage::from_pixel(4, 4, Rgba([100, 150, 200, 255]));
        let original = canvas.clone();

        // And a 2x2 layer that is fully transparent.
        let transparent = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0]));

        // When compositing the transparent layer at (1, 1).
        composite_onto(&mut canvas, &transparent, (1, 1));

        // Then the canvas is unchanged.
        assert_eq!(canvas.as_raw(), original.as_raw());
    }

    // --- composite_clips (row-parallel) tests ---

    #[test]
    fn composite_clips_parallel_matches_sequential() {
        // Given a 10x10 canvas and two overlapping clips.
        let mut canvas_par = RgbaImage::from_pixel(10, 10, Rgba([50, 50, 50, 255]));
        let mut canvas_seq = canvas_par.clone();

        // Clip 1: semi-transparent red at (2, 2).
        let clip1 = WarpedClip {
            buffer: RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 180])),
            offset: (2, 2),
            receipt: None,
        };

        // Clip 2: semi-transparent blue at (4, 4) — overlaps clip 1.
        let clip2 = WarpedClip {
            buffer: RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 200])),
            offset: (4, 4),
            receipt: None,
        };

        let clips = vec![clip1, clip2];

        // When compositing with the parallel path.
        composite_clips(&mut canvas_par, &clips);

        // And compositing sequentially (ground truth).
        for warped in &clips {
            composite_onto(&mut canvas_seq, &warped.buffer, warped.offset);
        }

        // Then both canvases are identical.
        assert_eq!(canvas_par.as_raw(), canvas_seq.as_raw());
    }

    #[test]
    fn composite_clips_parallel_with_empty_clips() {
        // Given a 6x6 canvas and a mix of empty and non-empty clips.
        let mut canvas = RgbaImage::from_pixel(6, 6, Rgba([100, 100, 100, 255]));
        let original = canvas.clone();

        let clips = vec![
            WarpedClip {
                buffer: RgbaImage::new(0, 0),
                offset: (0, 0),
                receipt: None,
            },
            WarpedClip {
                buffer: RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 128])),
                offset: (2, 2),
                receipt: None,
            },
            WarpedClip {
                buffer: RgbaImage::new(0, 0),
                offset: (0, 0),
                receipt: None,
            },
        ];

        // When compositing with the parallel path.
        composite_clips(&mut canvas, &clips);

        // Then only the non-empty clip affected the canvas.
        let mut expected = original.clone();
        composite_onto(&mut expected, &clips[1].buffer, clips[1].offset);
        assert_eq!(canvas.as_raw(), expected.as_raw());
    }

    // --- classify_transform tests ---

    #[test]
    fn classify_identity_is_scale_translate() {
        // Given an identity matrix.
        let matrix = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

        // When classifying.
        let result = classify_transform(&matrix);

        // Then it is classified as scale+translate.
        assert!(matches!(result, TransformClass::ScaleTranslate));
    }

    #[test]
    fn classify_rotation_is_general() {
        // Given a 45-degree rotation matrix.
        let (s, c) = std::f32::consts::FRAC_PI_4.sin_cos();
        let matrix = [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0];

        // When classifying.
        let result = classify_transform(&matrix);

        // Then it is classified as general.
        assert!(matches!(result, TransformClass::General));
    }

    #[test]
    fn classify_scale_translate_is_scale_translate() {
        // Given a scale(2, 3) + translate(100, 200) matrix.
        let matrix = [2.0, 0.0, 100.0, 0.0, 3.0, 200.0, 0.0, 0.0, 1.0];

        // When classifying.
        let result = classify_transform(&matrix);

        // Then it is classified as scale+translate.
        assert!(matches!(result, TransformClass::ScaleTranslate));
    }

    #[test]
    fn classify_negative_scale_is_general() {
        // Given a negative scale matrix (horizontal flip).
        let matrix = [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

        // When classifying.
        let result = classify_transform(&matrix);

        // Then it is classified as general (negative scale not supported by fast path).
        assert!(matches!(result, TransformClass::General));
    }

    // --- occlusion_cull tests ---

    fn test_plan(
        aabb: (u32, u32, u32, u32),
        opacity: f32,
        transform_class: TransformClass,
    ) -> ClipPlan {
        ClipPlan {
            resolved: ResolvedItem {
                item: ss_core::test_utils::fixtures::build_image_item(
                    "test", "test.png", 0.0, 10.0,
                ),
                translate: euclid::vec2(0.0, 0.0),
                scale: euclid::vec2(1.0, 1.0),
                rotation: 0.0,
                opacity,
                z_path: vec![0],
            },
            source: Arc::new(RgbaImage::new(1, 1)),
            placement: PlacedRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0,
            },
            forward: Affine3x3::scale(1.0, 1.0),
            aabb,
            opacity,
            transform_class,
            viewport_scale: (1.0, 1.0),
        }
    }

    #[test]
    fn occlusion_cull_fully_occluded_clip_is_culled() {
        // Given two plans sorted by z ascending:
        //   plan[0]: bottom, AABB (0,0,1920,1080), opaque, ScaleTranslate
        //   plan[1]: top, AABB (0,0,1920,1080), opaque, ScaleTranslate
        let plans = vec![
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then plan[0] is culled (false), plan[1] is visible (true).
        assert!(!visible[0], "bottom clip should be culled");
        assert!(visible[1], "top clip should be visible");
    }

    #[test]
    fn occlusion_cull_partially_visible_clip_is_not_culled() {
        // Given two plans:
        //   plan[0]: bottom, AABB (0,0,1920,1080), opaque, ScaleTranslate
        //   plan[1]: top, AABB (0,0,960,1080), opaque, ScaleTranslate (covers left half)
        let plans = vec![
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 960, 1080), 1.0, TransformClass::ScaleTranslate),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then both are visible — bottom clip extends beyond the occluder.
        assert!(
            visible[0],
            "bottom clip should be visible (extends beyond occluder)"
        );
        assert!(visible[1], "top clip should be visible");
    }

    #[test]
    fn occlusion_cull_rotated_clip_is_not_occluder() {
        // Given two plans:
        //   plan[0]: bottom, AABB (0,0,1920,1080), opaque, ScaleTranslate
        //   plan[1]: top, AABB (0,0,1920,1080), opaque, General (rotated)
        let plans = vec![
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::General),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then both are visible — rotated clip is not an occluder.
        assert!(
            visible[0],
            "bottom clip should be visible (rotated clip is not occluder)"
        );
        assert!(visible[1], "top clip should be visible");
    }

    #[test]
    fn occlusion_cull_semi_transparent_clip_is_not_occluder() {
        // Given two plans:
        //   plan[0]: bottom, AABB (0,0,1920,1080), opacity=1.0, ScaleTranslate
        //   plan[1]: top, AABB (0,0,1920,1080), opacity=0.5, ScaleTranslate
        let plans = vec![
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 1920, 1080), 0.5, TransformClass::ScaleTranslate),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then both are visible — semi-transparent clip is not an occluder.
        assert!(
            visible[0],
            "bottom clip should be visible (semi-transparent clip is not occluder)"
        );
        assert!(visible[1], "top clip should be visible");
    }

    #[test]
    fn occlusion_cull_zero_aabb_is_culled() {
        // Given two plans:
        //   plan[0]: bottom, AABB (0,0,0,0), opaque, ScaleTranslate
        //   plan[1]: top, AABB (0,0,1920,1080), opaque, ScaleTranslate
        let plans = vec![
            test_plan((0, 0, 0, 0), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then plan[0] is culled, plan[1] is visible.
        assert!(!visible[0], "zero AABB clip should be culled");
        assert!(visible[1], "top clip should be visible");
    }

    #[test]
    fn occlusion_cull_single_occluder_check_no_union() {
        // Given three plans:
        //   plan[0]: bottom, AABB (0,0,1920,1080), opaque, ScaleTranslate
        //   plan[1]: middle, AABB (0,0,960,1080), opaque, ScaleTranslate (left half)
        //   plan[2]: top, AABB (960,0,960,1080), opaque, ScaleTranslate (right half)
        let plans = vec![
            test_plan((0, 0, 1920, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((0, 0, 960, 1080), 1.0, TransformClass::ScaleTranslate),
            test_plan((960, 0, 960, 1080), 1.0, TransformClass::ScaleTranslate),
        ];

        // When calling occlusion_cull.
        let visible = occlusion_cull(&plans);

        // Then plan[0] is visible — neither occluder alone fully contains it.
        assert!(
            visible[0],
            "bottom clip should be visible (no single occluder fully contains it)"
        );
        assert!(visible[1], "middle clip should be visible");
        assert!(visible[2], "top clip should be visible");
    }

    // --- warp_resize vs warp_into correctness test ---

    #[test]
    fn warp_resize_matches_warp_into_for_scale_translate() {
        // Given a 100×100 source image with a gradient pattern.
        let mut source = RgbaImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                source.put_pixel(x, y, Rgba([x as u8, y as u8, 128, 255]));
            }
        }

        // And a scale(2.0, 2.0) + translate(10, 20) transform.
        let forward = [2.0f32, 0.0, 10.0, 0.0, 2.0, 20.0, 0.0, 0.0, 1.0];
        let aabb = (10u32, 20u32, 150u32, 150u32);

        // When using the fast resize path.
        let mut fast_result = RgbaImage::new(aabb.2, aabb.3);
        warp_resize_into(&mut fast_result, &source, &forward, aabb);

        // And using the general warp_into path.
        let adjusted =
            Affine3x3::translate(-(aabb.0 as f32), -(aabb.1 as f32)).then(Affine3x3(forward));
        let projection = Projection::from_matrix(adjusted.0).unwrap();
        let mut general_buf = RgbaImage::from_pixel(aabb.2, aabb.3, Rgba([0, 0, 0, 0]));
        warp_into(
            &source,
            &projection,
            Interpolation::Bilinear,
            Rgba([0, 0, 0, 0]),
            &mut general_buf,
        );

        // Then the outputs should be very similar.
        // Allow small differences due to different bilinear implementations
        // (fast_image_resize uses premultiplied alpha, warp_into uses straight alpha).
        let fast_raw = fast_result.as_raw();
        let general_raw = general_buf.as_raw();
        let mut max_diff = 0u8;
        for (a, b) in fast_raw.iter().zip(general_raw.iter()) {
            max_diff = max_diff.max(a.abs_diff(*b));
        }
        assert!(max_diff <= 12, "max pixel difference: {max_diff}");
    }
}
