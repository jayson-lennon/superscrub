//! Resolved item state after interpolation.
//!
//! A `ResolvedItem` represents the fully computed state of an item at a given
//! time, with all animation tracks interpolated into final transform values.
//! For items inside groups, the transforms are composed through the ancestor chain.

use crate::item::ItemDef;

/// The fully resolved state of an item at a specific point in time.
///
/// Produced by interpolating all animation tracks. For items inside groups,
/// the transform values already include the composed ancestor transforms.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedItem {
    /// The item definition (source of truth for sizing, content, etc.).
    pub item: ItemDef,
    /// Resolved position offset in pixels.
    pub translate: euclid::Vector2D<f32, euclid::UnknownUnit>,
    /// Resolved scale factors.
    pub scale: euclid::Vector2D<f32, euclid::UnknownUnit>,
    /// Resolved rotation in radians.
    pub rotation: f32,
    /// Resolved opacity in [0..1]. Multiplied through ancestor chain for groups.
    pub opacity: f32,
    /// Full z-ordering ancestry for correct sort order.
    ///
    /// Standalone item at z=3 → `[3]`. Group z=5 child z=0 → `[5, 0]`.
    /// Nested group z=1 → group z=5 → child z=0 → `[1, 5, 0]`.
    ///
    /// Sorted lexicographically by [`resolve_items`] so groups act as atomic
    /// z-units: all children of a group render as a contiguous block.
    ///
    /// [`resolve_items`]: crate::interpolation::resolve_items
    pub z_path: Vec<i32>,
}
