//! Time ↔ frame index conversion utilities.
//!
//! Pure functions for converting between time positions and frame indices.
//! These are used by both the background cache and the editor for scrubbing.

/// Convert a time in seconds to a frame index.
///
/// Returns `None` if the time is negative or past the last frame.
pub fn time_to_frame_index(time: f64, fps: u32, duration: f64) -> Option<usize> {
    if time < 0.0 || time >= duration {
        return None;
    }
    let index = (time * fps as f64).floor() as usize;
    let total = total_frames(fps, duration);
    if index >= total { None } else { Some(index) }
}

/// Convert a frame index to the time it represents.
pub fn frame_index_to_time(index: usize, fps: u32) -> f64 {
    index as f64 / fps as f64
}

/// Total number of frames for a given duration at a given fps.
pub fn total_frames(fps: u32, duration: f64) -> usize {
    (duration * fps as f64).floor() as usize
}
