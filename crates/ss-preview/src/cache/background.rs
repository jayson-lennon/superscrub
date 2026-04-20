//! Background frame pre-renderer using rayon for parallel rendering.
//!
//! Renders all frames of a project on a rayon thread pool and stores
//! [`RgbaImage`] frames in memory for instant access during scrubbing/playback.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use error_stack::Report;
use image::RgbaImage;
use rayon::prelude::*;
use ss_compositor::{FrameRenderer, Viewport};
use ss_core::project::Project;
use tracing::{info, warn};

use crate::cache::PreviewError;
use crate::cache::{PreviewCache, PreviewRenderProgress};
use crate::frame_index::{frame_index_to_time, total_frames};

/// Per-render shared state. Created fresh for each `start_render` call.
struct RenderState {
    /// Rendered frames, indexed by frame number.
    frames: Mutex<Vec<Option<RgbaImage>>>,
    /// Number of frames rendered so far.
    rendered_count: AtomicUsize,
    /// Total number of frames to render.
    total: usize,
    /// Set to true to cancel this render.
    cancel: AtomicBool,
}

/// Background frame pre-renderer.
///
/// Renders all frames of a project on a rayon thread pool and stores
/// [`RgbaImage`] frames in memory for instant access during scrubbing/playback.
pub struct BackgroundPreviewCache {
    renderer: Arc<dyn FrameRenderer>,
    /// The active render state. `None` when no render has been started.
    render: Mutex<Option<Arc<RenderState>>>,
}

impl BackgroundPreviewCache {
    /// Create a new cache backed by the given frame renderer.
    pub fn new(renderer: Arc<dyn FrameRenderer>) -> Self {
        Self {
            renderer,
            render: Mutex::new(None),
        }
    }
}

impl PreviewCache for BackgroundPreviewCache {
    fn name(&self) -> &'static str {
        "background"
    }

    fn start_render(
        &self,
        project: Project,
        project_file: PathBuf,
        preview_resolution: (u32, u32),
        preview_fps: u32,
    ) -> Result<(), Report<PreviewError>> {
        let frame_count = total_frames(preview_fps, project.duration);
        let viewport = Viewport::new_for_output(preview_resolution);

        // Cancel any previous render.
        {
            let guard = self.render.lock().unwrap();
            if let Some(prev) = guard.as_ref() {
                prev.cancel.store(true, Ordering::SeqCst);
            }
        }

        let render_state = Arc::new(RenderState {
            frames: Mutex::new(vec![None; frame_count]),
            rendered_count: AtomicUsize::new(0),
            total: frame_count,
            cancel: AtomicBool::new(false),
        });

        // Store the new render state (replaces previous).
        *self.render.lock().unwrap() = Some(render_state.clone());

        info!(
            "preview render started: frames={}, resolution={:?}",
            frame_count, preview_resolution
        );

        // Spawn a thread that drives rayon parallel rendering.
        let renderer = self.renderer.clone();
        let rs = render_state;

        std::thread::spawn(move || {
            (0..frame_count).into_par_iter().for_each(|index| {
                if rs.cancel.load(Ordering::Relaxed) {
                    return;
                }

                let time = frame_index_to_time(index, preview_fps);
                match renderer.render(&project, &project_file, time, &viewport) {
                    Ok(rgba_image) => {
                        rs.frames.lock().unwrap()[index] = Some(rgba_image);
                        rs.rendered_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_e) => {
                        warn!("frame render failed: index={}", index);
                    }
                }
            });
            if !rs.cancel.load(Ordering::Relaxed) {
                info!("preview render completed");
            } else {
                info!("preview render cancelled");
            }
        });

        Ok(())
    }

    fn get_frame(&self, index: usize) -> Option<RgbaImage> {
        let guard = self.render.lock().unwrap();
        guard
            .as_ref()
            .and_then(|rs| rs.frames.lock().unwrap().get(index).and_then(|f| f.clone()))
    }

    fn progress(&self) -> PreviewRenderProgress {
        let guard = self.render.lock().unwrap();
        match guard.as_ref() {
            None => PreviewRenderProgress {
                rendered: 0,
                total: 0,
            },
            Some(rs) => PreviewRenderProgress {
                rendered: rs.rendered_count.load(Ordering::Relaxed),
                total: rs.total,
            },
        }
    }

    fn cancel(&self) {
        let guard = self.render.lock().unwrap();
        if let Some(rs) = guard.as_ref() {
            rs.cancel.store(true, Ordering::SeqCst);
        }
    }
}
