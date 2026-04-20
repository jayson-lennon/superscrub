//! Tests for time ↔ frame index conversion functions.
//!
//! These are pure functions, easy to test exhaustively.

use rstest::rstest;
use ss_preview::{frame_index_to_time, time_to_frame_index, total_frames};

#[test]
fn time_to_frame_index_at_start() {
    // Given time=0.0, fps=30, dur=10.0.
    // When converting to frame index.
    // Then the result is 0.
    assert_eq!(time_to_frame_index(0.0, 30, 10.0), Some(0));
}

#[test]
fn time_to_frame_index_at_one_second() {
    // Given time=1.0, fps=30, dur=10.0.
    // When converting to frame index.
    // Then the result is 30.
    assert_eq!(time_to_frame_index(1.0, 30, 10.0), Some(30));
}

#[test]
fn time_to_frame_index_at_end_minus_one() {
    // Given time=9.999, fps=30, dur=10.0.
    // When converting to frame index.
    // Then the result is 299 (last frame).
    assert_eq!(time_to_frame_index(9.999, 30, 10.0), Some(299));
}

#[rstest]
#[case(-1.0)]
#[case(-0.001)]
fn time_to_frame_index_negative_returns_none(#[case] time: f64) {
    // Given a negative time.
    // When converting to frame index.
    // Then the result is None.
    assert_eq!(time_to_frame_index(time, 30, 10.0), None);
}

#[rstest]
#[case(10.0)]
#[case(15.0)]
fn time_to_frame_index_past_duration_returns_none(#[case] time: f64) {
    // Given a time >= duration.
    // When converting to frame index.
    // Then the result is None.
    assert_eq!(time_to_frame_index(time, 30, 10.0), None);
}

#[test]
fn frame_index_to_time_first_frame() {
    // Given index=0, fps=30.
    // When converting to time.
    // Then the result is 0.0.
    assert_eq!(frame_index_to_time(0, 30), 0.0);
}

#[test]
fn frame_index_to_time_frame_30() {
    // Given index=30, fps=30.
    // When converting to time.
    // Then the result is 1.0.
    assert_eq!(frame_index_to_time(30, 30), 1.0);
}

#[test]
fn total_frames_basic() {
    // Given fps=30, dur=10.0.
    // When computing total frames.
    // Then the result is 300.
    assert_eq!(total_frames(30, 10.0), 300);
}

#[test]
fn total_frames_non_integer_duration() {
    // Given fps=30, dur=10.5.
    // When computing total frames.
    // Then the result is 315.
    assert_eq!(total_frames(30, 10.5), 315);
}
