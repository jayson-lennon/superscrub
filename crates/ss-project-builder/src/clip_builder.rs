//! Clip construction with sizing and animations.
//!
//! Provides [`ClipParams`] (a `bon`-generated typestate builder for compile-time
//! enforcement of required fields) and [`ClipBuilder`] (which wraps a fully-built
//! `ClipParams` and adds animation accumulation and runtime validation).
//!
//! # Usage
//!
//! ```ignore
//! use ss_project_builder::{ClipBuilder, ClipParams, AnimBuilder};
//! use ss_project_builder::clip_builder::sizing;
//!
//! let clip = ClipBuilder::new(
//!     ClipParams::builder()
//!         .id("background")
//!         .path("assets/cover.png")
//!         .end_time(30.0)
//!         .sizing(sizing::fit_rect_cover(0, 0, 1920, 1080))
//!         .build()
//! )
//! .add_animation(
//!     AnimBuilder::opacity()
//!         .keyframe(0.0, 0.0)
//!         .keyframe(3.0, 1.0)
//! )
//! .build()
//! .unwrap();
//! ```
//!
//! The [`sizing`] module provides convenience constructors for [`Sizing`](ss_core::Sizing)
//! variants.

use ss_core::{ClipDef, ClipType, Sizing};

use crate::{AnimBuilder, BuilderError, BuilderErrors};

/// Convenience constructors for [`Sizing`] variants.
///
/// Use these when setting the `.sizing()` field on the bon-generated builder:
///
/// ```ignore
/// ClipParams::builder()
///     .sizing(sizing::fit_rect_cover(0, 0, 1920, 1080))
///     // ...
/// ```
pub mod sizing {
    use ss_core::{FitAnchor, FitMode, Sizing};

    /// Natural sizing — uses the image's native dimensions.
    pub fn natural() -> Sizing {
        Sizing::Natural
    }

    /// Explicit width and height in pixels.
    pub fn explicit(width: u32, height: u32) -> Sizing {
        Sizing::Explicit { width, height }
    }

    /// Fit to cover a rectangle (may crop), anchored at center.
    pub fn fit_rect_cover(x: i32, y: i32, w: u32, h: u32) -> Sizing {
        Sizing::FitRect {
            x,
            y,
            w,
            h,
            mode: FitMode::Cover,
            anchor: FitAnchor::Center,
        }
    }

    /// Fit to contain within a rectangle (may letterbox), anchored at center.
    pub fn fit_rect_contain(x: i32, y: i32, w: u32, h: u32) -> Sizing {
        Sizing::FitRect {
            x,
            y,
            w,
            h,
            mode: FitMode::Contain,
            anchor: FitAnchor::Center,
        }
    }

    /// Uniform scale factor.
    pub fn scale(factor: f32) -> Sizing {
        Sizing::Scale(factor)
    }
}

/// Parameters managed by `bon`'s typestate builder.
///
/// Required fields (`id`, `path`, `end_time`) are enforced at compile time —
/// you cannot call `.build()` without setting them. All other fields have
/// sensible defaults.
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct ClipParams {
    /// Unique identifier for the clip.
    pub id: String,
    /// Path to the source file, relative to the project.
    pub path: String,
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
    /// Sizing mode.
    #[builder(default = Sizing::Natural)]
    pub sizing: Sizing,
    /// Pivot point [0..1] relative to clip bounds.
    #[builder(default = [0.5, 0.5])]
    pub pivot: [f32; 2],
}

/// Builder for constructing a [`ClipDef`] with optional animations.
///
/// Wraps a fully-built [`ClipParams`] (enforced by `bon` at compile time)
/// and adds animation accumulation via [`add_animation`](ClipBuilder::add_animation).
///
/// Call [`build`](ClipBuilder::build) to validate and produce a `ClipDef`.
pub struct ClipBuilder {
    params: ClipParams,
    anim_builders: Vec<AnimBuilder>,
}

impl ClipBuilder {
    /// Creates a new `ClipBuilder` from fully-built [`ClipParams`].
    ///
    /// Use `ClipParams::builder().id(...).path(...).end_time(...).build()` to
    /// construct the params — `bon` enforces required fields at compile time.
    pub fn new(params: ClipParams) -> Self {
        Self {
            params,
            anim_builders: vec![],
        }
    }

    /// Appends an [`AnimBuilder`] to the clip's animation list.
    ///
    /// Animations are built and validated when [`build`](ClipBuilder::build) is called.
    pub fn add_animation(mut self, anim: AnimBuilder) -> Self {
        self.anim_builders.push(anim);
        self
    }

