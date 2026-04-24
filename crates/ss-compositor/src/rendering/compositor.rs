//! Main rendering pipeline.
//!
//! Composites clips onto a canvas with transforms, alpha blending, and z-ordering.
//! Uses `imageproc` for affine transforms via [`Projection`].

use std::path::Path;
use std::sync::Arc;

use error_stack::{Report, ResultExt};
use image::{Rgba, RgbaImage};
use imageproc::geometric_transformations::{Interpolation, Projection, warp_into};
use tracing::debug;

use ss_core::clip::ClipType;
use ss_core::interpolation::resolve_clip;
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

        // 3. Find active clips sorted by z_index.
        let mut active: Vec<_> = project
            .clips
            .iter()
            .filter(|c| time >= c.start_time && time < c.end_time)
            .collect();
        active.sort_by_key(|c| c.z_index);

        debug!("rendering frame: time={}, clips={}", time, active.len());

        // 4. For each clip: resolve transforms, warp, composite.
        for clip_def in active {
            render_clip(
                &mut canvas,
                &self.image_provider,
                clip_def,
                project_file,
                time,
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

/// Render a single clip onto the canvas.
fn render_clip(
    canvas: &mut RgbaImage,
    image_provider: &Arc<dyn ImageProvider>,
    clip_def: &ss_core::clip::ClipDef,
    project_file: &Path,
    time: f64,
    viewport: &Viewport,
    project_resolution: &[u32; 2],
) -> Result<(), Report<CompositorError>> {
    // Resolve the clip's animated state.
    let resolved = resolve_clip(clip_def, time)
        .change_context(CompositorError)
        .attach(clip_def.id.clone())?;

    // Resolve the image path.
    let image_path = match &clip_def.clip_type {
        ClipType::Image { path } => resolve_path(project_file, path)
            .change_context(CompositorError)
            .attach(clip_def.id.clone())?,
    };

    // Load the source image.
    let source_image = image_provider
        .get(&image_path)
        .change_context(CompositorError)
        .attach(clip_def.id.clone())?;

    // Compute base placement from sizing (in project-space).
    let placement = compute_placement(
        &clip_def.sizing,
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

    // Build the transform and warp the source image into a canvas-sized buffer.
    let clip_layer = warp_clip(&source_image, &placement, &resolved, viewport, viewport_scale);

    // Alpha composite the clip onto the canvas.
    composite_onto(canvas, &clip_layer);

    Ok(())
}

/// Warp a source image onto a canvas-sized buffer using the full transform pipeline.
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
    resolved: &ss_core::transform::ResolvedClip,
    viewport: &Viewport,
    viewport_scale: (f32, f32),
) -> RgbaImage {
    let (out_w, out_h) = viewport.output_size;

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
    let pivot_x = placement.x + placement.w * resolved.clip.pivot[0];
    let pivot_y = placement.y + placement.h * resolved.clip.pivot[1];

    // Build the forward transform: source → destination.
    // Each and_then applies the previous transform first, then the new one.
    let projection = Projection::scale(scale_x, scale_y)
        .and_then(Projection::translate(placement.x, placement.y))
        .and_then(Projection::translate(-pivot_x, -pivot_y))
        .and_then(Projection::rotate(resolved.rotation))
        .and_then(Projection::scale(resolved.scale.x, resolved.scale.y))
        .and_then(Projection::translate(pivot_x, pivot_y))
        .and_then(Projection::translate(
            resolved.translate.x * viewport_scale.0,
            resolved.translate.y * viewport_scale.1,
        ))
        .and_then(Projection::translate(
            viewport.camera_pan.0,
            viewport.camera_pan.1,
        ));

    let mut output = RgbaImage::from_pixel(out_w, out_h, Rgba([0, 0, 0, 0]));
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

    output
}

/// Alpha composite a layer onto the canvas using source-over blending.
fn composite_onto(canvas: &mut RgbaImage, layer: &RgbaImage) {
    for (canvas_pixel, layer_pixel) in canvas.pixels_mut().zip(layer.pixels()) {
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
