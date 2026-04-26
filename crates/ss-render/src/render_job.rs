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
use tracing::{debug, info, instrument};

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
    #[instrument(name = "render_job", skip_all, fields(
        start_time = %format!("{start_time:.3}"),
        end_time = %format!("{end_time:.3}"),
        fps = project.fps,
    ))]
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

        info!(total, "render started");

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
pub fn clamp_time_range(
    start: Option<f64>,
    end: Option<f64>,
    duration: std::time::Duration,
) -> (f64, f64) {
    let dur_secs = duration.as_secs_f64();
    let s = start.unwrap_or(0.0).clamp(0.0, dur_secs);
    let e = end.unwrap_or(dur_secs).clamp(0.0, dur_secs);
    (s, e.max(s)) // ensure end >= start
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::time::Duration;

    use super::{clamp_time_range, range_frame_count};

    #[rstest::rstest]
    #[case::default_range(None, None, Duration::from_secs_f64(30.0), 0.0, 30.0)]
    #[case::clamped_to_bounds(Some(-5.0), Some(100.0), Duration::from_secs_f64(30.0), 0.0, 30.0)]
    #[case::start_greater_than_end(
        Some(20.0),
        Some(10.0),
        Duration::from_secs_f64(30.0),
        20.0,
        20.0
    )]
    fn clamp_time_range_produces_correct_bounds(
        #[case] start: Option<f64>,
        #[case] end: Option<f64>,
        #[case] duration: Duration,
        #[case] expected_start: f64,
        #[case] expected_end: f64,
    ) {
        // Given start, end, and duration values.
        // When clamping the time range.
        let (s, e) = clamp_time_range(start, end, duration);

        // Then the bounds match expectations.
        assert_eq!(s, expected_start);
        assert_eq!(e, expected_end);
    }

    #[rstest::rstest]
    #[case::full_range(0.0, 10.0, 30, 300)]
    #[case::sub_range(5.0, 10.0, 30, 150)]
    #[case::start_equals_end(5.0, 5.0, 30, 0)]
    #[case::start_after_end(10.0, 5.0, 30, 0)]
    #[case::zero_fps(0.0, 10.0, 0, 0)]
    fn range_frame_count_computes_correctly(
        #[case] start: f64,
        #[case] end: f64,
        #[case] fps: u32,
        #[case] expected: usize,
    ) {
        // Given start, end, and fps values.
        // When computing the frame count.
        let count = range_frame_count(start, end, fps);

        // Then the count matches the expected value.
        assert_eq!(count, expected);
    }
}
