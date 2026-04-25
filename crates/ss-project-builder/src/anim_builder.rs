//! Animation track builder with keyframe accumulation.
//!
//! Provides [`AnimBuilder`] for constructing [`AnimationTrack`] instances
//! via property-specific constructors and keyframe method chains.
//!
//! # Usage
//!
//! ```ignore
//! use ss_project_builder::AnimBuilder;
//! use ss_core::Easing;
//!
//! let track = AnimBuilder::opacity()
//!     .keyframe(0.0, 0.0)
//!     .keyframe_with_easing(3.0, 1.0, Easing::SineInOut)
//!     .build();
//! ```
//!
//! Validation (e.g. non-empty keyframes) is deferred to the parent builder
//! — [`AnimBuilder::build`] returns an [`AnimationTrack`] directly.

use ss_core::{AnimatableProperty, AnimationTrack, Easing, Keyframe};

/// Builder for constructing an [`AnimationTrack`] with accumulated keyframes.
///
/// Create via one of the property-specific constructors, then chain
/// `.keyframe()` / `.keyframe_with_easing()` calls, and finally call `.build()`.
pub struct AnimBuilder {
    property: AnimatableProperty,
    keyframes: Vec<Keyframe>,
}

impl AnimBuilder {
    /// Creates a builder targeting the `ScaleX` property.
    pub fn scale_x() -> Self {
        Self::for_property(AnimatableProperty::ScaleX)
    }

    /// Creates a builder targeting the `ScaleY` property.
    pub fn scale_y() -> Self {
        Self::for_property(AnimatableProperty::ScaleY)
    }

    /// Creates a builder targeting the `TranslateX` property.
    pub fn translate_x() -> Self {
        Self::for_property(AnimatableProperty::TranslateX)
    }

    /// Creates a builder targeting the `TranslateY` property.
    pub fn translate_y() -> Self {
        Self::for_property(AnimatableProperty::TranslateY)
    }

    /// Creates a builder targeting the `Rotation` property.
    pub fn rotation() -> Self {
        Self::for_property(AnimatableProperty::Rotation)
    }

    /// Creates a builder targeting the `Opacity` property.
    pub fn opacity() -> Self {
        Self::for_property(AnimatableProperty::Opacity)
    }

    fn for_property(property: AnimatableProperty) -> Self {
        Self {
            property,
            keyframes: vec![],
        }
    }

    /// Appends a keyframe with [`Easing::Linear`].
    #[must_use]
    pub fn keyframe(mut self, time: f64, value: f32) -> Self {
        self.keyframes.push(Keyframe {
            time,
            value,
            easing: Easing::Linear,
        });
        self
    }

    /// Appends a keyframe with an explicit easing curve.
    #[must_use]
    pub fn keyframe_with_easing(mut self, time: f64, value: f32, easing: Easing) -> Self {
        self.keyframes.push(Keyframe {
            time,
            value,
            easing,
        });
        self
    }

    /// Consumes the builder and returns the [`AnimationTrack`].
    ///
    /// Does not validate — the parent builder is responsible for checking
    /// that keyframes are non-empty during its validation pass.
    pub fn build(self) -> AnimationTrack {
        AnimationTrack {
            property: self.property,
            keyframes: self.keyframes,
        }
    }

    /// Returns the property being animated.
    ///
    /// Intended for parent builders to build path context in error messages.
    pub fn property(&self) -> AnimatableProperty {
        self.property
    }

