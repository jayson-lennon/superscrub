//! Service wrapper for the [`PreviewCache`] trait.

use std::path::PathBuf;
use std::sync::Arc;

use derive_more::Debug;
use image::RgbaImage;
use ss_core::project::Project;

use crate::cache::PreviewError;
use crate::cache::{PreviewCache, PreviewRenderProgress};

/// Service wrapper for [`PreviewCache`].
#[derive(Debug, Clone)]
pub struct PreviewCacheService {
    #[debug("backend<{}>", self.backend.name())]
    backend: Arc<dyn PreviewCache>,
}

impl PreviewCacheService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn PreviewCache>) -> Self {
        Self { backend }
    }

    /// Start rendering all frames for the given project.
    ///
    /// # Errors
    ///
    /// Returns an error if the render cannot be started.
    pub fn start_render(
        &self,
        project: Project,
        project_file: PathBuf,
        preview_resolution: (u32, u32),
        preview_fps: u32,
    ) -> Result<(), error_stack::Report<PreviewError>> {
        self.backend
            .start_render(project, project_file, preview_resolution, preview_fps)
    }

    /// Get a rendered frame by index.
    pub fn get_frame(&self, index: usize) -> Option<RgbaImage> {
        self.backend.get_frame(index)
    }

    /// Get the current render progress.
    pub fn progress(&self) -> PreviewRenderProgress {
        self.backend.progress()
    }

    /// Cancel any in-progress render.
    pub fn cancel(&self) {
        self.backend.cancel();
    }
}
