//! Trait definitions for frame rendering and image loading.
//!
//! These traits decouple the compositor from filesystem operations
//! and enable dependency injection for testing.

use std::path::Path;

use error_stack::Report;
use image::RgbaImage;

use crate::errors::{CompositorError, ImageLoadError};
use crate::viewport::Viewport;
use ss_core::project::Project;

/// Loads and caches images by path.
pub trait ImageProvider: Send + Sync {
    /// Returns the name of this provider backend (for debugging).
    fn name(&self) -> &'static str;

    /// Load or retrieve a cached image by path.
    ///
    /// # Errors
    ///
    /// Returns an error if the image cannot be loaded.
    fn get(&self, path: &Path) -> Result<RgbaImage, Report<ImageLoadError>>;
}

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
