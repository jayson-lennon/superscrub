//! Viewport description for rendering.
//!
//! A viewport defines the viewing context: where the canvas sits within
//! the output frame, the output dimensions, and a camera transform
//! for future editor pan/zoom support.

/// Describes the viewing context for rendering a frame.
///
/// For final export: canvas fills the entire output, identity camera.
/// For editor preview: camera can pan/zoom, canvas may be offset within
/// a larger output.
#[derive(Debug, Clone)]
pub struct Viewport {
    /// The canvas rectangle within the output frame (in output pixels).
    /// For final render: (0, 0, project_w, project_h).
    /// For editor preview: may be offset/expanded.
    pub canvas_rect: (u32, u32, u32, u32), // x, y, w, h
    /// Output pixel dimensions of the rendered frame.
    pub output_size: (u32, u32),
    /// Camera pan offset applied to the entire scene.
    /// (0, 0) for final render. Non-zero for editor preview.
    pub camera_pan: (f32, f32),
}

impl Viewport {
    /// Create a viewport for final rendering at the given resolution.
    ///
    /// Canvas fills the entire output. No camera offset.
    pub fn new_for_output(resolution: (u32, u32)) -> Self {
        Self {
            canvas_rect: (0, 0, resolution.0, resolution.1),
            output_size: resolution,
            camera_pan: (0.0, 0.0),
        }
    }
}
