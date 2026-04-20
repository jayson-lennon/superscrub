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
