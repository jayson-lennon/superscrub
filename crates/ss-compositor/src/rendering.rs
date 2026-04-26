//! Frame rendering trait and error types.
//!
//! The [`FrameRenderer`] trait abstracts over rendering backends,
//! enabling dependency injection for testing.

use std::path::Path;

use error_stack::Report;
use image::RgbaImage;

pub mod buffer_pool;
pub mod compositor;
pub mod service;

use crate::viewport::Viewport;
use ss_core::project::Project;

/// Failed to render a frame.
#[derive(Debug, wherror::Error)]
#[error("failed to render frame")]
pub struct CompositorError;

/// Renders a single frame of a project at a given time within a viewport.
pub trait FrameRenderer: Send + Sync {
    /// Returns the name of this renderer backend (for debugging).
    fn name(&self) -> &'static str;

    /// Render a frame of the project at the given time.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails (e.g., image load failure).
    fn render(
        &self,
        project: &Project,
        project_file: &Path,
        time: f64,
        viewport: &Viewport,
    ) -> Result<RgbaImage, Report<CompositorError>>;
}
