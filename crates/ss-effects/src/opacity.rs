//! Opacity transform utility for composable clip pipelines.
//!
//! Provides two approaches to applying opacity:
//!
//! - **[`opacity`]** — A **destructive** transform that uniformly scales opacity
//!   keyframe values across a batch of [`ClipBuilder`](ss_project_builder::ClipBuilder)
//!   instances, or adds a new opacity animation to clips that lack one.
//!
//! - **[`group_opacity`]** — A **non-destructive** alternative that wraps clips
//!   in a group with its own opacity animation. The group's opacity **multiplies**
//!   into children during interpolation, preserving their original values.
//!
//! # Destructive Approach
//!
//! The [`opacity`] function transforms `Vec<ClipBuilder>` → `Vec<ClipBuilder>`, making it
//! composable in pipelines:
//!
//! ```ignore
//! let clips = opacity(ken_burns.build(), 0.5);
//! project.add_clips_at(0.0, clips);
//! ```
//!
//! This is **destructive** — once applied, the original opacity values are lost.
//! Adjusting from 50% to 75% requires rebuilding from the original effect.
//! This is a deliberate trade-off for simplicity.
//!
//! # Non-Destructive Approach
//!
//! The [`group_opacity`] function wraps clips in a group with a constant-opacity
//! animation, returning an [`ItemDef`](ss_core::ItemDef). The group's opacity
//! **multiplies** into children during interpolation, preserving their original
//! values.
//!
//! ```ignore
//! // Direct: add the group to a project
//! let group = group_opacity("fade", ken_burns.build(), 0.5)?;
//! let project = ProjectBuilder::new(params)
//!     .add_item(group)
//!     .build()?;
//! ```
//!
//! To nest the group inside another group (e.g., to apply additional transforms):
//!
//! ```ignore
//! let inner = group_opacity("dimmed", clips, 0.5)?;
//! let outer = GroupBuilder::new(GroupParams::builder()
//!     .id("slide")
//!     .children(vec![inner])
//!     .end_time(30.0)
//!     .build())
//! .add_animation(AnimBuilder::translate_x().keyframe(0.0, 0.0).keyframe(30.0, 100.0))
//! .build()?;
//! ```
//!
//! ## When to use which
//!
//! - **[`opacity`]** — Simple pipelines where you won't change the factor later.
//!   Destructive but zero-cost at render time.
//! - **[`group_opacity`]** — When you need adjustable opacity, or want to compose
//!   multiple opacity layers (e.g., clip at 80% inside a group at 50% → renders at 40%).

use ss_core::AnimatableProperty;
use ss_project_builder::{AnimBuilder, BuilderError, BuilderErrors, ClipBuilder};

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