    /// Consumes the builder, validates, and returns a [`ClipDef`].
    ///
    /// # Validation
    ///
    /// - `end_time` must be greater than `start_time`
    /// - All animations must have at least one keyframe
    ///
    /// All errors are collected into a single [`BuilderErrors`] rather than
    /// failing on the first issue.
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if any validation rules are violated.
    pub fn build(self) -> Result<ClipDef, BuilderErrors> {
        let mut errors = BuilderErrors::new();
        let clip_id = &self.params.id;
        let clip_ctx = format!("clip \"{clip_id}\"");

        // Validate end_time > start_time.
        if self.params.end_time <= self.params.start_time {
            errors.push(
                BuilderError::new("end_time must be greater than start_time")
                    .in_context(&clip_ctx),
            );
        }

        // Build all animations, then validate keyframes.
        let animations: Vec<_> = self
            .anim_builders
            .into_iter()
            .map(|ab| ab.build())
            .collect();

        for track in &animations {
            if track.keyframes.is_empty() {
                let prop_name = property_display_name(track.property);
                errors.push(
                    BuilderError::new("animation has no keyframes")
                        .in_context(format!("animation \"{prop_name}\""))
                        .in_context(&clip_ctx),
                );
            }
        }

        errors.into_result()?;

        Ok(ClipDef {
            id: self.params.id,
            clip_type: ClipType::Image {
                path: self.params.path,
            },
            track: self.params.track,
            start_time: self.params.start_time,
            end_time: self.params.end_time,
            z_index: self.params.z_index,
            sizing: self.params.sizing,
            pivot: self.params.pivot,
            animations,
        })
    }
}

