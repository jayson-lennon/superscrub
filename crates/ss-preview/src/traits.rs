//! Trait definitions for preview frame caching.
//!
//! The [`PreviewCache`] trait abstracts over cache backends, enabling
//! dependency injection for testing. [`RenderProgress`] reports rendering status.

use std::path::PathBuf;

use error_stack::Report;
use image::RgbaImage;

use ss_core::project::Project;

use crate::errors::PreviewError;

/// Progress of a background render operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderProgress {
    /// Number of frames rendered so far.
    pub rendered: usize,
    /// Total number of frames to render.
    pub total: usize,
}

impl RenderProgress {
    /// Whether all frames have been rendered.
    pub fn is_complete(&self) -> bool {
        self.rendered == self.total
    }

    /// Render progress as a fraction in [0.0, 1.0].
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            self.rendered as f64 / self.total as f64
        }
    }
}

/// Pre-renders and caches project frames for instant scrubbing and playback.
///
/// Frames are rendered on a background thread pool and stored as [`RgbaImage`]
/// in memory. The cache can serve partially-rendered results during an ongoing render.
pub trait PreviewCache: Send + Sync {
    /// Returns the name of this cache backend (for debugging).
    fn name(&self) -> &'static str;

    /// Start rendering all frames for the given project at preview settings.
    ///
    /// If a render is already in progress, it is cancelled and restarted.
    /// Frames from the previous render remain accessible until overwritten.
    ///
    /// `preview_resolution` is the resolution to render at (may differ from project output).
    /// `preview_fps` is the framerate for frame timing.
    ///
    /// # Errors
    ///
    /// Returns an error if the render cannot be started.
    fn start_render(
        &self,
        project: Project,
        project_file: PathBuf,
        preview_resolution: (u32, u32),
        preview_fps: u32,
    ) -> Result<(), Report<PreviewError>>;

    /// Get a rendered frame by index.
    ///
    /// Returns `None` if the index is out of range or not yet rendered.
    fn get_frame(&self, index: usize) -> Option<RgbaImage>;

    /// Get the current render progress.
    fn progress(&self) -> RenderProgress;

    /// Cancel any in-progress render.
    ///
    /// Already-rendered frames remain accessible.
    fn cancel(&self);
}
