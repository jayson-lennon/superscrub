//! Render orchestration: iterates frames, renders via compositor, encodes, tracks progress.
//!
//! [`RenderJob`] is the core render loop. It is decoupled from ffmpeg specifics
//! via the [`FrameEncoder`] trait, allowing
//! testing with [`FakeFrameEncoder`](crate::fake_encoder::FakeFrameEncoder).

use std::path::Path;

use error_stack::{Report, ResultExt};
use ss_compositor::FrameRendererService;
use ss_compositor::Viewport;
use ss_core::project::Project;
use ss_preview::frame_index_to_time;
use tracing::{debug, info};

use crate::encoder::FrameEncoder;
use crate::progress::{ProgressTracker, RenderPhase};

/// Failed to render the project.
#[derive(Debug, wherror::Error)]
#[error("render failed")]
pub struct RenderError;

/// Orchestrates the frame rendering and encoding pipeline.
///
/// Owns a [`FrameRendererService`] for producing frames and delegates
/// encoding to a [`FrameEncoder`] implementation.
pub struct RenderJob {
    renderer: FrameRendererService,
}

impl RenderJob {
    /// Create a new render job with the given frame renderer.
    pub fn new(renderer: FrameRendererService) -> Self {
        Self { renderer }
    }

    /// Render all frames in the given time range and encode them.
    ///
    /// Iterates from `start_time` to `end_time` at the project's FPS,
    /// rendering each frame via the compositor and sending it to the encoder.
    /// Progress is reported via `progress_tracker`. Cancellation is checked
    /// between frames via `cancel_receiver`.
    ///
    /// # Arguments
    ///
    /// * `encoder` - The frame encoder (ffmpeg or fake)
    /// * `project` - The project to render
    /// * `project_file` - Path to the project file (for resolving relative paths)
    /// * `start_time` - Start of the time range (seconds)
    /// * `end_time` - End of the time range (seconds)
    /// * `progress_tracker` - Shared progress state
    /// * `cancel_receiver` - Channel to check for cancellation signals
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if rendering, encoding, or finalization fails,
    /// or if the render is cancelled.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        encoder: &dyn FrameEncoder,
        project: &Project,
        project_file: &Path,
        start_time: f64,
        end_time: f64,
        progress_tracker: &ProgressTracker,
        cancel_receiver: &kanal::Receiver<()>,
    ) -> Result<(), Report<RenderError>> {
        let fps = project.fps;
        let total = range_frame_count(start_time, end_time, fps);
        let viewport = Viewport::new_for_output((project.resolution[0], project.resolution[1]));

        progress_tracker.set_phase(RenderPhase::Rendering);

        info!(
            "render started: frames={}, range=({:?}, {:?})",
            total, start_time, end_time
        );

        for i in 0..total {
            // Check cancellation between frames.
            if let Ok(Some(())) = cancel_receiver.try_recv() {
                progress_tracker.set_phase(RenderPhase::Cancelled);
                return Err(Report::new(RenderError).attach("cancelled by user"));
            }

            let time = start_time + frame_index_to_time(i, fps);

            let frame = self
                .renderer
                .render(project, project_file, time, &viewport)
                .change_context(RenderError)
                .attach(format!("frame {}/{}", i + 1, total))?;

            encoder
                .send_frame(&frame)
                .change_context(RenderError)
                .attach(format!("frame {}/{}", i + 1, total))?;

            progress_tracker.update(i + 1, time);
            if (i + 1) % 10 == 0 {
                debug!("render progress: frame={}/{}", i + 1, total);
            }
        }

        encoder
            .finish()
            .change_context(RenderError)
            .attach("finalizing encoder")?;

        info!("render completed");

        Ok(())
    }
}

/// Compute the number of frames in a time range at a given FPS.
///
/// Returns `floor((end - start) * fps)`. Returns 0 if start >= end.
pub fn range_frame_count(start: f64, end: f64, fps: u32) -> usize {
    if start >= end || fps == 0 {
        return 0;
    }
    ((end - start) * fps as f64).floor() as usize
}

/// Clamp a time range to the project bounds.
///
/// Returns `(clamped_start, clamped_end)`.
pub fn clamp_time_range(start: Option<f64>, end: Option<f64>, duration: f64) -> (f64, f64) {
    let s = start.unwrap_or(0.0).clamp(0.0, duration);
    let e = end.unwrap_or(duration).clamp(0.0, duration);
    (s, e.max(s)) // ensure end >= start
}

#[cfg(test)]
mod tests {
    use super::{clamp_time_range, range_frame_count};

    #[test]
    fn default_range_is_full_duration() {
        let (start, end) = clamp_time_range(None, None, 30.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 30.0);
    }

    #[test]
    fn custom_start_end_clamped() {
        let (start, end) = clamp_time_range(Some(-5.0), Some(100.0), 30.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 30.0);
    }

    #[test]
    fn start_greater_than_end_clamped() {
        let (start, end) = clamp_time_range(Some(20.0), Some(10.0), 30.0);
        // end is clamped to max(start, end), so both become 20.0
        assert_eq!(start, 20.0);
        assert_eq!(end, 20.0);
    }

    #[test]
    fn frame_count_for_full_range() {
        // 10 seconds at 30 fps = 300 frames.
        assert_eq!(range_frame_count(0.0, 10.0, 30), 300);
    }

    #[test]
    fn frame_count_for_sub_range() {
        // 5 seconds at 30 fps = 150 frames.
        assert_eq!(range_frame_count(5.0, 10.0, 30), 150);
    }

    #[test]
    fn frame_count_zero_when_start_equals_end() {
        assert_eq!(range_frame_count(5.0, 5.0, 30), 0);
    }

    #[test]
    fn frame_count_zero_when_start_after_end() {
        assert_eq!(range_frame_count(10.0, 5.0, 30), 0);
    }

    #[test]
    fn frame_count_zero_when_fps_zero() {
        assert_eq!(range_frame_count(0.0, 10.0, 0), 0);
    }
}