/// Returns the user-facing name for an [`AnimatableProperty`](ss_core::AnimatableProperty).
///
/// Uses the serde rename value (e.g., `"scale_x"`) for readable error messages.
fn property_display_name(
    property: ss_core::AnimatableProperty,
) -> &'static str {
    match property {
        ss_core::AnimatableProperty::ScaleX => "scale_x",
        ss_core::AnimatableProperty::ScaleY => "scale_y",
        ss_core::AnimatableProperty::TranslateX => "translate_x",
        ss_core::AnimatableProperty::TranslateY => "translate_y",
        ss_core::AnimatableProperty::Rotation => "rotation",
        ss_core::AnimatableProperty::Opacity => "opacity",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use ss_core::{AnimatableProperty, FitAnchor, FitMode, Sizing};

    /// Creates a minimal valid `ClipParams` for test reuse.
    fn minimal_params(id: &str, path: &str, end_time: f64) -> ClipParams {
        ClipParams::builder()
            .id(id)
            .path(path)
            .end_time(end_time)
            .build()
    }

    #[test]
    fn build_minimal_clip_with_required_fields() {
        // Given a ClipBuilder with only required fields.
        let params = minimal_params("test", "img.png", 10.0);

        // When building with no animations.
        let clip = ClipBuilder::new(params).build().unwrap();

        // Then defaults are applied correctly.
        assert_eq!(clip.id, "test");
        assert_eq!(
            clip.clip_type,
            ClipType::Image {
                path: "img.png".into(),
            }
        );
        assert_eq!(clip.track, 0);
        assert_eq!(clip.start_time, 0.0);
        assert_eq!(clip.end_time, 10.0);
        assert_eq!(clip.z_index, 0);
        assert_eq!(clip.sizing, Sizing::Natural);
        assert_eq!(clip.pivot, [0.5, 0.5]);
        assert!(clip.animations.is_empty());
    }

    #[test]
    fn build_clip_with_all_optional_fields() {
        // Given a ClipBuilder with all optional fields set.
        let params = ClipParams::builder()
            .id("bg")
            .path("bg.png")
            .track(2)
            .start_time(1.0)
            .end_time(15.0)
            .z_index(10)
            .sizing(sizing::explicit(800, 600))
            .pivot([0.0, 1.0])
            .build();

        // When building.
        let clip = ClipBuilder::new(params).build().unwrap();

        // Then all values are reflected in the output.
        assert_eq!(clip.track, 2);
        assert_eq!(clip.start_time, 1.0);
        assert_eq!(clip.end_time, 15.0);
        assert_eq!(clip.z_index, 10);
        assert_eq!(clip.sizing, Sizing::Explicit {
            width: 800,
            height: 600,
        });
        assert_eq!(clip.pivot, [0.0, 1.0]);
    }

    #[test]
    fn build_clip_with_animations() {
        // Given a ClipBuilder with two animations.
        let params = minimal_params("test", "img.png", 10.0);
        let anim1 = AnimBuilder::opacity()
            .keyframe(0.0, 0.0)
            .keyframe(3.0, 1.0);
        let anim2 = AnimBuilder::scale_x()
            .keyframe_with_easing(0.0, 1.0, ss_core::Easing::SineInOut);

        // When building.
        let clip = ClipBuilder::new(params)
            .add_animation(anim1)
            .add_animation(anim2)
            .build()
            .unwrap();

        // Then both animations are present.
        assert_eq!(clip.animations.len(), 2);
        assert_eq!(clip.animations[0].property, AnimatableProperty::Opacity);
        assert_eq!(clip.animations[1].property, AnimatableProperty::ScaleX);
    }

    #[test]
    fn build_clip_with_empty_animation_produces_error() {
        // Given a ClipBuilder with an animation that has no keyframes.
        let params = minimal_params("test", "img.png", 10.0);
        let anim = AnimBuilder::opacity();

        // When building.
        let result = ClipBuilder::new(params).add_animation(anim).build();

        // Then the error has correct path context.
        let errors = result.expect_err("should fail with empty animation");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(msg.contains("clip \"test\""), "error should mention clip: {msg}");
        assert!(msg.contains("animation \"opacity\""), "error should mention property: {msg}");
        assert!(msg.contains("no keyframes"), "error should describe issue: {msg}");
    }

    #[test]
    fn build_clip_with_invalid_time_range() {
        // Given a ClipBuilder where end_time <= start_time.
        let params = ClipParams::builder()
            .id("bad")
            .path("img.png")
            .start_time(10.0)
            .end_time(5.0)
            .build();

        // When building.
        let result = ClipBuilder::new(params).build();

        // Then the error mentions the time range.
        let errors = result.expect_err("should fail with invalid time range");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(
            msg.contains("end_time must be greater than start_time"),
            "error should describe time issue: {msg}"
        );
    }

    #[test]
    fn build_clip_collects_multiple_errors() {
        // Given a ClipBuilder with both invalid time range and empty animation.
        let params = ClipParams::builder()
            .id("multi")
            .path("img.png")
            .start_time(10.0)
            .end_time(5.0)
            .build();

        // When building.
        let result = ClipBuilder::new(params)
            .add_animation(AnimBuilder::opacity())
            .add_animation(AnimBuilder::rotation().keyframe(0.0, 0.0))
            .build();

        // Then both errors are collected.
        let errors = result.expect_err("should fail with multiple errors");
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert_eq!(messages.len(), 2, "should have exactly 2 errors: {messages:?}");
        let combined = messages.join("; ");
        assert!(combined.contains("end_time"), "should mention time error: {combined}");
        assert!(combined.contains("no keyframes"), "should mention keyframe error: {combined}");
    }

    #[rstest]
    #[case::natural(sizing::natural(), Sizing::Natural)]
    #[case::explicit(sizing::explicit(800, 600), Sizing::Explicit { width: 800, height: 600 })]
    #[case::fit_rect_cover(
        sizing::fit_rect_cover(0, 0, 1920, 1080),
        Sizing::FitRect { x: 0, y: 0, w: 1920, h: 1080, mode: FitMode::Cover, anchor: FitAnchor::Center }
    )]
    #[case::fit_rect_contain(
        sizing::fit_rect_contain(10, 20, 640, 480),
        Sizing::FitRect { x: 10, y: 20, w: 640, h: 480, mode: FitMode::Contain, anchor: FitAnchor::Center }
    )]
    #[case::scale(sizing::scale(2.0), Sizing::Scale(2.0))]
    fn sizing_helpers_produce_correct_variants(#[case] actual: Sizing, #[case] expected: Sizing) {
        // Given a sizing helper result.
        // When comparing to the expected Sizing variant.
        // Then they are equal.
        assert_eq!(actual, expected);
    }

    #[test]
    fn property_display_name_matches_serde_renames() {
        // Given each AnimatableProperty variant.
        // When getting the display name.
        // Then it matches the serde rename value.
        assert_eq!(property_display_name(AnimatableProperty::ScaleX), "scale_x");
        assert_eq!(property_display_name(AnimatableProperty::ScaleY), "scale_y");
        assert_eq!(
            property_display_name(AnimatableProperty::TranslateX),
            "translate_x"
        );
        assert_eq!(
            property_display_name(AnimatableProperty::TranslateY),
            "translate_y"
        );
        assert_eq!(property_display_name(AnimatableProperty::Rotation), "rotation");
        assert_eq!(property_display_name(AnimatableProperty::Opacity), "opacity");
    }

    #[test]
    fn bon_builder_accepts_string_into_for_id_and_path() {
        // Given a builder using &str for id and path (tests #[builder(on(String, into))]).
        let params = ClipParams::builder()
            .id("test")  // &str, not String
            .path("img.png")
            .end_time(5.0)
            .build();

        // When building the clip.
        let clip = ClipBuilder::new(params).build().unwrap();

        // Then the strings are properly owned.
        assert_eq!(clip.id, "test");
        assert_eq!(
            clip.clip_type,
            ClipType::Image {
                path: "img.png".into(),
            }
        );
    }
}
