//! Service wrappers for compositor traits.
//!
//! Service wrappers hold `Arc<dyn Trait>` for shared ownership and
//! provide the standard dependency injection pattern used across
//! SuperScrub.

use std::path::Path;
use std::sync::Arc;

use derive_more::Debug;
use image::RgbaImage;
use ss_core::project::Project;

use crate::errors::CompositorError;
use crate::traits::{FrameRenderer, ImageProvider};
use crate::viewport::Viewport;

/// Service wrapper for [`FrameRenderer`].
#[derive(Debug, Clone)]
pub struct FrameRendererService {
    #[debug("backend<{}>", self.backend.name())]
    backend: Arc<dyn FrameRenderer>,
}

impl FrameRendererService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn FrameRenderer>) -> Self {
        Self { backend }
    }

    /// Render a frame of the project at the given time.
    ///
    /// # Errors
    ///
    /// Returns an error if rendering fails.
    pub fn render(
        &self,
        project: &Project,
        project_file: &Path,
        time: f64,
        viewport: &Viewport,
    ) -> Result<RgbaImage, error_stack::Report<CompositorError>> {
        self.backend.render(project, project_file, time, viewport)
    }
}

/// Service wrapper for [`ImageProvider`].
#[derive(Debug, Clone)]
pub struct ImageProviderService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added when needed
    backend: Arc<dyn ImageProvider>,
}

impl ImageProviderService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn ImageProvider>) -> Self {
        Self { backend }
    }
}
