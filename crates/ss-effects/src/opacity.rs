//! Opacity transform utility for composable clip pipelines.
//!
//! Provides the [`opacity`] free function that uniformly scales opacity keyframe
//! values across a batch of [`ClipBuilder`](ss_project_builder::ClipBuilder)
//! instances, or adds a new opacity animation to clips that lack one.
//!
//! # Composability
//!
//! The function transforms `Vec<ClipBuilder>` → `Vec<ClipBuilder>`, making it
//! composable in pipelines:
//!
//! ```ignore
//! let clips = opacity(ken_burns.build(), 0.5);
//! project.add_clips_at(0.0, clips);
//! ```
//!
//! # Destructive Nature
//!
//! This is a **destructive** transform — once applied, the original opacity values
//! are lost. Adjusting from 50% to 75% requires rebuilding from the original
//! effect. This is a deliberate trade-off for simplicity: the function takes
//! ownership of clips and returns transformed clips, with no wrapper types or
//! deferred evaluation.
//!
//! # Future: Clip Groups
//!
//! A future `ClipGroup` concept could track related clips as a unit, enabling
//! **non-destructive** property overrides that compose on top of individual clip
//! values rather than replacing them:
//!
//! ```ignore
//! // Hypothetical future API
//! let group = ClipGroup::from(ken_burns.build())
//!     .with_opacity(0.5)           // non-destructive layer
//!     .with_time_offset(5.0);      // non-destructive layer
//!
//! // Adjusting later is cheap — just change the override
//! group.set_opacity(0.75);
//! ```
//!
//! Groups would enable:
//! - **Non-destructive transforms** — override properties without losing originals
//! - **Nested effects** — groups within groups, each with its own overrides
//! - **Re-ordering** — swap transform order without rebuilding
//! - **Deferred evaluation** — resolve final values only when building the project
//!
//! This would require changes to `ProjectBuilder` and potentially `ss-core` to
//! support a group abstraction. For now, the destructive free-function approach
//! keeps the implementation simple while the API surface is still small.

use ss_core::AnimatableProperty;
use ss_project_builder::{AnimBuilder, ClipBuilder};

/// Uniformly scales opacity across a batch of clips.
///
/// For each clip in `clips`:
/// - If the clip already has an opacity animation, all keyframe values are
///   multiplied by `factor`.
/// - If the clip has no opacity animation, a new one is added with a single
///   keyframe at time 0 holding `factor`.
/// - All non-opacity animations are left untouched.
///
/// Takes ownership of `clips` and returns transformed clips — composable in
/// pipelines like `opacity(ken_burns.build(), 0.5)`.
///
/// # Panics
///
/// Does not panic, but note that `factor` values outside `[0.0, 1.0]` will
/// produce opacity values outside the normal range. The caller is responsible
/// for ensuring valid inputs.
///
/// # Examples
///
/// ```ignore
/// use ss_effects::opacity;
///
/// // Dim all clips to 50%
/// let clips = opacity(ken_burns.build(), 0.5);
/// ```
pub fn opacity(clips: Vec<ClipBuilder>, factor: f32) -> Vec<ClipBuilder> {
    clips
        .into_iter()
        .map(|clip| transform_clip_opacity(clip, factor))
        .collect()
}

