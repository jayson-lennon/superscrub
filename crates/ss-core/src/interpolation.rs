//! Interpolation engine for animations.
//!
//! This module provides easing functions, keyframe interpolation, and
//! multi-track transform resolution. Given an item and a time, it produces
//! a fully resolved transform state. For groups, it recursively resolves
//! all children with composed ancestor transforms.

use tracing::trace;

use crate::animation::{AnimatableProperty, Easing, Keyframe};
use crate::item::{ItemContent, ItemDef};
use crate::transform::ResolvedItem;

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

/// Composed transform state passed through recursive resolution.
#[derive(Clone)]
struct ComposedTransform {
    translate_x: f32,
    translate_y: f32,
    scale_x: f32,
    scale_y: f32,
    rotation: f32,
    opacity: f32,
}

impl Default for ComposedTransform {
    fn default() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            opacity: 1.0,
        }
    }
}

/// Resolve the local animation tracks of an item into a `ComposedTransform`.
fn resolve_local_transform(
    item: &ItemDef,
    time: f64,
) -> Result<ComposedTransform, error_stack::Report<InterpolationError>> {
    let mut tx = ComposedTransform::default();

    trace!("resolving item: id={}, time={}", item.id, time);

    for track in &item.animations {
        let value = interpolate_keyframes(&track.keyframes, time)?;

        match track.property {
            AnimatableProperty::TranslateX => tx.translate_x = value,
            AnimatableProperty::TranslateY => tx.translate_y = value,
            AnimatableProperty::ScaleX => tx.scale_x = value,
            AnimatableProperty::ScaleY => tx.scale_y = value,
            AnimatableProperty::Rotation => tx.rotation = value,
            AnimatableProperty::Opacity => tx.opacity = value,
        }
    }

    Ok(tx)
}

/// Compose a child's local transform with its parent's transform.
fn compose_transforms(parent: &ComposedTransform, child: &ComposedTransform) -> ComposedTransform {
    ComposedTransform {
        // Translate: parent translate + child translate (scaled by parent scale).
        translate_x: parent.translate_x + child.translate_x * parent.scale_x,
        translate_y: parent.translate_y + child.translate_y * parent.scale_y,
        // Scale: multiply through.
        scale_x: parent.scale_x * child.scale_x,
        scale_y: parent.scale_y * child.scale_y,
        // Rotation: additive.
        rotation: parent.rotation + child.rotation,
        // Opacity: multiplicative.
        opacity: parent.opacity * child.opacity,
    }
}

/// Recursively resolve items, producing a flat list of `ResolvedItem`s.
fn resolve_item_recursive(
    items: &[ItemDef],
    time: f64,
    parent_transform: Option<&ComposedTransform>,
    parent_z_path: &[i32],
) -> Result<Vec<ResolvedItem>, error_stack::Report<InterpolationError>> {
    let mut results = Vec::new();

    for item in items {
        // Skip inactive items.
        if time < item.start_time || time >= item.end_time {
            continue;
        }

        let local = resolve_local_transform(item, time)?;

        // Compose with parent transform.
        let composed = match parent_transform {
            Some(parent) => compose_transforms(parent, &local),
            None => local,
        };

        // Build this item's z-path: parent path + own z_index.
        let mut z_path = parent_z_path.to_vec();
        z_path.push(item.z_index);

        match &item.content {
            ItemContent::Image { .. } => {
                results.push(ResolvedItem {
                    item: item.clone(),
                    translate: euclid::vec2(composed.translate_x, composed.translate_y),
                    scale: euclid::vec2(composed.scale_x, composed.scale_y),
                    rotation: composed.rotation,
                    opacity: composed.opacity,
                    z_path,
                });
            }
            ItemContent::Group { children } => {
                // Recurse into children with this group's composed transform and z-path.
                let child_results =
                    resolve_item_recursive(children, time, Some(&composed), &z_path)?;
                results.extend(child_results);
            }
        }
    }

    Ok(results)
}

