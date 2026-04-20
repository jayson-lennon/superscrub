//! Tests for easing functions and keyframe interpolation.

mod test_utils;

use rstest::rstest;
use ss_core::animation::{Easing, Keyframe};
use ss_core::interpolation::{apply_easing, interpolate_keyframes};
use test_utils::fixtures::{kf, kf_with_easing};

// ============================================================
// Easing tests
// ============================================================

#[rstest]
#[case::linear_start(0.0, Easing::Linear, 0.0)]
#[case::linear_mid(0.5, Easing::Linear, 0.5)]
#[case::linear_end(1.0, Easing::Linear, 1.0)]
#[case::sine_in_out_start(0.0, Easing::SineInOut, 0.0)]
#[case::sine_in_out_end(1.0, Easing::SineInOut, 1.0)]
fn easing_returns_correct_endpoints(#[case] t: f32, #[case] easing: Easing, #[case] expected: f32) {
    // Given a normalized progress value and an easing curve.
    // When applying the easing.
    let result = apply_easing(t, easing);

    // Then the result matches the expected value.
    assert!((result - expected).abs() < 1e-5);
}

#[test]
fn sine_in_out_midpoint_is_approximately_half() {
    // Given t = 0.5 with SineInOut easing.
    // When applying the easing.
    let result = apply_easing(0.5, Easing::SineInOut);

    // Then the result is approximately 0.5 (it's exactly 0.5 by symmetry).
    assert!((result - 0.5).abs() < 1e-5);
}

#[rstest]
#[case::linear_quarter(0.25, Easing::Linear, 0.25)]
#[case::linear_three_quarter(0.75, Easing::Linear, 0.75)]
fn linear_easing_is_identity(#[case] t: f32, #[case] easing: Easing, #[case] expected: f32) {
    // Given a t value with Linear easing.
    // When applying the easing.
    let result = apply_easing(t, easing);

    // Then the result is exactly t.
    assert!((result - expected).abs() < 1e-5);
}

#[test]
fn sine_in_out_is_symmetric() {
    // Given SineInOut easing.
    // When applying at t and 1-t.
    let a = apply_easing(0.25, Easing::SineInOut);
    let b = apply_easing(0.75, Easing::SineInOut);

    // Then the results are symmetric (a + b = 1).
    assert!((a + b - 1.0).abs() < 1e-5);
}

// ============================================================
// Keyframe interpolation tests
// ============================================================

#[test]
fn interpolate_at_start_returns_first_value() {
    // Given two keyframes [0.0 → 10.0, 10.0 → 20.0].
    let keyframes = vec![kf(0.0, 10.0), kf(10.0, 20.0)];

    // When interpolating at the start time.
    let result = interpolate_keyframes(&keyframes, 0.0).unwrap();

    // Then we get the first keyframe's value.
    assert!((result - 10.0).abs() < 1e-5);
}

#[test]
fn interpolate_at_end_returns_last_value() {
    // Given two keyframes [0.0 → 10.0, 10.0 → 20.0].
    let keyframes = vec![kf(0.0, 10.0), kf(10.0, 20.0)];

    // When interpolating at the end time.
    let result = interpolate_keyframes(&keyframes, 10.0).unwrap();

    // Then we get the last keyframe's value.
    assert!((result - 20.0).abs() < 1e-5);
}

#[test]
fn interpolate_at_midpoint_returns_lerped_value() {
    // Given two keyframes [0.0 → 0.0, 10.0 → 100.0] with linear easing.
    let keyframes = vec![kf(0.0, 0.0), kf(10.0, 100.0)];

    // When interpolating at the midpoint.
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then we get the linearly interpolated value.
    assert!((result - 50.0).abs() < 1e-5);
}

#[test]
fn interpolate_with_three_keyframes() {
    // Given three keyframes [0.0 → 0.0, 5.0 → 50.0, 10.0 → 100.0].
    let keyframes = vec![kf(0.0, 0.0), kf(5.0, 50.0), kf(10.0, 100.0)];

    // When interpolating between the second and third keyframe.
    let result = interpolate_keyframes(&keyframes, 7.5).unwrap();

    // Then we get the correct interpolated value.
    assert!((result - 75.0).abs() < 1e-5);
}

#[test]
fn interpolate_single_keyframe_always_returns_its_value() {
    // Given a single keyframe [5.0 → 42.0].
    let keyframes = vec![kf(5.0, 42.0)];

    // When interpolating at various times.
    let before = interpolate_keyframes(&keyframes, 0.0).unwrap();
    let at = interpolate_keyframes(&keyframes, 5.0).unwrap();
    let after = interpolate_keyframes(&keyframes, 100.0).unwrap();

    // Then all return the single keyframe's value.
    assert!((before - 42.0).abs() < 1e-5);
    assert!((at - 42.0).abs() < 1e-5);
    assert!((after - 42.0).abs() < 1e-5);
}

