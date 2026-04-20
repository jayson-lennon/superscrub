//! Resolved clip state after interpolation.
//!
//! A `ResolvedClip` represents the fully computed state of a clip at a given
//! time, with all animation tracks interpolated into final transform values.

use crate::clip::ClipDef;

/// The fully resolved state of a clip at a specific point in time.
///
/// Produced by interpolating all animation tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedClip {
    /// The clip definition (source of truth for sizing, type, etc.).
    pub clip: ClipDef,
    /// Resolved position offset in pixels.
    pub translate: euclid::Vector2D<f32, euclid::UnknownUnit>,
    /// Resolved scale factors.
    pub scale: euclid::Vector2D<f32, euclid::UnknownUnit>,
    /// Resolved rotation in radians.
    pub rotation: f32,
    /// Resolved opacity in [0..1].
    pub opacity: f32,
}