/// Wraps clips in a group with a constant-opacity animation (non-destructive).
///
/// Unlike [`opacity`], which destructively scales keyframe values, this function
/// creates a group that applies opacity as a separate layer. The children's own
/// opacity animations are preserved — the group's opacity **multiplies** into
/// them during interpolation.
///
/// # Composability
///
/// Returns an [`ItemDef`](ss_core::ItemDef) (a group containing the built clips).
/// Add it to a project via
/// [`ProjectBuilder::add_item`](ss_project_builder::ProjectBuilder::add_item),
/// or nest it inside another group via
/// [`GroupParams::children`](ss_project_builder::GroupParams::children).
///
/// # Errors
///
/// Returns [`BuilderErrors`](ss_project_builder::BuilderErrors) if any clip
/// fails to build, or if the resulting children list is empty.
///
/// # Examples
///
/// ```ignore
/// // Direct: add group to project
/// let group = group_opacity("fade", ken_burns.build(), 0.5)?;
/// let project = ProjectBuilder::new(params)
///     .add_item(group)
///     .build()?;
///
/// // Nested: wrap in another group with additional transforms
/// let inner = group_opacity("dimmed", clips, 0.5)?;
/// let outer = GroupBuilder::new(GroupParams::builder()
///     .id("slide")
///     .children(vec![inner])
///     .end_time(30.0)
///     .build())
/// .add_animation(AnimBuilder::translate_x().keyframe(0.0, 0.0).keyframe(30.0, 100.0))
/// .build()?;
/// let project = ProjectBuilder::new(params).add_item(outer).build()?;
/// ```
pub fn group_opacity(
    id: impl Into<String>,
    clips: Vec<ClipBuilder>,
    factor: f32,
) -> Result<ss_core::ItemDef, BuilderErrors> {
    // Build all clips into ItemDefs.
    let mut errors = BuilderErrors::new();
    let mut children = Vec::with_capacity(clips.len());
    for clip in clips {
        match clip.build() {
            Ok(item) => children.push(item),
            Err(e) => errors.merge(e),
        }
    }
    errors.into_result()?;

    if children.is_empty() {
        let mut errs = BuilderErrors::new();
        errs.push(BuilderError::new(
            "group_opacity requires at least one clip",
        ));
        return Err(errs);
    }

    // Compute time range from children.
    let start_time = children
        .iter()
        .map(|c| c.start_time)
        .fold(f64::INFINITY, f64::min);
    let end_time = children
        .iter()
        .map(|c| c.end_time)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(ss_core::ItemDef {
        id: id.into(),
        content: ss_core::ItemContent::Group { children },
        track: 0,
        start_time,
        end_time,
        z_index: 0,
        sizing: ss_core::Sizing::Natural,
        pivot: [0.5, 0.5],
        animations: vec![ss_core::AnimationTrack {
            property: ss_core::AnimatableProperty::Opacity,
            keyframes: vec![ss_core::Keyframe {
                time: 0.0,
                value: factor,
                easing: ss_core::Easing::Linear,
            }],
        }],
    })
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

    // ============================================================
    // Destructive opacity tests
    // ============================================================

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

    // ============================================================
    // Non-destructive group_opacity tests
    // ============================================================

    #[test]
    fn group_opacity_wraps_clips_in_group() {
        // Given a single clip.
        let clip = test_clip("a");

        // When calling group_opacity.
        let result = group_opacity("grp", vec![clip], 0.5).unwrap();

        // Then the result is a group with correct children and content type.
        assert_eq!(result.id, "grp");
        assert!(matches!(result.content, ss_core::ItemContent::Group { .. }));
        if let ss_core::ItemContent::Group { children } = &result.content {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].id, "a");
        }
    }

    #[test]
    fn group_opacity_with_factor_half() {
        // Given a clip.
        let clip = test_clip("a");

        // When calling group_opacity with factor 0.5.
        let result = group_opacity("grp", vec![clip], 0.5).unwrap();

        // Then the group has an opacity animation with keyframe value 0.5.
        let opacity_track = result
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes.len(), 1);
        assert_eq!(opacity_track.keyframes[0].value, 0.5);
    }

    #[test]
    fn group_opacity_preserves_child_animations() {
        // Given a clip with its own opacity animation.
        let clip = clip_with_opacity("child", &[(0.0, 0.8), (5.0, 0.2)]);

        // When calling group_opacity.
        let result = group_opacity("grp", vec![clip], 0.5).unwrap();

        // Then the child's opacity animation survives untouched.
        if let ss_core::ItemContent::Group { children } = &result.content {
            let child_opacity = children[0]
                .animations
                .iter()
                .find(|a| a.property == AnimatableProperty::Opacity)
                .unwrap();
            assert_eq!(child_opacity.keyframes[0].value, 0.8);
            assert_eq!(child_opacity.keyframes[1].value, 0.2);
        } else {
            panic!("expected group");
        }
    }

    #[test]
    fn group_opacity_computes_time_range() {
        // Given two clips with different start/end times.
        let clip1 = ClipBuilder::new(
            ClipParams::builder()
                .id("a")
                .path("a.png")
                .start_time(2.0)
                .end_time(8.0)
                .build(),
        );
        let clip2 = ClipBuilder::new(
            ClipParams::builder()
                .id("b")
                .path("b.png")
                .start_time(1.0)
                .end_time(12.0)
                .build(),
        );

        // When calling group_opacity.
        let result = group_opacity("grp", vec![clip1, clip2], 0.5).unwrap();

        // Then the group time range is min start, max end.
        assert_eq!(result.start_time, 1.0);
        assert_eq!(result.end_time, 12.0);
    }

    #[test]
    fn group_opacity_with_empty_clips_returns_error() {
        // Given an empty vec of clips.
        let clips: Vec<ClipBuilder> = vec![];

        // When calling group_opacity.
        let result = group_opacity("grp", clips, 0.5);

        // Then an error is returned mentioning the requirement.
        let errors = result.expect_err("should fail with empty clips");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("at least one clip"),
            "error should mention requirement: {msg}"
        );
    }

    #[test]
    fn group_opacity_with_invalid_clip_returns_error() {
        // Given a clip with an invalid time range (end < start).
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bad")
                .path("img.png")
                .start_time(10.0)
                .end_time(5.0)
                .build(),
        );

        // When calling group_opacity.
        let result = group_opacity("grp", vec![clip], 0.5);

        // Then the error from the clip build is propagated.
        let errors = result.expect_err("should fail with invalid clip");
        let combined: String = errors.iter().map(|e| e.to_string()).collect();
        assert!(
            combined.contains("clip \"bad\""),
            "error should mention clip: {combined}"
        );
    }

    #[test]
    fn group_opacity_factor_zero() {
        // Given a clip.
        let clip = test_clip("a");

        // When calling group_opacity with factor 0.0.
        let result = group_opacity("grp", vec![clip], 0.0).unwrap();

        // Then the keyframe value is 0.0.
        let opacity_track = result
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes[0].value, 0.0);
    }

    #[test]
    fn group_opacity_factor_one() {
        // Given a clip.
        let clip = test_clip("a");

        // When calling group_opacity with factor 1.0 (identity).
        let result = group_opacity("grp", vec![clip], 1.0).unwrap();

        // Then the keyframe value is 1.0.
        let opacity_track = result
            .animations
            .iter()
            .find(|a| a.property == AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity_track.keyframes[0].value, 1.0);
    }

    #[test]
    fn group_opacity_with_multiple_clips() {
        // Given three clips.
        let clip1 = test_clip("a");
        let clip2 = test_clip("b");
        let clip3 = test_clip("c");

        // When calling group_opacity.
        let result = group_opacity("grp", vec![clip1, clip2, clip3], 0.5).unwrap();

        // Then all children are preserved in order.
        if let ss_core::ItemContent::Group { children } = &result.content {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0].id, "a");
            assert_eq!(children[1].id, "b");
            assert_eq!(children[2].id, "c");
        } else {
            panic!("expected group");
        }
    }

    #[test]
    fn group_opacity_child_with_own_opacity_multiplies_via_resolve() {
        // Given a clip with opacity 0.8 wrapped in a group at 0.5.
        let clip = clip_with_opacity("child", &[(0.0, 0.8)]);
        let group = group_opacity("grp", vec![clip], 0.5).unwrap();

        // When resolving items at any time.
        let resolved = ss_core::resolve_items(&[group], 0.0).unwrap();

        // Then the child's interpolated opacity is 0.8 * 0.5 = 0.4.
        assert_eq!(resolved.len(), 1);
        assert!(
            (resolved[0].opacity - 0.4).abs() < 0.001,
            "expected opacity 0.4, got {}",
            resolved[0].opacity
        );
    }
}
