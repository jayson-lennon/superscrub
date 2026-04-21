//! Test fake for [`FrameEncoder`].
//!
//! Stores raw frame data in memory for test assertions.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use image::RgbaImage;

use crate::encoder::{EncodeError, FrameEncoder};

/// Internal state for the fake encoder.
struct FakeState {
    frames: Vec<Vec<u8>>,
    finished: bool,
}

/// A fake encoder that stores frames in memory for test inspection.
///
/// Thread-safe via interior mutability. Use [`frame_count`](FakeFrameEncoder::frame_count)
/// and [`is_finished`](FakeFrameEncoder::is_finished) to assert on the encoding process.
pub struct FakeFrameEncoder {
    state: Arc<parking_lot::Mutex<FakeState>>,
    finish_called: AtomicUsize,
}

impl Default for FakeFrameEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFrameEncoder {
    /// Create a new fake encoder.
    pub fn new() -> Self {
        Self {
            state: Arc::new(parking_lot::Mutex::new(FakeState {
                frames: Vec::new(),
                finished: false,
            })),
            finish_called: AtomicUsize::new(0),
        }
    }

    /// Number of frames sent to the encoder.
    pub fn frame_count(&self) -> usize {
        self.state.lock().frames.len()
    }

    /// Whether [`finish`](FrameEncoder::finish) has been called.
    pub fn is_finished(&self) -> bool {
        self.state.lock().finished
    }

    /// Number of times [`finish`](FrameEncoder::finish) was called.
    pub fn finish_call_count(&self) -> usize {
        self.finish_called.load(Ordering::SeqCst)
    }
}

impl FrameEncoder for FakeFrameEncoder {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn send_frame(&self, frame: &RgbaImage) -> Result<(), Report<EncodeError>> {
        self.state.lock().frames.push(frame.as_raw().clone());
        Ok(())
    }

    fn finish(&self) -> Result<(), Report<EncodeError>> {
        self.finish_called.fetch_add(1, Ordering::SeqCst);
        self.state.lock().finished = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::FakeFrameEncoder;
    use crate::encoder::FrameEncoder;

    #[test]
    fn name_returns_fake() {
        let encoder = FakeFrameEncoder::new();
        assert_eq!(encoder.name(), "fake");
    }

    #[test]
    fn send_frame_stores_frame() {
        // Given a fake encoder.
        let encoder = FakeFrameEncoder::new();

        // When sending a frame.
        let frame = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        encoder.send_frame(&frame).unwrap();

        // Then the frame count is 1.
        assert_eq!(encoder.frame_count(), 1);
    }

    #[test]
    fn frame_count_equals_number_of_sent_frames() {
        let encoder = FakeFrameEncoder::new();
        let frame = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));

        encoder.send_frame(&frame).unwrap();
        encoder.send_frame(&frame).unwrap();
        encoder.send_frame(&frame).unwrap();

        assert_eq!(encoder.frame_count(), 3);
    }

    #[test]
    fn finish_sets_finished_flag() {
        let encoder = FakeFrameEncoder::new();

        assert!(!encoder.is_finished());
        encoder.finish().unwrap();
        assert!(encoder.is_finished());
    }

    #[test]
    fn finish_call_count_increments() {
        let encoder = FakeFrameEncoder::new();

        assert_eq!(encoder.finish_call_count(), 0);
        encoder.finish().unwrap();
        assert_eq!(encoder.finish_call_count(), 1);
    }
}
