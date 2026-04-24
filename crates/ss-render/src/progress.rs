//! Progress tracking for render operations.
//!
//! Uses [`parking_lot::Mutex`] behind a newtype for cheap cloning.
//! Designed to be shared between the render thread and the UI/caller.
//! Will be replaced with `ArcSwap` in a future phase for lock-free reads.

use std::sync::Arc;

/// Current phase of the render pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPhase {
    /// Actively rendering frames.
    Rendering,
    /// Muxing audio into the video file.
    MuxingAudio,
    /// Render completed successfully.
    Complete,
    /// Render was cancelled by the user.
    Cancelled,
}

/// Snapshot of render progress at a point in time.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    /// Current phase of the render pipeline.
    pub phase: RenderPhase,
    /// Number of frames rendered so far.
    pub frames_rendered: usize,
    /// Total number of frames to render.
    pub total_frames: usize,
    /// Time position of the most recently rendered frame.
    pub current_time: f64,
}

impl RenderProgress {
    /// Progress as a fraction in [0.0, 1.0].
    ///
    /// Returns 0.0 if total_frames is 0.
    pub fn fraction(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.frames_rendered as f64 / self.total_frames as f64
    }

    /// Whether the render has completed successfully.
    pub fn is_complete(&self) -> bool {
        self.phase == RenderPhase::Complete
    }
}

/// Thread-safe progress tracker.
///
/// Wraps `Arc<parking_lot::Mutex<RenderProgress>>` for shared access
/// between the render thread and the caller/UI.
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    inner: Arc<parking_lot::Mutex<RenderProgress>>,
}

impl ProgressTracker {
    /// Create a new tracker for the given total number of frames.
    ///
    /// Initial state: phase = Rendering, frames_rendered = 0, current_time = 0.0.
    pub fn new(total_frames: usize) -> Self {
        Self {
            inner: Arc::new(parking_lot::Mutex::new(RenderProgress {
                phase: RenderPhase::Rendering,
                frames_rendered: 0,
                total_frames,
                current_time: 0.0,
            })),
        }
    }

    /// Take a snapshot of the current progress.
    pub fn snapshot(&self) -> RenderProgress {
        self.inner.lock().clone()
    }

    /// Update the rendered frame count and current time.
    pub fn update(&self, frames_rendered: usize, current_time: f64) {
        let mut guard = self.inner.lock();
        guard.frames_rendered = frames_rendered;
        guard.current_time = current_time;
    }

    /// Set the current render phase.
    pub fn set_phase(&self, phase: RenderPhase) {
        self.inner.lock().phase = phase;
    }
}

#[cfg(test)]
mod tests {
    use super::{ProgressTracker, RenderPhase};

    #[test]
    fn new_tracker_has_zero_rendered() {
        // Given a new tracker with 100 total frames.
        let tracker = ProgressTracker::new(100);

        // Then no frames have been rendered.
        let snap = tracker.snapshot();
        assert_eq!(snap.frames_rendered, 0);
        assert_eq!(snap.total_frames, 100);
        assert_eq!(snap.phase, RenderPhase::Rendering);
    }

    #[test]
    fn update_increments_rendered_count() {
        // Given a tracker.
        let tracker = ProgressTracker::new(100);

        // When updating progress.
        tracker.update(42, 4.2);

        // Then the snapshot reflects the update.
        let snap = tracker.snapshot();
        assert_eq!(snap.frames_rendered, 42);
        assert_eq!(snap.current_time, 4.2);
    }

    #[test]
    fn set_phase_updates_phase() {
        // Given a tracker with 100 total frames.
        let tracker = ProgressTracker::new(100);

        // When setting the phase to MuxingAudio.
        tracker.set_phase(RenderPhase::MuxingAudio);

        // Then the snapshot reflects MuxingAudio.
        assert_eq!(tracker.snapshot().phase, RenderPhase::MuxingAudio);

        // When setting the phase to Complete.
        tracker.set_phase(RenderPhase::Complete);

        // Then the snapshot reflects Complete.
        assert_eq!(tracker.snapshot().phase, RenderPhase::Complete);
    }

    #[test]
    fn fraction_computes_correctly() {
        // Given a tracker with 200 total frames.
        let tracker = ProgressTracker::new(200);

        // When updating progress to 50 frames rendered.
        tracker.update(50, 1.0);

        // Then the fraction is 0.25.
        assert!((tracker.snapshot().fraction() - 0.25).abs() < 0.001);
    }

    #[test]
    fn fraction_is_zero_when_no_frames() {
        // Given a tracker with 0 total frames.
        let tracker = ProgressTracker::new(0);

        // When querying the progress fraction.
        let fraction = tracker.snapshot().fraction();

        // Then it is 0.0.
        assert_eq!(fraction, 0.0);
    }

    #[test]
    fn is_complete_when_phase_is_complete() {
        // Given a tracker in the initial Rendering phase.
        let tracker = ProgressTracker::new(10);

        // Then it is not complete.
        assert!(!tracker.snapshot().is_complete());

        // When setting the phase to Complete.
        tracker.set_phase(RenderPhase::Complete);

        // Then it is complete.
        assert!(tracker.snapshot().is_complete());
    }

    #[test]
    fn snapshot_is_a_copy() {
        // Given a tracker.
        let tracker = ProgressTracker::new(100);

        // When taking a snapshot and then updating.
        let snap1 = tracker.snapshot();
        tracker.update(50, 5.0);

        // Then the first snapshot is unchanged.
        assert_eq!(snap1.frames_rendered, 0);
        assert_eq!(tracker.snapshot().frames_rendered, 50);
    }

    #[test]
    fn cancelled_phase_is_not_complete() {
        // Given a tracker with phase set to Cancelled.
        let tracker = ProgressTracker::new(10);
        tracker.set_phase(RenderPhase::Cancelled);

        // When checking if the render is complete.
        let complete = tracker.snapshot().is_complete();

        // Then it is not considered complete.
        assert!(!complete);
    }
}