/// Transforms a single clip's opacity by scaling existing keyframes or adding
/// a new opacity animation.
fn transform_clip_opacity(clip: ClipBuilder, factor: f32) -> ClipBuilder {
    let (params, anims) = clip.into_parts();

    let opacity_idx = anims
        .iter()
        .position(|a| a.property() == AnimatableProperty::Opacity);

    let anims = match opacity_idx {
        Some(idx) => {
            // Scale existing opacity keyframe values.
            anims
                .into_iter()
                .enumerate()
                .map(|(i, a)| {
                    if i == idx {
                        a.with_scaled_values(factor)
                    } else {
                        a
                    }
                })
                .collect()
        }
        None => {
            // Add a new opacity animation at the given factor.
            let mut anims = anims;
            anims.push(AnimBuilder::opacity().keyframe(0.0, factor));
            anims
        }
    };

    ClipBuilder::from_parts(params, anims)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use ss_project_builder::ClipParams;

    /// Creates a minimal `ClipBuilder` for testing.
    fn test_clip(id: &str) -> ClipBuilder {
        ClipBuilder::new(
            ClipParams::builder()
                .id(id)
                .path("img.png")
                .end_time(10.0)
                .build(),
        )
    }

    /// Creates a `ClipBuilder` with an opacity animation holding given values.
    fn clip_with_opacity(id: &str, keyframes: &[(f64, f32)]) -> ClipBuilder {
        let mut builder = test_clip(id);
        let mut anim = AnimBuilder::opacity();
        for (t, v) in keyframes {
            anim = anim.keyframe(*t, *v);
        }
        builder = builder.add_animation(anim);
        builder
    }

    #[test]
    fn scales_existing_opacity_keyframes() {
        // Given a clip with opacity keyframes at 1.0 and 0.0.
        let clip = clip_with_opacity("a", &[(0.0, 1.0), (5.0, 0.0)]);

        // When applying opacity at factor 0.5.
        let result = opacity(vec![clip], 0.5);

        // Then values are halved.
        let built = result.into_iter().next().unwrap().build().unwrap();
        let opacity_track = built
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes[0].value, 0.5);
        assert_eq!(opacity_track.keyframes[1].value, 0.0);
    }

    #[test]
    fn adds_opacity_to_clip_without_it() {
        // Given a clip with no opacity animation.
        let clip = test_clip("no_opacity");

        // When applying opacity at factor 0.8.
        let result = opacity(vec![clip], 0.8);

        // Then a new opacity animation is added.
        let built = result.into_iter().next().unwrap().build().unwrap();
        let opacity_track = built
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes.len(), 1);
        assert_eq!(opacity_track.keyframes[0].value, 0.8);
    }

    #[test]
    fn factor_zero_produces_zero_opacity() {
        // Given a clip with opacity at 1.0.
        let clip = clip_with_opacity("zero", &[(0.0, 1.0), (3.0, 0.5)]);

        // When applying opacity at factor 0.0.
        let result = opacity(vec![clip], 0.0);

        // Then all values are 0.0.
        let built = result.into_iter().next().unwrap().build().unwrap();
        let opacity_track = built
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes[0].value, 0.0);
        assert_eq!(opacity_track.keyframes[1].value, 0.0);
    }

    #[test]
    fn factor_one_is_identity_for_existing_opacity() {
        // Given a clip with opacity keyframes.
        let clip = clip_with_opacity("identity", &[(0.0, 0.7), (4.0, 0.3)]);

        // When applying opacity at factor 1.0.
        let result = opacity(vec![clip], 1.0);

        // Then values are unchanged.
        let built = result.into_iter().next().unwrap().build().unwrap();
        let opacity_track = built
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes[0].value, 0.7);
        assert_eq!(opacity_track.keyframes[1].value, 0.3);
    }

    #[test]
    fn non_opacity_animations_untouched() {
        // Given a clip with translate_x and opacity.
        let clip = clip_with_opacity("mixed", &[(0.0, 1.0)]).add_animation(
            AnimBuilder::translate_x()
                .keyframe(0.0, 50.0)
                .keyframe(5.0, 200.0),
        );

        // When applying opacity at factor 0.5.
        let result = opacity(vec![clip], 0.5);

        // Then translate_x keyframes are unchanged.
        let built = result.into_iter().next().unwrap().build().unwrap();
        let tx_track = built
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::TranslateX)
            .unwrap();
        assert_eq!(tx_track.keyframes[0].value, 50.0);
        assert_eq!(tx_track.keyframes[1].value, 200.0);
    }

    #[test]
    fn multiple_clips_all_transformed() {
        // Given three clips with different opacity configurations.
        let clip_a = clip_with_opacity("a", &[(0.0, 1.0)]);
        let clip_b = clip_with_opacity("b", &[(0.0, 0.8), (5.0, 0.0)]);
        let clip_c = test_clip("c"); // no opacity

        // When applying opacity at factor 0.5.
        let result = opacity(vec![clip_a, clip_b, clip_c], 0.5);

        // Then all clips are transformed.
        assert_eq!(result.len(), 3);

        let built: Vec<_> = result.into_iter().map(|c| c.build().unwrap()).collect();

        // Clip A: 1.0 * 0.5 = 0.5
        let a_opacity = built[0]
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(a_opacity.keyframes[0].value, 0.5);

        // Clip B: 0.8 * 0.5 = 0.4, 0.0 * 0.5 = 0.0
        let b_opacity = built[1]
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(b_opacity.keyframes[0].value, 0.4);
        assert_eq!(b_opacity.keyframes[1].value, 0.0);

        // Clip C: gets new opacity at 0.5
        let c_opacity = built[2]
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(c_opacity.keyframes[0].value, 0.5);
    }

    #[test]
    fn empty_clips_returns_empty() {
        // Given an empty vec.
        let clips: Vec<ClipBuilder> = vec![];

        // When applying opacity.
        let result = opacity(clips, 0.5);

        // Then the result is empty.
        assert!(result.is_empty());
    }
}
