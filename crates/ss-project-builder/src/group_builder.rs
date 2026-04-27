//! Group construction with child items and animations.
//!
//! Provides [`GroupParams`] (a `bon`-generated typestate builder for compile-time
//! enforcement of required fields) and [`GroupBuilder`] (which wraps a fully-built
//! `GroupParams` and adds animation accumulation and runtime validation).
//!
//! Also provides [`group_from_clips`] as a convenience for wrapping pre-built
//! [`ClipBuilder`](crate::ClipBuilder) instances into a group with auto-computed
//! time range.
//!
//! # Usage
//!
//! ```ignore
//! use ss_project_builder::{GroupBuilder, GroupParams, ClipBuilder, ClipParams};
//!
//! // Manual construction with pre-built children.
//! let child = ClipBuilder::new(
//!     ClipParams::builder().id("bg").path("bg.png").end_time(10.0).build()
//! ).build().unwrap();
//!
//! let group = GroupBuilder::new(
//!     GroupParams::builder()
//!         .id("group-1")
//!         .children(vec![child])
//!         .end_time(10.0)
//!         .build()
//! )
//! .add_animation(AnimBuilder::opacity().keyframe(0.0, 0.0).keyframe(3.0, 1.0))
//! .build()
//! .unwrap();
//! ```

use ss_core::{ItemContent, ItemDef};

use crate::{AnimBuilder, BuilderError, BuilderErrors, ClipBuilder};

/// Parameters managed by `bon`'s typestate builder.
///
/// Required fields (`id`, `children`, `end_time`) are enforced at compile time —
/// you cannot call `.build()` without setting them. All other fields have
/// sensible defaults.
///
/// Children must be pre-built [`ItemDef`] instances (already validated).
#[derive(Debug, bon::Builder)]
#[builder(on(String, into))]
pub struct GroupParams {
    /// Unique identifier for the group.
    pub id: String,
    /// Pre-built child items (already validated).
    pub children: Vec<ItemDef>,
    /// Track lane index.
    #[builder(default = 0)]
    pub track: u32,
    /// Start time in seconds.
    #[builder(default = 0.0)]
    pub start_time: f64,
    /// End time in seconds.
    pub end_time: f64,
    /// Z-order for rendering. Higher values render on top.
    #[builder(default = 0)]
    pub z_index: i32,
    /// Pivot point [0..1] relative to group bounds.
    #[builder(default = [0.5, 0.5])]
    pub pivot: [f32; 2],
}

/// Builder for constructing a group [`ItemDef`] with optional animations.
///
/// Wraps a fully-built [`GroupParams`] (enforced by `bon` at compile time)
/// and adds animation accumulation via [`add_animation`](GroupBuilder::add_animation).
///
/// Call [`build`](GroupBuilder::build) to validate and produce an `ItemDef`
/// with [`ItemContent::Group`].
pub struct GroupBuilder {
    params: GroupParams,
    anim_builders: Vec<AnimBuilder>,
}

impl GroupBuilder {
    /// Creates a new `GroupBuilder` from fully-built [`GroupParams`].
    ///
    /// Use `GroupParams::builder().id(...).children(...).end_time(...).build()` to
    /// construct the params — `bon` enforces required fields at compile time.
    pub fn new(params: GroupParams) -> Self {
        Self {
            params,
            anim_builders: vec![],
        }
    }

    /// Appends an [`AnimBuilder`] to the group's animation list.
    ///
    /// Animations are built and validated when [`build`](GroupBuilder::build) is called.
    #[must_use]
    pub fn add_animation(mut self, anim: AnimBuilder) -> Self {
        self.anim_builders.push(anim);
        self
    }

    /// Returns a new `GroupBuilder` shifted by `offset` seconds on the timeline.
    ///
    /// Shifts the group's `start_time`, `end_time`, and all animation keyframe times,
    /// **and** recursively shifts all children's time ranges and keyframes using
    /// [`ItemDef::with_time_offset`].
    #[must_use]
    pub fn with_offset(self, offset: f64) -> Self {
        let mut params = self.params;
        params.start_time += offset;
        params.end_time += offset;
        params.children = params
            .children
            .into_iter()
            .map(|c| c.with_time_offset(offset))
            .collect();
        let shifted_anims = self
            .anim_builders
            .into_iter()
            .map(|ab| ab.with_time_offset(offset))
            .collect();
        Self {
            params,
            anim_builders: shifted_anims,
        }
    }

