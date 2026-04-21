//! Fake preview cache for testing.
//!
//! Does no actual rendering. Supports inserting pre-built frames
//! and tracking method calls.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use image::RgbaImage;
use tracing::trace;

use ss_core::project::Project;

use super::{PreviewCache, PreviewError, PreviewRenderProgress};
use crate::frame_index::total_frames;

/// Fake preview cache for testing.
///
/// Does no actual rendering. Supports inserting pre-built frames
/// and tracking method calls.
pub struct FakePreviewCache {
    frames: Mutex<Vec<Option<RgbaImage>>>,
    progress: Mutex<PreviewRenderProgress>,
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
            progress: Mutex::new(PreviewRenderProgress {
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
        trace!("fake cache operation: start_render");
        self.start_render_count.fetch_add(1, Ordering::SeqCst);
        let frame_count = total_frames(preview_fps, project.duration);
        *self.frames.lock().unwrap() = vec![None; frame_count];
        *self.progress.lock().unwrap() = PreviewRenderProgress {
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

    fn progress(&self) -> PreviewRenderProgress {
        *self.progress.lock().unwrap()
    }

    fn cancel(&self) {
        self.cancel_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the fake preview cache.
    //!
    //! All tests verify observable behavior through the [`PreviewCache`] trait
    //! interface using [`FakePreviewCache`]. No actual rendering is performed.

    use std::sync::atomic::Ordering;

    use image::RgbaImage;
    use ss_core::project::{EncodingConfig, Project};

    use super::FakePreviewCache;
    use super::super::PreviewCache;
    use super::super::PreviewRenderProgress;

    fn test_project() -> Project {
        Project {
            resolution: [100, 100],
            fps: 30,
            duration: 10.0,
            output: "out.mp4".into(),
            background: [0, 0, 0, 255],
            audio: None,
            encoding: EncodingConfig::default(),
            clips: vec![],
        }
    }

    #[test]
    fn name_returns_fake() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // Then the name is "fake".
        assert_eq!(cache.name(), "fake");
    }

    #[test]
    fn initial_progress_is_zero() {
        // Given a new fake cache.
        let cache = FakePreviewCache::new();

        // Then progress is (0, 0).
        let progress = cache.progress();
        assert_eq!(progress.rendered, 0);
        assert_eq!(progress.total, 0);
    }

    #[test]
    fn start_render_sets_progress_total() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // When starting a render with 30fps/10s.
        cache
            .start_render(test_project(), "project.json".into(), (100, 100), 30)
            .unwrap();

        // Then total is 300.
        assert_eq!(cache.progress().total, 300);
    }

    #[test]
    fn start_render_increments_count() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // When starting a render.
        cache
            .start_render(test_project(), "project.json".into(), (100, 100), 30)
            .unwrap();

        // Then start_render_count is 1.
        assert_eq!(cache.start_render_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn start_render_records_settings() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // When starting a render with specific settings.
        cache
            .start_render(test_project(), "project.json".into(), (640, 480), 24)
            .unwrap();

        // Then the settings are recorded.
        assert_eq!(
            *cache.last_preview_resolution.lock().unwrap(),
            Some((640, 480))
        );
        assert_eq!(*cache.last_preview_fps.lock().unwrap(), Some(24));
    }

    #[test]
    fn get_frame_returns_none_before_insert() {
        // Given a cache with a started render.
        let cache = FakePreviewCache::new();
        cache
            .start_render(test_project(), "project.json".into(), (100, 100), 30)
            .unwrap();

        // When getting frame 0 before inserting anything.
        // Then the result is None.
        assert!(cache.get_frame(0).is_none());
    }

    #[test]
    fn get_frame_returns_data_after_insert() {
        // Given a cache with a started render.
        let cache = FakePreviewCache::new();
        cache
            .start_render(test_project(), "project.json".into(), (100, 100), 30)
            .unwrap();

        // When inserting a frame at index 0.
        let image = RgbaImage::new(10, 10);
        cache.insert_frame(0, image);

        // Then get_frame(0) returns Some.
        assert!(cache.get_frame(0).is_some());
    }

    #[test]
    fn get_frame_out_of_range_returns_none() {
        // Given a cache with a started render.
        let cache = FakePreviewCache::new();
        cache
            .start_render(test_project(), "project.json".into(), (100, 100), 30)
            .unwrap();

        // When getting frame 999 (out of range).
        // Then the result is None.
        assert!(cache.get_frame(999).is_none());
    }

    #[test]
    fn cancel_increments_count() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // When cancelling.
        cache.cancel();

        // Then cancel_count is 1.
        assert_eq!(cache.cancel_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn set_progress_updates_progress() {
        // Given a fake cache.
        let cache = FakePreviewCache::new();

        // When setting progress to (50, 100).
        cache.set_progress(50, 100);

        // Then progress reports (50, 100).
        let progress = cache.progress();
        assert_eq!(progress.rendered, 50);
        assert_eq!(progress.total, 100);
    }

    #[test]
    fn progress_is_complete_when_done() {
        // Given a PreviewRenderProgress with rendered == total.
        let progress = PreviewRenderProgress {
            rendered: 10,
            total: 10,
        };

        // Then is_complete returns true.
        assert!(progress.is_complete());
    }

    #[test]
    fn progress_fraction_when_half_done() {
        // Given a PreviewRenderProgress with rendered=50, total=100.
        let progress = PreviewRenderProgress {
            rendered: 50,
            total: 100,
        };

        // Then fraction returns 0.5.
        assert_eq!(progress.fraction(), 0.5);
    }
}
