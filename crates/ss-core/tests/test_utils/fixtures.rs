//! Test fixture builders.

use ss_core::animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
use ss_core::clip::{ClipDef, ClipType, Sizing};
use ss_core::project::{AudioConfig, Project};

/// Build a minimal image clip with sensible defaults.
pub fn build_image_clip(id: &str, path: &str, start_time: f64, end_time: f64) -> ClipDef {
    ClipDef {
        id: id.to_string(),
        clip_type: ClipType::Image {
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

/// Build an image clip with a single animation track.
pub fn build_clip_with_animation(
    id: &str,
    property: AnimatableProperty,
    keyframes: Vec<Keyframe>,
) -> ClipDef {
    let mut clip = build_image_clip(id, "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property,
        keyframes,
    }];
    clip
}

/// Build a minimal project with the given clips.
pub fn build_project(clips: Vec<ClipDef>) -> Project {
    Project {
        resolution: [1920, 1080],
        fps: 60,
        duration: 30.0,
        output: "output.mp4".to_string(),
        background: [0x2c, 0x2e, 0x34, 0xff],
        audio: Some(AudioConfig {
            path: "assets/song.mp3".to_string(),
            start_time: 0.0,
        }),
        clips,
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