#[test]
fn interpolate_before_first_keyframe_holds_first_value() {
    // Given keyframes starting at t=5.
    let keyframes = vec![kf(5.0, 10.0), kf(10.0, 20.0)];

    // When interpolating before the first keyframe.
    let result = interpolate_keyframes(&keyframes, 2.0).unwrap();

    // Then we get the first keyframe's value.
    assert!((result - 10.0).abs() < 1e-5);
}

#[test]
fn interpolate_after_last_keyframe_holds_last_value() {
    // Given keyframes ending at t=10.
    let keyframes = vec![kf(0.0, 10.0), kf(10.0, 20.0)];

    // When interpolating after the last keyframe.
    let result = interpolate_keyframes(&keyframes, 50.0).unwrap();

    // Then we get the last keyframe's value.
    assert!((result - 20.0).abs() < 1e-5);
}

#[test]
fn interpolate_same_time_keyframes_returns_second() {
    // Given two keyframes at the same time.
    let keyframes = vec![kf(5.0, 10.0), kf(5.0, 20.0)];

    // When interpolating at that time.
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then the second keyframe's value wins.
    assert!((result - 20.0).abs() < 1e-5);
}

#[test]
fn interpolate_empty_keyframes_returns_error() {
    // Given an empty keyframes list.
    let keyframes: Vec<Keyframe> = vec![];

    // When interpolating.
    let result = interpolate_keyframes(&keyframes, 5.0);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn sine_in_out_differs_from_linear_at_quarter_point() {
    // Given two keyframes with SineInOut easing.
    let keyframes = vec![
        kf_with_easing(0.0, 0.0, Easing::Linear),
        kf_with_easing(10.0, 100.0, Easing::SineInOut),
    ];

    // When interpolating at t=2.5 (quarter point).
    let result = interpolate_keyframes(&keyframes, 2.5).unwrap();

    // Then the result differs from a simple linear interpolation (25.0).
    // SineInOut should be slower at the start.
    assert!(
        result < 25.0,
        "SineInOut at quarter should be less than linear, got {result}"
    );
}

#[test]
fn interpolate_with_negative_values() {
    // Given two keyframes interpolating between negative numbers.
    let keyframes = vec![kf(0.0, -100.0), kf(10.0, -200.0)];

    // When interpolating at the midpoint.
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then the result is the correct negative midpoint.
    assert!((result - (-150.0)).abs() < 1e-5);
}

#[test]
fn interpolate_with_negative_time() {
    // Given keyframes starting at t=0.
    let keyframes = vec![kf(0.0, 10.0), kf(10.0, 20.0)];

    // When interpolating at a negative time.
    let result = interpolate_keyframes(&keyframes, -5.0).unwrap();

    // Then the first keyframe value is held (clamped to start).
    assert!((result - 10.0).abs() < 1e-5);
}

#[test]
fn interpolate_with_decreasing_values() {
    // Given keyframes going from high to low.
    let keyframes = vec![kf(0.0, 100.0), kf(10.0, 0.0)];

    // When interpolating at the midpoint.
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then the result is 50 (correct decreasing lerp).
    assert!((result - 50.0).abs() < 1e-5);
}

#[test]
fn interpolate_three_same_time_keyframes_returns_last() {
    // Given three keyframes all at the same time.
    let keyframes = vec![kf(5.0, 10.0), kf(5.0, 20.0), kf(5.0, 30.0)];

    // When interpolating at that time.
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then the last keyframe's value wins.
    assert!((result - 30.0).abs() < 1e-5);
}

#[test]
fn interpolate_at_exact_middle_keyframe_time() {
    // Given three keyframes at t=0, t=5, t=10.
    let keyframes = vec![kf(0.0, 0.0), kf(5.0, 50.0), kf(10.0, 100.0)];

    // When interpolating at exactly t=5 (the middle keyframe time).
    let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

    // Then we get the middle keyframe's value.
    assert!((result - 50.0).abs() < 1e-5);
}

#[rstest]
#[case::near_start(0.1)]
#[case::near_end(0.9)]
fn sine_in_out_stays_bounded(#[case] t: f32) {
    // Given SineInOut easing at a point near the boundaries.
    // When applying the easing.
    let result = apply_easing(t, Easing::SineInOut);

    // Then the result stays within [0, 1].
    assert!(
        result >= 0.0 && result <= 1.0,
        "result {result} is out of bounds"
    );
}
