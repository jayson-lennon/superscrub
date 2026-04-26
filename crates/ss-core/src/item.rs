//! Item definitions for the timeline.
//!
//! An item represents a single visual element on the timeline with a start time,
//! end time, track, z-order, sizing, and optional animations. Items can be either
//! images or groups of nested items.

use crate::animation::AnimationTrack;

/// Definition of a single item on the timeline.
///
/// Replaces the former `ClipDef`. An item can be an image or a group
/// of nested items, enabling non-destructive effects via grouping.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ItemDef {
    /// Unique identifier for reference.
    pub id: String,
    /// What this item renders.
    #[serde(flatten)]
    pub content: ItemContent,
    /// Which track lane this item occupies (for timeline display).
    pub track: u32,
    /// When this item becomes visible (seconds).
    pub start_time: f64,
    /// When this item stops being visible (seconds).
    pub end_time: f64,
    /// Render order. Higher values are rendered on top.
    pub z_index: i32,
    /// How the item is sized and placed on the canvas.
    #[serde(default)]
    pub sizing: Sizing,
    /// Pivot point for rotation and scale [0..1 range relative to item bounds].
    #[serde(default = "default_pivot")]
    pub pivot: [f32; 2],
    /// Animations applied to this item.
    #[serde(default)]
    pub animations: Vec<AnimationTrack>,
}

fn default_pivot() -> [f32; 2] {
    [0.5, 0.5]
}

/// What an item renders.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum ItemContent {
    /// An image file.
    #[serde(rename = "image")]
    Image {
        /// Path to the image, relative to the project file.
        path: String,
    },
    /// A group of nested items. Groups enable non-destructive effects
    /// (e.g., opacity) applied to all children, with transforms composing
    /// through the ancestor chain.
    #[serde(rename = "group")]
    Group {
        /// Child items within this group.
        children: Vec<ItemDef>,
    },
}

/// How an item is sized on the canvas.
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

impl ItemDef {
    /// Returns a new `ItemDef` with all time-related fields shifted by `offset`.
    ///
    /// Shifts `start_time`, `end_time`, and all animation keyframe times.
    /// Recursively shifts children in group items.
    ///
    /// Use positive offsets to position items later on the timeline,
    /// negative to shift earlier.
    #[must_use]
    pub fn with_time_offset(&self, offset: f64) -> ItemDef {
        let mut item = self.clone();
        item.start_time += offset;
        item.end_time += offset;
        item.animations = item
            .animations
            .iter()
            .map(|track| AnimationTrack {
                property: track.property,
                keyframes: track
                    .keyframes
                    .iter()
                    .map(|kf| crate::animation::Keyframe {
                        time: kf.time + offset,
                        value: kf.value,
                        easing: kf.easing,
                    })
                    .collect(),
            })
            .collect();
        item.content = match &self.content {
            ItemContent::Image { path } => ItemContent::Image { path: path.clone() },
            ItemContent::Group { children } => ItemContent::Group {
                children: children
                    .iter()
                    .map(|c| c.with_time_offset(offset))
                    .collect(),
            },
        };
        item
    }
}

