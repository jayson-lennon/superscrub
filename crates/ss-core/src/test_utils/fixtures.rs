//! Test fixture builders.

use crate::animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
use crate::clip::{ClipDef, ClipType, Sizing};
use crate::project::{AudioClipDef, Project};

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

/// Build an image clip with multiple animation tracks.
pub fn build_clip_with_animations(
    id: &str,
    animations: Vec<(AnimatableProperty, Vec<Keyframe>)>,
) -> ClipDef {
    let mut clip = build_image_clip(id, "test.png", 0.0, 10.0);
    clip.animations = animations
        .into_iter()
        .map(|(property, keyframes)| AnimationTrack {
            property,
            keyframes,
        })
        .collect();
    clip
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
    }
}

/// Build a minimal project with the given clips.
pub fn build_project(clips: Vec<ClipDef>) -> Project {
    Project {
        resolution: [1920, 1080],
        fps: 60,
        duration: std::time::Duration::from_secs_f64(30.0),
        output: "output.mp4".to_string(),
        background: [0x2c, 0x2e, 0x34, 0xff],
        audio_clips: vec![build_audio_clip("audio", "assets/song.mp3", 0.0, 30.0)],
        encoding: crate::project::EncodingConfig::default(),
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