    /// Returns the number of keyframes accumulated so far.
    ///
    /// Intended for parent builders to validate non-empty keyframes.
    pub fn keyframe_count(&self) -> usize {
        self.keyframes.len()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn constructor_for(property: AnimatableProperty) -> AnimBuilder {
        match property {
            AnimatableProperty::ScaleX => AnimBuilder::scale_x(),
            AnimatableProperty::ScaleY => AnimBuilder::scale_y(),
            AnimatableProperty::TranslateX => AnimBuilder::translate_x(),
            AnimatableProperty::TranslateY => AnimBuilder::translate_y(),
            AnimatableProperty::Rotation => AnimBuilder::rotation(),
            AnimatableProperty::Opacity => AnimBuilder::opacity(),
        }
    }

    #[rstest]
    #[case::scale_x(AnimatableProperty::ScaleX)]
    #[case::scale_y(AnimatableProperty::ScaleY)]
    #[case::translate_x(AnimatableProperty::TranslateX)]
    #[case::translate_y(AnimatableProperty::TranslateY)]
    #[case::rotation(AnimatableProperty::Rotation)]
    #[case::opacity(AnimatableProperty::Opacity)]
    fn all_property_constructors_set_correct_property(#[case] property: AnimatableProperty) {
        // Given a constructor for the property.
        let builder = constructor_for(property);

        // When building the track.
        let track = builder.build();

        // Then the property matches.
        assert_eq!(track.property, property);
    }

    #[test]
    fn build_returns_track_with_correct_property() {
        // Given a builder for opacity.
        let builder = AnimBuilder::opacity();

        // When building with no keyframes.
        let track = builder.build();

        // Then the property is Opacity and keyframes are empty.
        assert_eq!(track.property, AnimatableProperty::Opacity);
        assert!(track.keyframes.is_empty());
    }

    #[test]
    fn keyframe_appends_with_linear_easing() {
        // Given an opacity builder.
        let builder = AnimBuilder::opacity();

        // When adding two keyframes.
        let track = builder.keyframe(0.0, 1.0).keyframe(5.0, 2.0).build();

        // Then both have Linear easing.
        assert_eq!(track.keyframes.len(), 2);
        assert_eq!(track.keyframes[0].time, 0.0);
        assert_eq!(track.keyframes[0].value, 1.0);
        assert_eq!(track.keyframes[0].easing, Easing::Linear);
        assert_eq!(track.keyframes[1].time, 5.0);
        assert_eq!(track.keyframes[1].value, 2.0);
        assert_eq!(track.keyframes[1].easing, Easing::Linear);
    }

    #[test]
    fn keyframe_with_easing_uses_provided_easing() {
        // Given an opacity builder.
        let builder = AnimBuilder::opacity();

        // When adding a keyframe with SineInOut easing.
        let track = builder
            .keyframe_with_easing(0.0, 1.0, Easing::SineInOut)
            .build();

        // Then the easing is SineInOut.
        assert_eq!(track.keyframes.len(), 1);
        assert_eq!(track.keyframes[0].easing, Easing::SineInOut);
    }

    #[test]
    fn mixed_keyframe_types_preserve_order() {
        // Given a translate_x builder.
        let builder = AnimBuilder::translate_x();

        // When mixing keyframe and keyframe_with_easing calls.
        let track = builder
            .keyframe(0.0, 0.0)
            .keyframe_with_easing(2.0, 50.0, Easing::SineInOut)
            .keyframe(4.0, 100.0)
            .build();

        // Then keyframes are in order with correct easings.
        assert_eq!(track.keyframes.len(), 3);

        assert_eq!(track.keyframes[0].time, 0.0);
        assert_eq!(track.keyframes[0].value, 0.0);
        assert_eq!(track.keyframes[0].easing, Easing::Linear);

        assert_eq!(track.keyframes[1].time, 2.0);
        assert_eq!(track.keyframes[1].value, 50.0);
        assert_eq!(track.keyframes[1].easing, Easing::SineInOut);

        assert_eq!(track.keyframes[2].time, 4.0);
        assert_eq!(track.keyframes[2].value, 100.0);
        assert_eq!(track.keyframes[2].easing, Easing::Linear);
    }

    #[test]
    fn keyframe_count_tracks_accumulation() {
        // Given a fresh builder.
        let builder = AnimBuilder::rotation();

        // Then count is 0.
        assert_eq!(builder.keyframe_count(), 0);

        // When adding one keyframe.
        let builder = builder.keyframe(0.0, 0.0);

        // Then count is 1.
        assert_eq!(builder.keyframe_count(), 1);

        // When adding another keyframe.
        let builder = builder.keyframe(1.0, 90.0);

        // Then count is 2.
        assert_eq!(builder.keyframe_count(), 2);
    }

    #[test]
    fn property_accessor_returns_set_property() {
        // Given a builder for translate_y.
        let builder = AnimBuilder::translate_y();

        // When querying the property.
        let property = builder.property();

        // Then it returns TranslateY.
        assert_eq!(property, AnimatableProperty::TranslateY);
    }
}