    /// Returns read-only access to the group params.
    ///
    /// Useful for inspecting group metadata without deconstructing.
    pub fn params(&self) -> &GroupParams {
        &self.params
    }

    /// Consumes the builder, validates, and returns a group [`ItemDef`].
    ///
    /// # Validation
    ///
    /// - `end_time` must be greater than `start_time`
    /// - Children must not be empty
    /// - All animations must have at least one keyframe
    /// - Each child's `end_time` must not exceed the group's `end_time`
    ///
    /// All errors are collected into a single [`BuilderErrors`] rather than
    /// failing on the first issue.
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if any validation rules are violated.
    pub fn build(self) -> Result<ItemDef, BuilderErrors> {
        let mut errors = BuilderErrors::new();
        let group_id = &self.params.id;
        let group_ctx = format!("group \"{group_id}\"");

        // Validate end_time > start_time.
        if self.params.end_time <= self.params.start_time {
            errors.push(
                BuilderError::new("end_time must be greater than start_time")
                    .in_context(&group_ctx),
            );
        }

        // Validate non-empty children.
        if self.params.children.is_empty() {
            errors.push(
                BuilderError::new("group must have at least one child").in_context(&group_ctx),
            );
        }

        // Validate children fit within group time range.
        for child in &self.params.children {
            if child.end_time > self.params.end_time {
                errors.push(
                    BuilderError::new(format!(
                        "child \"{}\" end_time ({}) exceeds group end_time ({})",
                        child.id, child.end_time, self.params.end_time
                    ))
                    .in_context(&group_ctx),
                );
            }
        }

        // Build all animations, then validate keyframes.
        let animations: Vec<_> = self
            .anim_builders
            .into_iter()
            .map(|ab| ab.build())
            .collect();

        for track in &animations {
            if track.keyframes.is_empty() {
                let prop_name = crate::clip_builder::property_display_name(track.property);
                errors.push(
                    BuilderError::new("animation has no keyframes")
                        .in_context(format!("animation \"{prop_name}\""))
                        .in_context(&group_ctx),
                );
            }
        }

        errors.into_result()?;

        Ok(ItemDef {
            id: self.params.id,
            content: ItemContent::Group {
                children: self.params.children,
            },
            track: self.params.track,
            start_time: self.params.start_time,
            end_time: self.params.end_time,
            z_index: self.params.z_index,
            sizing: ss_core::Sizing::Natural,
            pivot: self.params.pivot,
            animations,
        })
    }
}

