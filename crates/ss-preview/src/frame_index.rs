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

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use rstest::rstest;

    #[rstest]
    #[case(0.0, 30, 10.0, Some(0))]
    #[case(1.0, 30, 10.0, Some(30))]
    #[case(9.999, 30, 10.0, Some(299))]
    #[case(-1.0, 30, 10.0, None)]
    #[case(-0.001, 30, 10.0, None)]
    #[case(10.0, 30, 10.0, None)]
    #[case(15.0, 30, 10.0, None)]
    fn time_to_frame_index(
        #[case] time: f64,
        #[case] fps: u32,
        #[case] duration: f64,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(super::time_to_frame_index(time, fps, duration), expected);
    }

    #[rstest]
    #[case(0, 30, 0.0)]
    #[case(30, 30, 1.0)]
    fn frame_index_to_time(#[case] index: usize, #[case] fps: u32, #[case] expected: f64) {
        assert_eq!(super::frame_index_to_time(index, fps), expected);
    }

    #[rstest]
    #[case(30, 10.0, 300)]
    #[case(30, 10.5, 315)]
    fn total_frames(#[case] fps: u32, #[case] duration: f64, #[case] expected: usize) {
        assert_eq!(super::total_frames(fps, duration), expected);
    }
}
