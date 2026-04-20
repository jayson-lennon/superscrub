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
