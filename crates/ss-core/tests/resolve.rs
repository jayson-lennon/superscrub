//! Tests for multi-track clip resolution.

mod test_utils;

use ss_core::animation::{AnimatableProperty, Easing};
use ss_core::clip::{ClipType, Sizing};
use ss_core::interpolation::resolve_clip;
use test_utils::fixtures::{build_clip_with_animation, build_image_clip, kf, kf_with_easing};

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
fn clip_with_translate_x_animation() {
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
fn clip_with_multiple_tracks() {
    // Given a clip with both translate_x and opacity animations.
    let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
    clip.animations = vec![
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::TranslateX,
            keyframes: vec![kf(0.0, 0.0), kf(10.0, 200.0)],
        },
        ss_core::animation::AnimationTrack {
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
fn clip_preserves_definition() {
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
fn clip_with_all_animatable_properties() {
    // Given a clip with all six properties animated.
    let mut clip = build_image_clip("test", "img.png", 0.0, 10.0);
    clip.animations = vec![
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::TranslateX,
            keyframes: vec![kf(0.0, 10.0), kf(10.0, 20.0)],
        },
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::TranslateY,
            keyframes: vec![kf(0.0, 30.0), kf(10.0, 40.0)],
        },
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::ScaleX,
            keyframes: vec![kf(0.0, 1.0), kf(10.0, 2.0)],
        },
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::ScaleY,
            keyframes: vec![kf(0.0, 1.0), kf(10.0, 3.0)],
        },
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::Rotation,
            keyframes: vec![kf(0.0, 0.0), kf(10.0, 6.28)],
        },
        ss_core::animation::AnimationTrack {
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
    assert!((resolved.rotation - 3.14).abs() < 1e-2);
    assert!((resolved.opacity - 0.5).abs() < 1e-3);
}

#[test]
fn clip_with_sine_in_out_easing_differs_from_linear() {
    // Given two clips: one with linear, one with SineInOut.
    let linear_clip = build_clip_with_animation(
        "linear",
        AnimatableProperty::TranslateX,
        vec![kf(0.0, 0.0), kf(10.0, 100.0)],
    );
    let sine_clip = build_clip_with_animation(
        "sine",
        AnimatableProperty::TranslateX,
        vec![kf_with_easing(0.0, 0.0, Easing::Linear), kf_with_easing(10.0, 100.0, Easing::SineInOut)],
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
        ss_core::animation::AnimationTrack {
            property: AnimatableProperty::TranslateX,
            keyframes: vec![kf(0.0, 0.0), kf(10.0, 100.0)],
        },
        ss_core::animation::AnimationTrack {
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
    clip.animations = vec![ss_core::animation::AnimationTrack {
        property: AnimatableProperty::Opacity,
        keyframes: vec![],
    }];

    // When resolving the clip.
    let result = resolve_clip(&clip, 5.0);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn clip_resolved_at_clip_start_time() {
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
fn clip_resolved_at_clip_end_time() {
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