/// Convenience function that builds clips and wraps them into a [`GroupParams`].
///
/// Computes `start_time` as the minimum of all children's start times and
/// `end_time` as the maximum of all children's end times. This is the common
/// pattern for wrapping clips into a group for non-destructive effects.
///
/// # Errors
///
/// Returns `Err(BuilderErrors)` if any clip builder fails to build, or if
/// the resulting children list is empty.
pub fn group_from_clips(
    id: impl Into<String>,
    clips: Vec<ClipBuilder>,
) -> Result<GroupParams, BuilderErrors> {
    let mut errors = BuilderErrors::new();
    let mut children = Vec::with_capacity(clips.len());

    for clip_builder in clips {
        match clip_builder.build() {
            Ok(item) => children.push(item),
            Err(e) => errors.merge(e),
        }
    }

    errors.into_result()?;

    if children.is_empty() {
        let mut errs = BuilderErrors::new();
        errs.push(BuilderError::new(
            "group_from_clips requires at least one clip",
        ));
        return Err(errs);
    }

    let start_time = children
        .iter()
        .map(|c| c.start_time)
        .fold(f64::INFINITY, f64::min);
    let end_time = children
        .iter()
        .map(|c| c.end_time)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(GroupParams {
        id: id.into(),
        children,
        track: 0,
        start_time,
        end_time,
        z_index: 0,
        pivot: [0.5, 0.5],
    })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::ClipParams;
    use ss_core::{AnimatableProperty, ItemContent};

    /// Creates a minimal image `ItemDef` for test reuse.
    fn image_item(id: &str, start: f64, end: f64) -> ItemDef {
        ItemDef {
            id: id.to_string(),
            content: ItemContent::Image {
                path: std::path::PathBuf::from("test.png"),
            },
            track: 0,
            start_time: start,
            end_time: end,
            z_index: 0,
            sizing: ss_core::Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        }
    }

    #[test]
    fn build_minimal_group_with_required_fields() {
        // Given a GroupBuilder with only required fields.
        let params = GroupParams::builder()
            .id("grp")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .end_time(5.0)
            .build();

        // When building with no animations.
        let item = GroupBuilder::new(params).build().unwrap();

        // Then defaults are applied correctly.
        assert_eq!(item.id, "grp");
        assert!(matches!(item.content, ItemContent::Group { .. }));
        assert_eq!(item.track, 0);
        assert_eq!(item.start_time, 0.0);
        assert_eq!(item.end_time, 5.0);
        assert_eq!(item.z_index, 0);
        assert_eq!(item.pivot, [0.5, 0.5]);
        assert!(item.animations.is_empty());
    }

    #[test]
    fn build_group_with_all_optional_fields() {
        // Given a GroupBuilder with all optional fields set.
        let params = GroupParams::builder()
            .id("grp")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .track(2)
            .start_time(1.0)
            .end_time(15.0)
            .z_index(10)
            .pivot([0.0, 1.0])
            .build();

        // When building.
        let item = GroupBuilder::new(params).build().unwrap();

        // Then all values are reflected in the output.
        assert_eq!(item.track, 2);
        assert_eq!(item.start_time, 1.0);
        assert_eq!(item.end_time, 15.0);
        assert_eq!(item.z_index, 10);
        assert_eq!(item.pivot, [0.0, 1.0]);
    }

    #[test]
    fn build_group_with_animations() {
        // Given a GroupBuilder with two animations.
        let params = GroupParams::builder()
            .id("grp")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .end_time(5.0)
            .build();
        let anim1 = AnimBuilder::opacity().keyframe(0.0, 0.0).keyframe(3.0, 1.0);
        let anim2 = AnimBuilder::scale_x().keyframe(0.0, 1.0);

        // When building.
        let item = GroupBuilder::new(params)
            .add_animation(anim1)
            .add_animation(anim2)
            .build()
            .unwrap();

        // Then both animations are present.
        assert_eq!(item.animations.len(), 2);
        assert_eq!(item.animations[0].property, AnimatableProperty::Opacity);
        assert_eq!(item.animations[1].property, AnimatableProperty::ScaleX);
    }

    #[test]
    fn build_group_with_empty_children_produces_error() {
        // Given a GroupBuilder with no children.
        let params = GroupParams::builder()
            .id("empty")
            .children(vec![])
            .end_time(5.0)
            .build();

        // When building.
        let result = GroupBuilder::new(params).build();

        // Then the error mentions empty children.
        let errors = result.expect_err("should fail with empty children");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("group \"empty\""),
            "error should mention group: {msg}"
        );
        assert!(
            msg.contains("at least one child"),
            "error should describe empty children: {msg}"
        );
    }

    #[test]
    fn build_group_with_invalid_time_range() {
        // Given a GroupBuilder where end_time <= start_time.
        let params = GroupParams::builder()
            .id("bad-time")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .start_time(10.0)
            .end_time(5.0)
            .build();

        // When building.
        let result = GroupBuilder::new(params).build();

        // Then the error mentions the time range.
        let errors = result.expect_err("should fail with invalid time range");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("end_time must be greater than start_time"),
            "error should describe time issue: {msg}"
        );
    }

    #[test]
    fn build_group_with_child_exceeding_end_time() {
        // Given a GroupBuilder where a child's end_time exceeds the group's.
        let params = GroupParams::builder()
            .id("overflow")
            .children(vec![image_item("c1", 0.0, 15.0)]) // child ends at 15
            .end_time(10.0) // group ends at 10
            .build();

        // When building.
        let result = GroupBuilder::new(params).build();

        // Then the error mentions the child exceeding group time.
        let errors = result.expect_err("should fail with child exceeding group time");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("child \"c1\""),
            "error should mention child: {msg}"
        );
        assert!(
            msg.contains("exceeds group end_time"),
            "error should describe overflow: {msg}"
        );
    }

    #[test]
    fn build_group_with_empty_animation_produces_error() {
        // Given a GroupBuilder with an animation that has no keyframes.
        let params = GroupParams::builder()
            .id("grp")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .end_time(5.0)
            .build();
        let anim = AnimBuilder::opacity();

        // When building.
        let result = GroupBuilder::new(params).add_animation(anim).build();

        // Then the error has correct path context.
        let errors = result.expect_err("should fail with empty animation");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("group \"grp\""),
            "error should mention group: {msg}"
        );
        assert!(
            msg.contains("animation \"opacity\""),
            "error should mention property: {msg}"
        );
        assert!(
            msg.contains("no keyframes"),
            "error should describe issue: {msg}"
        );
    }

    #[test]
    fn build_group_collects_multiple_errors() {
        // Given a GroupBuilder with both invalid time range and empty animation.
        let params = GroupParams::builder()
            .id("multi")
            .children(vec![]) // empty children
            .start_time(10.0)
            .end_time(5.0) // invalid time range
            .build();

        // When building.
        let result = GroupBuilder::new(params)
            .add_animation(AnimBuilder::opacity())
            .build();

        // Then multiple errors are collected.
        let errors = result.expect_err("should fail with multiple errors");
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert!(
            messages.len() >= 3,
            "should have at least 3 errors: {messages:?}"
        );
        let combined = messages.join("; ");
        assert!(
            combined.contains("end_time must be greater than start_time"),
            "should mention time error: {combined}"
        );
        assert!(
            combined.contains("at least one child"),
            "should mention empty children: {combined}"
        );
        assert!(
            combined.contains("no keyframes"),
            "should mention keyframe error: {combined}"
        );
    }

    #[test]
    fn with_offset_shifts_group_times_and_children() {
        // Given a group at t=0..10 with a child at t=2..8.
        let params = GroupParams::builder()
            .id("shift")
            .children(vec![image_item("c1", 2.0, 8.0)])
            .start_time(0.0)
            .end_time(10.0)
            .build();

        // When applying an offset of 5.0.
        let item = GroupBuilder::new(params).with_offset(5.0).build().unwrap();

        // Then group times are shifted.
        assert_eq!(item.start_time, 5.0);
        assert_eq!(item.end_time, 15.0);

        // And children's times are also shifted.
        let ItemContent::Group { children } = &item.content else {
            panic!("expected group")
        };
        assert_eq!(children[0].start_time, 7.0);
        assert_eq!(children[0].end_time, 13.0);
    }

    #[test]
    fn with_offset_shifts_animation_keyframes() {
        // Given a group with an opacity animation at keyframe times 0.0 and 3.0.
        let params = GroupParams::builder()
            .id("anim-shift")
            .children(vec![image_item("c1", 0.0, 5.0)])
            .start_time(0.0)
            .end_time(5.0)
            .build();
        let anim = AnimBuilder::opacity().keyframe(0.0, 0.0).keyframe(3.0, 1.0);

        // When applying an offset of 10.0.
        let item = GroupBuilder::new(params)
            .add_animation(anim)
            .with_offset(10.0)
            .build()
            .unwrap();

        // Then keyframe times are shifted.
        assert_eq!(item.start_time, 10.0);
        assert_eq!(item.end_time, 15.0);
        assert_eq!(item.animations[0].keyframes[0].time, 10.0);
        assert_eq!(item.animations[0].keyframes[1].time, 13.0);
    }

    #[test]
    fn with_offset_with_zero_offset_is_identity() {
        // Given a group with a child.
        let params = GroupParams::builder()
            .id("identity")
            .children(vec![image_item("c1", 2.0, 8.0)])
            .start_time(0.0)
            .end_time(10.0)
            .build();
        let anim = AnimBuilder::scale_x().keyframe(3.0, 2.0);

        // When applying an offset of 0.0.
        let item = GroupBuilder::new(params)
            .add_animation(anim)
            .with_offset(0.0)
            .build()
            .unwrap();

        // Then nothing changes.
        assert_eq!(item.start_time, 0.0);
        assert_eq!(item.end_time, 10.0);
        assert_eq!(item.animations[0].keyframes[0].time, 3.0);
        let ItemContent::Group { children } = &item.content else {
            panic!("expected group")
        };
        assert_eq!(children[0].start_time, 2.0);
        assert_eq!(children[0].end_time, 8.0);
    }

    // ============================================================
    // group_from_clips
    // ============================================================

    #[test]
    fn group_from_clips_with_single_clip() {
        // Given a single ClipBuilder.
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bg")
                .path("bg.png")
                .end_time(10.0)
                .build(),
        );

        // When wrapping in group_from_clips.
        let params = group_from_clips("grp", vec![clip]).unwrap();

        // Then the group has the child with computed time range.
        assert_eq!(params.id, "grp");
        assert_eq!(params.children.len(), 1);
        assert_eq!(params.children[0].id, "bg");
        assert_eq!(params.start_time, 0.0);
        assert_eq!(params.end_time, 10.0);
    }

    #[test]
    fn group_from_clips_computes_time_range_from_children() {
        // Given two clips with different time ranges.
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
                .start_time(0.0)
                .end_time(10.0)
                .build(),
        );

        // When wrapping in group_from_clips.
        let params = group_from_clips("grp", vec![clip1, clip2]).unwrap();

        // Then start_time is the min and end_time is the max.
        assert_eq!(params.start_time, 0.0);
        assert_eq!(params.end_time, 10.0);
        assert_eq!(params.children.len(), 2);
    }

    #[test]
    fn group_from_clips_with_empty_clips_produces_error() {
        // Given an empty vec of clips.
        // When wrapping in group_from_clips.
        let result = group_from_clips("empty", Vec::<ClipBuilder>::new());

        // Then the error mentions the requirement.
        let errors = result.expect_err("should fail with empty clips");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("at least one clip"),
            "error should describe requirement: {msg}"
        );
    }

    #[test]
    fn group_from_clips_propagates_clip_build_errors() {
        // Given a clip that fails to build.
        let bad_clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bad")
                .path("img.png")
                .start_time(10.0)
                .end_time(5.0) // invalid
                .build(),
        );

        // When wrapping in group_from_clips.
        let result = group_from_clips("grp", vec![bad_clip]);

        // Then the clip's build errors are propagated.
        let errors = result.expect_err("should fail with clip error");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("end_time must be greater than start_time"),
            "error should propagate clip error: {msg}"
        );
    }

    #[test]
    fn bon_builder_accepts_string_into_for_id() {
        // Given a builder using &str for id (tests #[builder(on(String, into))]).
        let params = GroupParams::builder()
            .id("test") // &str, not String
            .children(vec![image_item("c1", 0.0, 5.0)])
            .end_time(5.0)
            .build();

        // When building the group.
        let item = GroupBuilder::new(params).build().unwrap();

        // Then the string is properly owned.
        assert_eq!(item.id, "test");
    }

    #[test]
    fn group_children_are_accessible_via_content() {
        // Given a group with two children.
        let params = GroupParams::builder()
            .id("grp")
            .children(vec![image_item("a", 0.0, 5.0), image_item("b", 0.0, 5.0)])
            .end_time(5.0)
            .build();

        // When building.
        let item = GroupBuilder::new(params).build().unwrap();

        // Then children are accessible via the content enum.
        match &item.content {
            ItemContent::Group { children } => {
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].id, "a");
                assert_eq!(children[1].id, "b");
            }
            _ => panic!("expected group content"),
        }
    }

    #[test]
    fn params_returns_original_params() {
        // Given a GroupBuilder with specific params.
        let params = GroupParams::builder()
            .id("peek")
            .children(vec![image_item("c1", 0.0, 7.0)])
            .end_time(7.0)
            .z_index(3)
            .build();
        let builder = GroupBuilder::new(params);

        // When accessing params.
        let p = builder.params();

        // Then the values match.
        assert_eq!(p.id, "peek");
        assert_eq!(p.z_index, 3);
        assert_eq!(p.end_time, 7.0);
    }
}
