//! Interpolation engine for animations.
//!
//! This module provides easing functions, keyframe interpolation, and
//! multi-track transform resolution. Given a clip and a time, it produces
//! a fully resolved transform state.

use tracing::trace;

use crate::animation::{AnimatableProperty, Easing, Keyframe};
use crate::clip::ClipDef;
use crate::transform::ResolvedClip;

/// Failed during interpolation.
#[derive(Debug, wherror::Error)]
#[error("interpolation error")]
pub struct InterpolationError;

/// Apply an easing curve to a normalized progress value.
///
/// `t` must be in `[0.0, 1.0]`. Returns a value generally in `[0.0, 1.0]`.
pub fn apply_easing(t: f32, easing: Easing) -> f32 {
    match easing {
        Easing::Linear => t,
        Easing::SineInOut => -(std::f32::consts::PI * t).cos() * 0.5 + 0.5,
    }
}

/// Interpolate a value from sorted keyframes at the given time.
///
/// - Before the first keyframe: returns the first keyframe's value.
/// - After the last keyframe: returns the last keyframe's value.
/// - Between two keyframes: normalizes progress, applies easing, lerps.
///
/// # Errors
///
/// Returns an error if the keyframes list is empty.
#[allow(clippy::cast_possible_truncation)]
pub fn interpolate_keyframes(
    keyframes: &[Keyframe],
    time: f64,
) -> Result<f32, error_stack::Report<InterpolationError>> {
    if keyframes.is_empty() {
        return Err(error_stack::Report::new(InterpolationError).attach("keyframes list is empty"));
    }

    // Before or at first keyframe: hold first value.
    // If multiple keyframes share this time, the last one wins.
    if time <= keyframes[0].time {
        let mut value = keyframes[0].value;
        for kf in &keyframes[1..] {
            if kf.time <= time {
                value = kf.value;
            } else {
                break;
            }
        }
        return Ok(value);
    }

    // After or at last keyframe: hold last value.
    // SAFETY: checked non-empty above.
    #[allow(clippy::indexing_slicing)]
    let last_idx = keyframes.len() - 1;
    if time >= keyframes[last_idx].time {
        return Ok(keyframes[last_idx].value);
    }

    // Find the two keyframes surrounding `time`.
    for i in 1..keyframes.len() {
        let prev = &keyframes[i - 1];
        let curr = &keyframes[i];

        if time <= curr.time {
            let duration = curr.time - prev.time;
            if duration == 0.0 {
                // Same-time keyframes: last one wins.
                return Ok(curr.value);
            }
            let t = ((time - prev.time) / duration) as f32;
            let eased_t = apply_easing(t, curr.easing);
            let value = prev.value + (curr.value - prev.value) * eased_t;
            return Ok(value);
        }
    }

    // Should not reach here given the boundary checks above.
    #[allow(clippy::indexing_slicing)]
    Ok(keyframes[last_idx].value)
}

