//! Test fixture builders.

use crate::animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
use crate::item::{ItemContent, ItemDef, Sizing};
use crate::project::{AudioClipDef, Project};

/// Build a minimal image item with sensible defaults.
pub fn build_image_item(id: &str, path: &str, start_time: f64, end_time: f64) -> ItemDef {
    ItemDef {
        id: id.to_string(),
        content: ItemContent::Image {
            path: path.to_string(),
        },
        track: 0,
        start_time,
        end_time,
        z_index: 0,
        sizing: Sizing::default(),
        pivot: [0.5, 0.5],
        animations: vec![],
    }
}

/// Build an image item with a single animation track.
pub fn build_item_with_animation(
    id: &str,
    property: AnimatableProperty,
    keyframes: Vec<Keyframe>,
) -> ItemDef {
    let mut item = build_image_item(id, "test.png", 0.0, 10.0);
    item.animations = vec![AnimationTrack {
        property,
        keyframes,
    }];
    item
}

/// Build an image item with multiple animation tracks.
pub fn build_item_with_animations(
    id: &str,
    animations: Vec<(AnimatableProperty, Vec<Keyframe>)>,
) -> ItemDef {
    let mut item = build_image_item(id, "test.png", 0.0, 10.0);
    item.animations = animations
        .into_iter()
        .map(|(property, keyframes)| AnimationTrack {
            property,
            keyframes,
        })
        .collect();
    item
}

/// Build a group item with children and optional animations.
pub fn build_group_item(
    id: &str,
    children: Vec<ItemDef>,
    animations: Vec<AnimationTrack>,
) -> ItemDef {
    ItemDef {
        id: id.to_string(),
        content: ItemContent::Group { children },
        track: 0,
        start_time: 0.0,
        end_time: 10.0,
        z_index: 0,
        sizing: Sizing::default(),
        pivot: [0.5, 0.5],
        animations,
    }
}

/// Build a minimal audio clip with sensible defaults.
pub fn build_audio_clip(id: &str, path: &str, start_time: f64, end_time: f64) -> AudioClipDef {
    AudioClipDef {
        id: id.to_string(),
        path: path.to_string(),
        track: 0,
        start_time,
        end_time,
        volume: 1.0,
        source_offset: 0.0,
        trim_end: 0.0,
        animations: vec![],
    }
}

/// Build a minimal project with the given items.
pub fn build_project(items: Vec<ItemDef>) -> Project {
    Project {
        resolution: [1920, 1080],
        fps: 60,
        duration: std::time::Duration::from_secs_f64(30.0),
        output: "output.mp4".to_string(),
        background: [0x2c, 0x2e, 0x34, 0xff],
        audio_clips: vec![build_audio_clip("audio", "assets/song.mp3", 0.0, 30.0)],
        encoding: crate::project::EncodingConfig::default(),
        items,
    }
}

/// Build a keyframe at the given time and value with linear easing.
pub fn kf(time: f64, value: f32) -> Keyframe {
    Keyframe {
        time,
        value,
        easing: Easing::Linear,
    }
}

/// Build a keyframe at the given time and value with the specified easing.
pub fn kf_with_easing(time: f64, value: f32, easing: Easing) -> Keyframe {
    Keyframe {
        time,
        value,
        easing,
    }
}
