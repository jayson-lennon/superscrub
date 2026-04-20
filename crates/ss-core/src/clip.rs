//! Clip definitions for the timeline.
//!
//! A clip represents a single visual element on the timeline with a start time,
//! end time, track, z-order, sizing, and optional animations.

use crate::animation::AnimationTrack;

/// Definition of a single clip on the timeline.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ClipDef {
    /// Unique identifier for reference.
    pub id: String,
    /// What this clip renders.
    #[serde(flatten)]
    pub clip_type: ClipType,
    /// Which track lane this clip occupies (for timeline display).
    pub track: u32,
    /// When this clip becomes visible (seconds).
    pub start_time: f64,
    /// When this clip stops being visible (seconds).
    pub end_time: f64,
    /// Render order. Higher values are rendered on top.
    pub z_index: i32,
    /// How the clip is sized and placed on the canvas.
    #[serde(default)]
    pub sizing: Sizing,
    /// Pivot point for rotation and scale [0..1 range relative to clip bounds].
    #[serde(default = "default_pivot")]
    pub pivot: [f32; 2],
    /// Animations applied to this clip.
    #[serde(default)]
    pub animations: Vec<AnimationTrack>,
}

fn default_pivot() -> [f32; 2] {
    [0.5, 0.5]
}

/// What a clip renders.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum ClipType {
    /// An image file.
    #[serde(rename = "image")]
    Image {
        /// Path to the image, relative to the project file.
        path: String,
    },
}

/// How a clip is sized on the canvas.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize, serde::Serialize)]
pub enum Sizing {
    /// Use the image's natural pixel dimensions.
    #[default]
    Natural,
    /// Explicit width and height in pixels.
    Explicit { width: u32, height: u32 },
    /// Fit within a rectangle on the canvas.
    FitRect {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        mode: FitMode,
        #[serde(default)]
        anchor: FitAnchor,
    },
    /// Scale by a uniform factor.
    Scale(f32),
}

/// How a fitted image is anchored within its rect.
/// Used by FitRect to determine where to crop/position.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize, serde::Serialize)]
pub enum FitAnchor {
    /// Center the image within the rect.
    #[default]
    Center,
    // Future: TopLeft, TopCenter, TopRight, etc.
}

/// How an image fits within a rectangle.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum FitMode {
    /// Scale to cover the entire rect (may crop).
    Cover,
    /// Scale to fit entirely within the rect (may letterbox).
    Contain,
}