/// Resolve the full state of a clip at a given time.
///
/// Interpolates all animation tracks and produces a [`ResolvedClip`].
/// Properties not animated receive default values:
/// - translate: (0, 0)
/// - scale: (1, 1)
/// - rotation: 0
/// - opacity: 1.0
///
/// # Errors
///
/// Returns an error if any animation track has empty keyframes.
pub fn resolve_clip(
    clip: &ClipDef,
    time: f64,
) -> Result<ResolvedClip, error_stack::Report<InterpolationError>> {
    let mut translate_x = 0.0f32;
    let mut translate_y = 0.0f32;
    let mut scale_x = 1.0f32;
    let mut scale_y = 1.0f32;
    let mut rotation = 0.0f32;
    let mut opacity = 1.0f32;

    trace!("resolving clip: id={}, time={}", clip.id, time);

    for track in &clip.animations {
        let value = interpolate_keyframes(&track.keyframes, time)?;

        match track.property {
            AnimatableProperty::TranslateX => translate_x = value,
            AnimatableProperty::TranslateY => translate_y = value,
            AnimatableProperty::ScaleX => scale_x = value,
            AnimatableProperty::ScaleY => scale_y = value,
            AnimatableProperty::Rotation => rotation = value,
            AnimatableProperty::Opacity => opacity = value,
        }
    }

    Ok(ResolvedClip {
        clip: clip.clone(),
        translate: euclid::vec2(translate_x, translate_y),
        scale: euclid::vec2(scale_x, scale_y),
        rotation,
        opacity,
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{apply_easing, interpolate_keyframes, resolve_clip};
    use crate::animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
    use crate::clip::{ClipType, Sizing};
    use crate::test_utils::fixtures::{
        build_clip_with_animation, build_image_clip, kf, kf_with_easing,
    };

    // ============================================================
    // Easing tests
    // ============================================================

    #[rstest]
    #[case::linear_start(0.0, Easing::Linear, 0.0)]
    #[case::linear_mid(0.5, Easing::Linear, 0.5)]
    #[case::linear_end(1.0, Easing::Linear, 1.0)]
    #[case::sine_in_out_start(0.0, Easing::SineInOut, 0.0)]
    #[case::sine_in_out_end(1.0, Easing::SineInOut, 1.0)]
    fn easing_returns_correct_endpoints(
        #[case] t: f32,
        #[case] easing: Easing,
        #[case] expected: f32,
    ) {
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
    fn interpolate_between_second_and_third_keyframe_returns_correct_lerp() {
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
    fn interpolate_between_negative_values_returns_negative_midpoint() {
        // Given two keyframes interpolating between negative numbers.
        let keyframes = vec![kf(0.0, -100.0), kf(10.0, -200.0)];

        // When interpolating at the midpoint.
        let result = interpolate_keyframes(&keyframes, 5.0).unwrap();

        // Then the result is the correct negative midpoint.
        assert!((result - (-150.0)).abs() < 1e-5);
    }

    #[test]
    fn interpolate_before_first_keyframe_with_negative_time_holds_first_value() {
        // Given keyframes starting at t=0.
        let keyframes = vec![kf(0.0, 10.0), kf(10.0, 20.0)];

        // When interpolating at a negative time.
        let result = interpolate_keyframes(&keyframes, -5.0).unwrap();

        // Then the first keyframe value is held (clamped to start).
        assert!((result - 10.0).abs() < 1e-5);
    }

    #[test]
    fn interpolate_decreasing_values_returns_correct_midpoint() {
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
(0.0..=1.0).contains(&result),
            "result {result} is out of bounds"
        );
    }

    // ============================================================
    // Clip resolution tests
    // ============================================================

    #[test]
    fn clip_with_no_animations_has_defaults() {
        // Given a clip with no animations.
        let clip = build_image_clip("test", "img.png", 0.0, 10.0);

        // When resolving at any time.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then all transform values are at their defaults.
        assert!((resolved.translate.x - 0.0).abs() < 1e-5);
        assert!((resolved.translate.y - 0.0).abs() < 1e-5);
        assert!((resolved.scale.x - 1.0).abs() < 1e-5);
        assert!((resolved.scale.y - 1.0).abs() < 1e-5);
        assert!((resolved.rotation - 0.0).abs() < 1e-5);
        assert!((resolved.opacity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn clip_with_translate_x_animation_interpolates_at_midpoint() {
        // Given a clip with a translate_x animation from 0 to 100.
        let clip = build_clip_with_animation(
            "test",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 0.0), kf(10.0, 100.0)],
        );

        // When resolving at the midpoint.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then translate_x is 50 (linear midpoint) and other values are defaults.
        assert!((resolved.translate.x - 50.0).abs() < 1e-5);
        assert!((resolved.translate.y - 0.0).abs() < 1e-5);
        assert!((resolved.scale.x - 1.0).abs() < 1e-5);
        assert!((resolved.opacity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn clip_with_multiple_animation_tracks_resolves_each_independently() {
        // Given a clip with both translate_x and opacity animations.
        let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
        clip.animations = vec![
            AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, 200.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, 1.0)],
            },
        ];

        // When resolving at the midpoint.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then both animations are resolved correctly.
        assert!((resolved.translate.x - 100.0).abs() < 1e-5);
        assert!((resolved.opacity - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resolve_clip_preserves_original_clip_definition() {
        // Given a clip.
        let clip = build_image_clip("my-clip", "photo.png", 2.0, 8.0);

        // When resolving.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then the clip definition is preserved in the result.
        assert_eq!(resolved.clip.id, "my-clip");
        assert!(matches!(resolved.clip.clip_type, ClipType::Image { .. }));
        assert_eq!(resolved.clip.sizing, Sizing::default());
    }

    #[test]
    fn clip_with_all_properties_animated_resolves_each_at_linear_midpoint() {
        // Given a clip with all six properties animated.
        let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
        clip.animations = vec![
            AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 10.0), kf(10.0, 20.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::TranslateY,
                keyframes: vec![kf(0.0, 30.0), kf(10.0, 40.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::ScaleX,
                keyframes: vec![kf(0.0, 1.0), kf(10.0, 2.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::ScaleY,
                keyframes: vec![kf(0.0, 1.0), kf(10.0, 3.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::Rotation,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, std::f32::consts::TAU)],
            },
            AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, 1.0)],
            },
        ];

        // When resolving at the midpoint.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then all properties are at their linear midpoints.
        assert!((resolved.translate.x - 15.0).abs() < 1e-3);
        assert!((resolved.translate.y - 35.0).abs() < 1e-3);
        assert!((resolved.scale.x - 1.5).abs() < 1e-3);
        assert!((resolved.scale.y - 2.0).abs() < 1e-3);
        assert!((resolved.rotation - std::f32::consts::PI).abs() < 1e-2);
        assert!((resolved.opacity - 0.5).abs() < 1e-3);
    }

    #[test]
    fn clip_with_sine_in_out_easing_produces_slower_start_than_linear() {
        // Given two clips: one with linear, one with SineInOut.
        let linear_clip = build_clip_with_animation(
            "linear",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 0.0), kf(10.0, 100.0)],
        );
        let sine_clip = build_clip_with_animation(
            "sine",
            AnimatableProperty::TranslateX,
            vec![
                kf_with_easing(0.0, 0.0, Easing::Linear),
                kf_with_easing(10.0, 100.0, Easing::SineInOut),
            ],
        );

        // When resolving both at t=2.5 (quarter point).
        let linear_result = resolve_clip(&linear_clip, 2.5).unwrap();
        let sine_result = resolve_clip(&sine_clip, 2.5).unwrap();

        // Then the SineInOut result is less than linear (slower start).
        assert!(
            sine_result.translate.x < linear_result.translate.x,
            "SineInOut ({}) should be less than linear ({})",
            sine_result.translate.x,
            linear_result.translate.x,
        );
    }

    #[test]
    fn clip_with_duplicate_property_tracks_uses_last() {
        // Given a clip with two translate_x tracks.
        let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
        clip.animations = vec![
            AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, 100.0)],
            },
            AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 0.0), kf(10.0, 999.0)],
            },
        ];

        // When resolving at the midpoint.
        let resolved = resolve_clip(&clip, 5.0).unwrap();

        // Then the second track's value is used (last wins).
        assert!((resolved.translate.x - 499.5).abs() < 1e-3);
    }

    #[test]
    fn clip_with_empty_keyframes_returns_error() {
        // Given a clip with an animation track that has empty keyframes.
        let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
        clip.animations = vec![AnimationTrack {
            property: AnimatableProperty::Opacity,
            keyframes: vec![],
        }];

        // When resolving the clip.
        let result = resolve_clip(&clip, 5.0);

        // Then an error is returned.
        assert!(result.is_err());
    }

    #[test]
    fn resolve_clip_at_start_time_interpolates_from_keyframe_time() {
        // Given a clip with an animation starting at t=2.
        let clip = build_clip_with_animation(
            "test",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 100.0), kf(10.0, 200.0)],
        );

        // When resolving at the clip's start time.
        let resolved = resolve_clip(&clip, 2.0).unwrap();

        // Then the animation is interpolated at t=2 (not clamped to clip bounds).
        assert!((resolved.translate.x - 120.0).abs() < 1e-3);
    }

    #[test]
    fn resolve_clip_at_end_time_interpolates_from_keyframe_time() {
        // Given a clip with an animation ending at value 200.
        let clip = build_clip_with_animation(
            "test",
            AnimatableProperty::Opacity,
            vec![kf(0.0, 0.0), kf(10.0, 1.0)],
        );

        // When resolving at the clip's end time.
        let resolved = resolve_clip(&clip, 8.0).unwrap();

        // Then the animation is interpolated at t=8 (not clamped to clip bounds).
        assert!((resolved.opacity - 0.8).abs() < 1e-3);
    }
}