/// Resolve all active items at a given time into a flat list of [`ResolvedItem`]s.
///
/// Recursively walks the item tree. For each active item at `time`:
/// 1. Resolve the item's own animations → local transform.
/// 2. Compose with ancestor transform (translate, scale, rotation, opacity).
/// 3. If the item is a `Group`, recurse into children.
/// 4. If the item is an `Image`, produce a `ResolvedItem`.
///
/// The result is sorted by `z_index`. Groups are atomic z-units: a group's
/// `z_index` determines the position of all its children as a block; children
/// within a group are sorted by their own `z_index`.
///
/// # Errors
///
/// Returns an error if any animation track has empty keyframes.
pub fn resolve_items(
    items: &[ItemDef],
    time: f64,
) -> Result<Vec<ResolvedItem>, error_stack::Report<InterpolationError>> {
    let mut results = resolve_item_recursive(items, time, None, &[])?;

    // Sort by z_path lexicographically. This makes groups atomic z-units:
    // all children of a group render as a contiguous block.
    results.sort_by(|a, b| a.z_path.cmp(&b.z_path));

    Ok(results)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{apply_easing, interpolate_keyframes, resolve_items};
    use crate::animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
    use crate::item::{ItemContent, Sizing};
    use crate::test_utils::fixtures::{
        build_group_item, build_image_item, build_item_with_animation, kf, kf_with_easing,
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
    // Item resolution tests
    // ============================================================

    #[test]
    fn item_with_no_animations_has_defaults() {
        // Given an item with no animations.
        let item = build_image_item("test", "img.png", 0.0, 10.0);

        // When resolving at any time.
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then all transform values are at their defaults.
        assert_eq!(resolved.len(), 1);
        let r = &resolved[0];
        assert!((r.translate.x - 0.0).abs() < 1e-5);
        assert!((r.translate.y - 0.0).abs() < 1e-5);
        assert!((r.scale.x - 1.0).abs() < 1e-5);
        assert!((r.scale.y - 1.0).abs() < 1e-5);
        assert!((r.rotation - 0.0).abs() < 1e-5);
        assert!((r.opacity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn item_with_translate_x_animation_interpolates_at_midpoint() {
        // Given an item with a translate_x animation from 0 to 100.
        let item = build_item_with_animation(
            "test",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 0.0), kf(10.0, 100.0)],
        );

        // When resolving at the midpoint.
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then translate_x is 50 (linear midpoint) and other values are defaults.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 50.0).abs() < 1e-5);
        assert!((resolved[0].translate.y - 0.0).abs() < 1e-5);
        assert!((resolved[0].scale.x - 1.0).abs() < 1e-5);
        assert!((resolved[0].opacity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn item_with_multiple_animation_tracks_resolves_each_independently() {
        // Given an item with both translate_x and opacity animations.
        let mut item = build_image_item("test", "img.png", 0.0, 10.0);
        item.animations = vec![
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
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then both animations are resolved correctly.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 100.0).abs() < 1e-5);
        assert!((resolved[0].opacity - 0.5).abs() < 1e-5);
    }

    #[test]
    fn resolve_items_preserves_original_item_definition() {
        // Given an item.
        let item = build_image_item("my-item", "photo.png", 2.0, 8.0);

        // When resolving.
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then the item definition is preserved in the result.
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].item.id, "my-item");
        assert!(matches!(
            resolved[0].item.content,
            ItemContent::Image { .. }
        ));
        assert_eq!(resolved[0].item.sizing, Sizing::default());
    }

    #[test]
    fn item_with_all_properties_animated_resolves_each_at_linear_midpoint() {
        // Given an item with all six properties animated.
        let mut item = build_image_item("test", "img.png", 0.0, 10.0);
        item.animations = vec![
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
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then all properties are at their linear midpoints.
        assert_eq!(resolved.len(), 1);
        let r = &resolved[0];
        assert!((r.translate.x - 15.0).abs() < 1e-3);
        assert!((r.translate.y - 35.0).abs() < 1e-3);
        assert!((r.scale.x - 1.5).abs() < 1e-3);
        assert!((r.scale.y - 2.0).abs() < 1e-3);
        assert!((r.rotation - std::f32::consts::PI).abs() < 1e-2);
        assert!((r.opacity - 0.5).abs() < 1e-3);
    }

    #[test]
    fn item_with_sine_in_out_easing_produces_slower_start_than_linear() {
        // Given two items: one with linear, one with SineInOut.
        let linear_item = build_item_with_animation(
            "linear",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 0.0), kf(10.0, 100.0)],
        );
        let sine_item = build_item_with_animation(
            "sine",
            AnimatableProperty::TranslateX,
            vec![
                kf_with_easing(0.0, 0.0, Easing::Linear),
                kf_with_easing(10.0, 100.0, Easing::SineInOut),
            ],
        );

        // When resolving both at t=2.5 (quarter point).
        let linear_result = resolve_items(&[linear_item], 2.5).unwrap();
        let sine_result = resolve_items(&[sine_item], 2.5).unwrap();

        // Then the SineInOut result is less than linear (slower start).
        assert!(
            sine_result[0].translate.x < linear_result[0].translate.x,
            "SineInOut ({}) should be less than linear ({})",
            sine_result[0].translate.x,
            linear_result[0].translate.x,
        );
    }

    #[test]
    fn item_with_duplicate_property_tracks_uses_last() {
        // Given an item with two translate_x tracks.
        let mut item = build_image_item("test", "img.png", 0.0, 10.0);
        item.animations = vec![
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
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then the second track's value is used (last wins).
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 499.5).abs() < 1e-3);
    }

    #[test]
    fn item_with_empty_keyframes_returns_error() {
        // Given an item with an animation track that has empty keyframes.
        let mut item = build_image_item("test", "img.png", 0.0, 10.0);
        item.animations = vec![AnimationTrack {
            property: AnimatableProperty::Opacity,
            keyframes: vec![],
        }];

        // When resolving the item.
        let result = resolve_items(&[item], 5.0);

        // Then an error is returned.
        assert!(result.is_err());
    }

    #[test]
    fn resolve_items_at_start_time_interpolates_from_keyframe_time() {
        // Given an item with an animation starting at t=2.
        let item = build_item_with_animation(
            "test",
            AnimatableProperty::TranslateX,
            vec![kf(0.0, 100.0), kf(10.0, 200.0)],
        );

        // When resolving at the item's start time.
        let resolved = resolve_items(&[item], 2.0).unwrap();

        // Then the animation is interpolated at t=2 (not clamped to item bounds).
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 120.0).abs() < 1e-3);
    }

    #[test]
    fn resolve_items_at_end_time_interpolates_from_keyframe_time() {
        // Given an item with an animation ending at value 200.
        let item = build_item_with_animation(
            "test",
            AnimatableProperty::Opacity,
            vec![kf(0.0, 0.0), kf(10.0, 1.0)],
        );

        // When resolving at the item's end time.
        let resolved = resolve_items(&[item], 8.0).unwrap();

        // Then the animation is interpolated at t=8 (not clamped to item bounds).
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].opacity - 0.8).abs() < 1e-3);
    }

    #[test]
    fn inactive_item_is_excluded_from_results() {
        // Given an item that is not active at t=15.
        let item = build_image_item("test", "img.png", 0.0, 10.0);

        // When resolving at t=15.
        let resolved = resolve_items(&[item], 15.0).unwrap();

        // Then the result is empty.
        assert!(resolved.is_empty());
    }

    #[test]
    fn multiple_items_are_sorted_by_z_index() {
        // Given two items with different z-indices (added in reverse order).
        let mut item_low = build_image_item("low", "a.png", 0.0, 10.0);
        item_low.z_index = 0;
        let mut item_high = build_image_item("high", "b.png", 0.0, 10.0);
        item_high.z_index = 10;

        // When resolving (high-z item listed first).
        let resolved = resolve_items(&[item_high, item_low], 5.0).unwrap();

        // Then the results are sorted by z_index ascending.
        assert_eq!(resolved[0].item.id, "low");
        assert_eq!(resolved[1].item.id, "high");
    }

    // ============================================================
    // Group resolution tests
    // ============================================================

    #[test]
    fn group_opacity_multiplies_into_children() {
        // Given a group at 50% opacity with a child image.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let group = build_group_item(
            "group",
            vec![child],
            vec![AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![kf(0.0, 0.5), kf(10.0, 0.5)],
            }],
        );

        // When resolving at any time.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's opacity is 0.5 (group opacity × child default opacity of 1.0).
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].opacity - 0.5).abs() < 1e-5);
    }

    #[test]
    fn nested_group_opacity_composes_multiply() {
        // Given a nested group: outer at 50%, inner at 50%, child at default 1.0.
        let child = build_image_item("leaf", "img.png", 0.0, 10.0);
        let inner_group = build_group_item(
            "inner",
            vec![child],
            vec![AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![kf(0.0, 0.5), kf(10.0, 0.5)],
            }],
        );
        let outer_group = build_group_item(
            "outer",
            vec![inner_group],
            vec![AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![kf(0.0, 0.5), kf(10.0, 0.5)],
            }],
        );

        // When resolving.
        let resolved = resolve_items(&[outer_group], 5.0).unwrap();

        // Then the leaf's opacity is 0.5 × 0.5 × 1.0 = 0.25.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].opacity - 0.25).abs() < 1e-5);
    }

    #[test]
    fn group_translate_composes_with_child_translate() {
        // Given a group with translate_x=100 and a child with translate_x=50.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let child = {
            let mut c = child;
            c.animations = vec![AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 50.0), kf(10.0, 50.0)],
            }];
            c
        };
        let group = build_group_item(
            "group",
            vec![child],
            vec![AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 100.0), kf(10.0, 100.0)],
            }],
        );

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's translate_x = 100 + 50*1.0 = 150.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 150.0).abs() < 1e-5);
    }

    #[test]
    fn group_scale_scales_child_translate() {
        // Given a group with scale_x=2.0 and a child with translate_x=100.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let child = {
            let mut c = child;
            c.animations = vec![AnimationTrack {
                property: AnimatableProperty::TranslateX,
                keyframes: vec![kf(0.0, 100.0), kf(10.0, 100.0)],
            }];
            c
        };
        let group = build_group_item(
            "group",
            vec![child],
            vec![AnimationTrack {
                property: AnimatableProperty::ScaleX,
                keyframes: vec![kf(0.0, 2.0), kf(10.0, 2.0)],
            }],
        );

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's translate_x = 0 + 100*2.0 = 200 (parent translate=0, child translate scaled by parent scale).
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].translate.x - 200.0).abs() < 1e-5);
    }

    #[test]
    fn group_rotation_adds_to_child_rotation() {
        // Given a group with rotation=PI/4 and a child with rotation=PI/4.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let child = {
            let mut c = child;
            c.animations = vec![AnimationTrack {
                property: AnimatableProperty::Rotation,
                keyframes: vec![
                    kf(0.0, std::f32::consts::FRAC_PI_4),
                    kf(10.0, std::f32::consts::FRAC_PI_4),
                ],
            }];
            c
        };
        let group = build_group_item(
            "group",
            vec![child],
            vec![AnimationTrack {
                property: AnimatableProperty::Rotation,
                keyframes: vec![
                    kf(0.0, std::f32::consts::FRAC_PI_4),
                    kf(10.0, std::f32::consts::FRAC_PI_4),
                ],
            }],
        );

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's rotation = PI/4 + PI/4 = PI/2.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn group_scale_multiplies_with_child_scale() {
        // Given a group with scale 2.0 and a child with scale 1.5.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let child = {
            let mut c = child;
            c.animations = vec![
                AnimationTrack {
                    property: AnimatableProperty::ScaleX,
                    keyframes: vec![kf(0.0, 1.5), kf(10.0, 1.5)],
                },
                AnimationTrack {
                    property: AnimatableProperty::ScaleY,
                    keyframes: vec![kf(0.0, 1.5), kf(10.0, 1.5)],
                },
            ];
            c
        };
        let group = build_group_item(
            "group",
            vec![child],
            vec![
                AnimationTrack {
                    property: AnimatableProperty::ScaleX,
                    keyframes: vec![kf(0.0, 2.0), kf(10.0, 2.0)],
                },
                AnimationTrack {
                    property: AnimatableProperty::ScaleY,
                    keyframes: vec![kf(0.0, 2.0), kf(10.0, 2.0)],
                },
            ],
        );

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's scale = 2.0 × 1.5 = 3.0.
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].scale.x - 3.0).abs() < 1e-5);
        assert!((resolved[0].scale.y - 3.0).abs() < 1e-5);
    }

    #[test]
    fn group_with_inactive_child_excludes_it() {
        // Given a group with two children, one inactive.
        let active = build_image_item("active", "a.png", 0.0, 10.0);
        let inactive = build_image_item("inactive", "b.png", 5.0, 10.0);
        let group = build_group_item("group", vec![active, inactive], vec![]);

        // When resolving at t=2.5 (only first child is active).
        let resolved = resolve_items(&[group], 2.5).unwrap();

        // Then only the active child appears.
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].item.id, "active");
    }

    #[test]
    fn group_itself_is_not_in_results_only_its_leaf_children() {
        // Given a group with one child.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let group = build_group_item("group", vec![child], vec![]);

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then only the child image appears, not the group.
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].item.id, "child");
        assert!(matches!(
            resolved[0].item.content,
            ItemContent::Image { .. }
        ));
    }

    #[test]
    fn inactive_group_excludes_all_children() {
        // Given a group that is inactive at t=15.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let group = build_group_item("group", vec![child], vec![]);
        // Group's time range: 0.0 to 10.0.

        // When resolving at t=15.
        let resolved = resolve_items(&[group], 15.0).unwrap();

        // Then no results.
        assert!(resolved.is_empty());
    }

    // ============================================================
    // Z-path and group z-ordering tests
    // ============================================================

    #[test]
    fn standalone_item_z_path_is_single_element() {
        // Given a standalone item at z_index=3.
        let mut item = build_image_item("standalone", "img.png", 0.0, 10.0);
        item.z_index = 3;

        // When resolving.
        let resolved = resolve_items(&[item], 5.0).unwrap();

        // Then the z_path is [3].
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].z_path, vec![3]);
    }

    #[test]
    fn group_child_z_path_includes_parent_z_index() {
        // Given a group at z=5 with a child at z=0.
        let child = build_image_item("child", "img.png", 0.0, 10.0);
        let mut group = build_group_item("group", vec![child], vec![]);
        group.z_index = 5;

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the child's z_path is [5, 0].
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].z_path, vec![5, 0]);
    }

    #[test]
    fn nested_group_z_path_includes_full_ancestry() {
        // Given nested groups: outer z=1, inner z=5, leaf z=0.
        let leaf = build_image_item("leaf", "img.png", 0.0, 10.0);
        let mut inner = build_group_item("inner", vec![leaf], vec![]);
        inner.z_index = 5;
        let mut outer = build_group_item("outer", vec![inner], vec![]);
        outer.z_index = 1;

        // When resolving.
        let resolved = resolve_items(&[outer], 5.0).unwrap();

        // Then the leaf's z_path is [1, 5, 0].
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].z_path, vec![1, 5, 0]);
    }

    #[test]
    fn group_children_sort_as_atomic_block_not_interleaved() {
        // Given a group at z=5 with two children (z=0, z=10) and
        // two standalone items at z=3 and z=7.
        let mut child_a = build_image_item("child_a", "a.png", 0.0, 10.0);
        child_a.z_index = 0;
        let mut child_b = build_image_item("child_b", "b.png", 0.0, 10.0);
        child_b.z_index = 10;
        let mut group = build_group_item("group", vec![child_a, child_b], vec![]);
        group.z_index = 5;

        let mut standalone_low = build_image_item("low", "c.png", 0.0, 10.0);
        standalone_low.z_index = 3;
        let mut standalone_mid = build_image_item("mid", "d.png", 0.0, 10.0);
        standalone_mid.z_index = 7;

        // When resolving.
        let resolved = resolve_items(&[group, standalone_low, standalone_mid], 5.0).unwrap();

        // Then the order is: low(3), child_a(5,0), child_b(5,10), mid(7).
        // The group's children are a contiguous block — not interleaved.
        assert_eq!(resolved.len(), 4);
        assert_eq!(resolved[0].item.id, "low");
        assert_eq!(resolved[1].item.id, "child_a");
        assert_eq!(resolved[2].item.id, "child_b");
        assert_eq!(resolved[3].item.id, "mid");
    }

    #[test]
    fn children_within_same_group_sorted_by_own_z_index() {
        // Given a group with three children at z=10, z=0, z=5 (added in that order).
        let mut child_c = build_image_item("c", "c.png", 0.0, 10.0);
        child_c.z_index = 10;
        let mut child_a = build_image_item("a", "a.png", 0.0, 10.0);
        child_a.z_index = 0;
        let mut child_b = build_image_item("b", "b.png", 0.0, 10.0);
        child_b.z_index = 5;
        let group = build_group_item("group", vec![child_c, child_a, child_b], vec![]);

        // When resolving.
        let resolved = resolve_items(&[group], 5.0).unwrap();

        // Then the children are sorted by their own z_index within the group.
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].item.id, "a");
        assert_eq!(resolved[1].item.id, "b");
        assert_eq!(resolved[2].item.id, "c");
    }

    #[test]
    fn group_with_higher_z_index_renders_on_top_of_standalone() {
        // Given a standalone item at z=2 and a group at z=5 with a child.
        let mut standalone = build_image_item("standalone", "s.png", 0.0, 10.0);
        standalone.z_index = 2;
        let child = build_image_item("child", "c.png", 0.0, 10.0);
        let mut group = build_group_item("group", vec![child], vec![]);
        group.z_index = 5;

        // When resolving.
        let resolved = resolve_items(&[group, standalone], 5.0).unwrap();

        // Then the standalone renders first, group's child renders on top.
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].item.id, "standalone");
        assert_eq!(resolved[1].item.id, "child");
    }

    #[test]
    fn group_with_lower_z_index_renders_below_standalone() {
        // Given a group at z=1 with a child and a standalone item at z=5.
        let child = build_image_item("child", "c.png", 0.0, 10.0);
        let mut group = build_group_item("group", vec![child], vec![]);
        group.z_index = 1;
        let mut standalone = build_image_item("standalone", "s.png", 0.0, 10.0);
        standalone.z_index = 5;

        // When resolving.
        let resolved = resolve_items(&[group, standalone], 5.0).unwrap();

        // Then the group's child renders first, standalone on top.
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].item.id, "child");
        assert_eq!(resolved[1].item.id, "standalone");
    }

    #[test]
    fn nested_groups_compose_z_ordering() {
        // Given outer group z=2, inner group z=8 with child z=0,
        // and a standalone item at z=5.
        let leaf = build_image_item("leaf", "l.png", 0.0, 10.0);
        let mut inner = build_group_item("inner", vec![leaf], vec![]);
        inner.z_index = 8;
        let mut outer = build_group_item("outer", vec![inner], vec![]);
        outer.z_index = 2;

        let mut standalone = build_image_item("standalone", "s.png", 0.0, 10.0);
        standalone.z_index = 5;

        // When resolving.
        let resolved = resolve_items(&[outer, standalone], 5.0).unwrap();

        // Then the nested leaf (z_path [2,8,0]) sorts before standalone (z_path [5]).
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].item.id, "leaf");
        assert_eq!(resolved[0].z_path, vec![2, 8, 0]);
        assert_eq!(resolved[1].item.id, "standalone");
        assert_eq!(resolved[1].z_path, vec![5]);
    }

    #[test]
    fn multiple_groups_at_different_z_indices_sort_correctly() {
        // Given two groups at z=1 and z=10, each with one child, plus a
        // standalone at z=5.
        let child_low = build_image_item("child_low", "cl.png", 0.0, 10.0);
        let mut group_low = build_group_item("group_low", vec![child_low], vec![]);
        group_low.z_index = 1;

        let child_high = build_image_item("child_high", "ch.png", 0.0, 10.0);
        let mut group_high = build_group_item("group_high", vec![child_high], vec![]);
        group_high.z_index = 10;

        let mut standalone = build_image_item("standalone", "s.png", 0.0, 10.0);
        standalone.z_index = 5;

        // When resolving.
        let resolved = resolve_items(&[group_high, standalone, group_low], 5.0).unwrap();

        // Then order is: group_low's child, standalone, group_high's child.
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].item.id, "child_low");
        assert_eq!(resolved[1].item.id, "standalone");
        assert_eq!(resolved[2].item.id, "child_high");
    }
}
