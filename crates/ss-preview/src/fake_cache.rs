//! Fake preview cache for testing.
//!
//! Does no actual rendering. Supports inserting pre-built frames
//! and tracking method calls.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use image::RgbaImage;

use ss_core::project::Project;

use crate::errors::PreviewError;
use crate::frame_index::total_frames;
use crate::traits::{PreviewCache, RenderProgress};

/// Fake preview cache for testing.
///
/// Does no actual rendering. Supports inserting pre-built frames
/// and tracking method calls.
pub struct FakePreviewCache {
    frames: Mutex<Vec<Option<RgbaImage>>>,
    progress: Mutex<RenderProgress>,
    /// Number of times `start_render` has been called.
    pub start_render_count: AtomicUsize,
    /// Number of times `cancel` has been called.
    pub cancel_count: AtomicUsize,
    /// The last preview resolution passed to `start_render`.
    pub last_preview_resolution: Mutex<Option<(u32, u32)>>,
    /// The last preview fps passed to `start_render`.
    pub last_preview_fps: Mutex<Option<u32>>,
}

impl FakePreviewCache {
    /// Create a new fake cache with no frames.
    pub fn new() -> Self {
        Self {
            frames: Mutex::new(Vec::new()),
            progress: Mutex::new(RenderProgress {
                rendered: 0,
                total: 0,
            }),
            start_render_count: AtomicUsize::new(0),
            cancel_count: AtomicUsize::new(0),
            last_preview_resolution: Mutex::new(None),
            last_preview_fps: Mutex::new(None),
        }
    }

    /// Insert a pre-built frame at the given index.
    ///
    /// Call this after `start_render` (which sets the frame count).
    pub fn insert_frame(&self, index: usize, image: RgbaImage) {
        let mut frames = self.frames.lock().unwrap();
        if index < frames.len() {
            frames[index] = Some(image);
        }
    }

    /// Set the progress to a specific value (for testing).
    pub fn set_progress(&self, rendered: usize, total: usize) {
        let mut progress = self.progress.lock().unwrap();
        progress.rendered = rendered;
        progress.total = total;
    }
}

impl Default for FakePreviewCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewCache for FakePreviewCache {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn start_render(
        &self,
        project: Project,
        _project_file: PathBuf,
        preview_resolution: (u32, u32),
        preview_fps: u32,
    ) -> Result<(), Report<PreviewError>> {
        self.start_render_count.fetch_add(1, Ordering::SeqCst);
        let frame_count = total_frames(preview_fps, project.duration);
        *self.frames.lock().unwrap() = vec![None; frame_count];
        *self.progress.lock().unwrap() = RenderProgress {
            rendered: 0,
            total: frame_count,
        };
        *self.last_preview_resolution.lock().unwrap() = Some(preview_resolution);
        *self.last_preview_fps.lock().unwrap() = Some(preview_fps);
        Ok(())
    }

    fn get_frame(&self, index: usize) -> Option<RgbaImage> {
        self.frames
            .lock()
            .unwrap()
            .get(index)
            .and_then(|f| f.clone())
    }

    fn progress(&self) -> RenderProgress {
        *self.progress.lock().unwrap()
    }

    fn cancel(&self) {
        self.cancel_count.fetch_add(1, Ordering::SeqCst);
    }
}
