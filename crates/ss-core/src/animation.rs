//! Animation tracks, keyframes, and easing functions.
//!
//! Each clip can have multiple animation tracks, each targeting a single
//! animatable property with a series of keyframes.

/// A single animation track targeting one property.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AnimationTrack {
    /// Which property this track animates.
    pub property: AnimatableProperty,
    /// Ordered keyframes for this track.
    pub keyframes: Vec<Keyframe>,
}

/// Properties that can be animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum AnimatableProperty {
    #[serde(rename = "translate_x")]
    TranslateX,
    #[serde(rename = "translate_y")]
    TranslateY,
    #[serde(rename = "scale_x")]
    ScaleX,
    #[serde(rename = "scale_y")]
    ScaleY,
    #[serde(rename = "rotation")]
    Rotation,
    #[serde(rename = "opacity")]
    Opacity,
}

/// A single keyframe in an animation track.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Keyframe {
    /// Time in seconds.
    pub time: f64,
    /// Value at this keyframe.
    pub value: f32,
    /// Easing curve from the previous keyframe to this one.
    /// For the first keyframe, this is ignored.
    #[serde(default)]
    pub easing: Easing,
}

/// Supported easing curves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
pub enum Easing {
    #[serde(rename = "Linear")]
    #[default]
    Linear,
    #[serde(rename = "SineInOut")]
    SineInOut,
}
