//! Test fixture builders for editor tests.

use std::path::PathBuf;

use ss_core::clip::{ClipDef, ClipType, Sizing};
use ss_core::project::Project;

/// The project file path used in tests.
pub const PROJECT_FILE: &str = "/test/project.json";

/// Build a minimal project with a single clip.
pub fn minimal_project() -> Project {
    Project {
        resolution: [200, 100],
        fps: 30,
        duration: 10.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio: None,
        clips: vec![ClipDef {
            id: "clip1".into(),
            clip_type: ClipType::Image {
                path: "img.png".into(),
            },
            track: 0,
            start_time: 0.0,
            end_time: 10.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        }],
    }
}

/// Build a multi-track project for timeline tests.
#[allow(dead_code)]
pub fn multi_track_project() -> Project {
    Project {
        resolution: [200, 100],
        fps: 30,
        duration: 10.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio: None,
        clips: vec![
            ClipDef {
                id: "bg".into(),
                clip_type: ClipType::Image {
                    path: "bg.png".into(),
                },
                track: 0,
                start_time: 0.0,
                end_time: 10.0,
                z_index: 0,
                sizing: Sizing::Natural,
                pivot: [0.5, 0.5],
                animations: vec![],
            },
            ClipDef {
                id: "overlay".into(),
                clip_type: ClipType::Image {
                    path: "ov.png".into(),
                },
                track: 1,
                start_time: 2.0,
                end_time: 8.0,
                z_index: 1,
                sizing: Sizing::Natural,
                pivot: [0.5, 0.5],
                animations: vec![],
            },
        ],
    }
}