/// How an image fits within a rectangle.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum FitMode {
    /// Scale to cover the entire rect (may crop).
    Cover,
    /// Scale to fit entirely within the rect (may letterbox).
    Contain,
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::{AnimatableProperty, Easing, Keyframe};

    /// Creates a minimal image `ItemDef` for test reuse.
    fn image_item(id: &str, start: f64, end: f64) -> ItemDef {
        ItemDef {
            id: id.to_string(),
            content: ItemContent::Image {
                path: "test.png".to_string(),
            },
            track: 0,
            start_time: start,
            end_time: end,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        }
    }

    #[test]
    fn with_time_offset_shifts_start_and_end_time() {
        // Given an image item at t=0..10.
        let item = image_item("a", 0.0, 10.0);

        // When applying a time offset of 5.0.
        let shifted = item.with_time_offset(5.0);

        // Then start_time and end_time are shifted.
        assert_eq!(shifted.start_time, 5.0);
        assert_eq!(shifted.end_time, 15.0);
    }

    #[test]
    fn with_time_offset_shifts_animation_keyframe_times() {
        // Given an item with an opacity animation at keyframe times 0.0 and 3.0.
        let item = ItemDef {
            animations: vec![AnimationTrack {
                property: AnimatableProperty::Opacity,
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: 0.0,
                        easing: Easing::Linear,
                    },
                    Keyframe {
                        time: 3.0,
                        value: 1.0,
                        easing: Easing::SineInOut,
                    },
                ],
            }],
            ..image_item("anim", 0.0, 10.0)
        };

        // When applying a time offset of 7.0.
        let shifted = item.with_time_offset(7.0);

        // Then keyframe times are shifted, values and easings preserved.
        assert_eq!(shifted.animations[0].keyframes[0].time, 7.0);
        assert_eq!(shifted.animations[0].keyframes[0].value, 0.0);
        assert_eq!(shifted.animations[0].keyframes[0].easing, Easing::Linear);
        assert_eq!(shifted.animations[0].keyframes[1].time, 10.0);
        assert_eq!(shifted.animations[0].keyframes[1].value, 1.0);
        assert_eq!(shifted.animations[0].keyframes[1].easing, Easing::SineInOut);
    }

    #[test]
    fn with_time_offset_recurses_into_group_children() {
        // Given a group containing two image children.
        let child1 = image_item("c1", 1.0, 5.0);
        let child2 = image_item("c2", 3.0, 8.0);
        let group = ItemDef {
            id: "grp".to_string(),
            content: ItemContent::Group {
                children: vec![child1, child2],
            },
            track: 0,
            start_time: 0.0,
            end_time: 10.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        };

        // When applying a time offset of 2.0.
        let shifted = group.with_time_offset(2.0);

        // Then the group's times are shifted.
        assert_eq!(shifted.start_time, 2.0);
        assert_eq!(shifted.end_time, 12.0);

        // And the children's times are also shifted.
        let ItemContent::Group { children } = &shifted.content else {
            panic!("expected group")
        };
        assert_eq!(children[0].start_time, 3.0);
        assert_eq!(children[0].end_time, 7.0);
        assert_eq!(children[1].start_time, 5.0);
        assert_eq!(children[1].end_time, 10.0);
    }

    #[test]
    fn with_time_offset_with_zero_is_identity() {
        // Given an image item at t=2..8.
        let item = image_item("id", 2.0, 8.0);

        // When applying a time offset of 0.0.
        let shifted = item.with_time_offset(0.0);

        // Then nothing changes.
        assert_eq!(shifted.start_time, 2.0);
        assert_eq!(shifted.end_time, 8.0);
    }

    #[test]
    fn with_time_offset_preserves_non_time_fields() {
        // Given an item with specific non-time fields.
        let item = ItemDef {
            id: "preserve".to_string(),
            content: ItemContent::Image {
                path: "img.png".to_string(),
            },
            track: 3,
            start_time: 1.0,
            end_time: 5.0,
            z_index: 10,
            sizing: Sizing::Scale(2.0),
            pivot: [0.0, 1.0],
            animations: vec![],
        };

        // When applying a time offset of 100.0.
        let shifted = item.with_time_offset(100.0);

        // Then non-time fields are preserved.
        assert_eq!(shifted.id, "preserve");
        assert_eq!(shifted.track, 3);
        assert_eq!(shifted.z_index, 10);
        assert_eq!(shifted.sizing, Sizing::Scale(2.0));
        assert_eq!(shifted.pivot, [0.0, 1.0]);
        assert!(matches!(shifted.content, ItemContent::Image { .. }));
    }

    #[test]
    fn with_time_offset_handles_nested_groups() {
        // Given a group containing a nested group with an image child.
        let inner_child = image_item("leaf", 0.0, 3.0);
        let inner_group = ItemDef {
            id: "inner".to_string(),
            content: ItemContent::Group {
                children: vec![inner_child],
            },
            track: 0,
            start_time: 0.0,
            end_time: 3.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        };
        let outer_group = ItemDef {
            id: "outer".to_string(),
            content: ItemContent::Group {
                children: vec![inner_group],
            },
            track: 0,
            start_time: 0.0,
            end_time: 3.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        };

        // When applying a time offset of 4.0.
        let shifted = outer_group.with_time_offset(4.0);

        // Then all levels are shifted.
        assert_eq!(shifted.start_time, 4.0);
        assert_eq!(shifted.end_time, 7.0);
        let ItemContent::Group { children } = &shifted.content else {
            panic!("expected group")
        };
        let outer_children = &children[0];
        assert_eq!(outer_children.start_time, 4.0);
        assert_eq!(outer_children.end_time, 7.0);
        let ItemContent::Group { children } = &outer_children.content else {
            panic!("expected group")
        };
        let inner_children = &children[0];
        assert_eq!(inner_children.start_time, 4.0);
        assert_eq!(inner_children.end_time, 7.0);
    }
}
