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
use tracing::debug;

use ss_core::interpolation::resolve_items;
use ss_core::item::ItemContent;
use ss_core::path_resolve::resolve_path;
use ss_core::project::Project;

use crate::image::ImageProvider;
use crate::rendering::CompositorError;
use crate::rendering::FrameRenderer;
use crate::sizing::compute_placement;
use crate::viewport::Viewport;

/// The main compositor renderer.
///
/// Uses an [`ImageProvider`] to load images and renders frames by compositing
/// clips with transforms, alpha blending, and z-ordering.
pub struct CompositorRenderer {
    image_provider: Arc<dyn ImageProvider>,
}

impl CompositorRenderer {
    /// Create a new renderer with the given image provider.
    pub fn new(image_provider: Arc<dyn ImageProvider>) -> Self {
        Self { image_provider }
    }
}

impl FrameRenderer for CompositorRenderer {
    fn name(&self) -> &'static str {
        "compositor"
    }

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

        debug!("rendering frame: time={}, items={}", time, resolved.len());

        // 4. For each resolved item: warp, composite.
        for resolved_item in &resolved {
            render_resolved_item(
                &mut canvas,
                &self.image_provider,
                resolved_item,
                project_file,
                viewport,
                &project.resolution,
            )?;
        }

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

/// Render a single resolved item onto the canvas.
fn render_resolved_item(
    canvas: &mut RgbaImage,
    image_provider: &Arc<dyn ImageProvider>,
    resolved: &ss_core::transform::ResolvedItem,
    project_file: &Path,
    viewport: &Viewport,
    project_resolution: &[u32; 2],
) -> Result<(), Report<CompositorError>> {
    // Resolve the image path.
    let image_path = match &resolved.item.content {
        ItemContent::Image { path } => resolve_path(project_file, path)
            .change_context(CompositorError)
            .attach(resolved.item.id.clone())?,
        // Groups are flattened by `resolve_items`; they never appear here.
        ItemContent::Group { .. } => unreachable!("groups should be flattened by resolve_items"),
    };

    // Load the source image.
    let source_image = image_provider
        .get(&image_path)
        .change_context(CompositorError)
        .attach(resolved.item.id.clone())?;

    // Compute base placement from sizing (in project-space).
    let placement = compute_placement(
        &resolved.item.sizing,
        (source_image.width(), source_image.height()),
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

    // Build the transform and warp the source image into a bounding-box-sized buffer.
    let warped = warp_clip(
        &source_image,
        &placement,
        resolved,
        viewport,
        viewport_scale,
    );

    // Alpha composite the warped clip onto the canvas at its bounding-box offset.
    composite_onto(canvas, &warped.buffer, warped.offset);

    Ok(())
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
        a0*b0 + a1*b3 + a2*b6, a0*b1 + a1*b4 + a2*b7, a0*b2 + a1*b5 + a2*b8,
        a3*b0 + a4*b3 + a5*b6, a3*b1 + a4*b4 + a5*b7, a3*b2 + a4*b5 + a5*b8,
        a6*b0 + a7*b3 + a8*b6, a6*b1 + a7*b4 + a8*b7, a6*b2 + a7*b5 + a8*b8,
    ]
}

/// Result of warping a clip: a bounding-box-sized sub-buffer and its (x, y) offset on the canvas.
struct WarpedClip {
    /// The warped pixel data, sized to the bounding box.
    buffer: RgbaImage,
    /// Top-left corner of the bounding box on the canvas.
    offset: (u32, u32),
}

/// Warp a source image into a bounding-box-sized buffer using the full transform pipeline.
///
/// Instead of allocating a full-canvas buffer, computes the axis-aligned bounding box
/// of the transformed clip and warps only into that region. Returns the sub-buffer and
/// its offset on the canvas.
///
/// The transform pipeline maps source pixels to their destination position on the canvas:
/// 1. Scale source to placement size
/// 2. Translate to placement position
/// 3. Rotate and scale around pivot (animation)
/// 4. Translate by animation offset
/// 5. Apply camera pan
fn warp_clip(
    source: &RgbaImage,
    placement: &crate::sizing::PlacedRect,
    resolved: &ss_core::transform::ResolvedItem,
    viewport: &Viewport,
    viewport_scale: (f32, f32),
) -> WarpedClip {
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
    let (aabb_x, aabb_y, aabb_w, aabb_h) =
        transformed_aabb(&forward.0, source.width(), source.height(), canvas_w, canvas_h);

    // Off-screen or degenerate clip — no work to do.
    if aabb_w == 0 || aabb_h == 0 {
        return WarpedClip {
            buffer: RgbaImage::new(0, 0),
            offset: (0, 0),
        };
    }

    // Adjust the forward matrix so sub-buffer coords map correctly:
    // sub-buffer (0,0) → canvas (aabb_x, aabb_y) → source.
    let adjusted = Affine3x3::translate(-(aabb_x as f32), -(aabb_y as f32)).then(forward);

    // Create a Projection from the adjusted matrix for warp_into.
    let projection = Projection::from_matrix(adjusted.0)
        .expect("transform matrix composed of scale/translate/rotate is invertible");

    let mut output = RgbaImage::from_pixel(aabb_w, aabb_h, Rgba([0, 0, 0, 0]));
    warp_into(
        source,
        &projection,
        Interpolation::Bilinear,
        Rgba([0u8, 0u8, 0u8, 0u8]),
        &mut output,
    );

    // Apply opacity to alpha channel.
    if resolved.opacity < 1.0 {
        for pixel in output.pixels_mut() {
            pixel.0[3] = (pixel.0[3] as f32 * resolved.opacity) as u8;
        }
    }

    WarpedClip {
        buffer: output,
        offset: (aabb_x, aabb_y),
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
    (
        m[0] * x + m[1] * y + m[2],
        m[3] * x + m[4] * y + m[5],
    )
}

/// Alpha composite a layer onto the canvas at the given offset using source-over blending.
///
/// Only iterates pixels within the layer's bounding box and skips fully transparent
/// pixels, making it efficient for small layers on large canvases.
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

            let out_r = (layer_pixel.0[0] as f32 * la
                + canvas_pixel.0[0] as f32 * ca * (1.0 - la))
                / out_a;
            let out_g = (layer_pixel.0[1] as f32 * la
                + canvas_pixel.0[1] as f32 * ca * (1.0 - la))
                / out_a;
            let out_b = (layer_pixel.0[2] as f32 * la
                + canvas_pixel.0[2] as f32 * ca * (1.0 - la))
                / out_a;

            *canvas_pixel = Rgba([
                out_r as u8,
                out_g as u8,
                out_b as u8,
                (out_a * 255.0) as u8,
            ]);
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
}
