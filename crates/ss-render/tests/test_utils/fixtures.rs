//! Test fixture builders for render tests.

use ss_core::clip::{ClipDef, ClipType, Sizing};
use ss_core::project::{EncodingConfig, Project};

/// The project file path used in tests.
pub const PROJECT_FILE: &str = "/test/project.json";

/// Build a minimal project with a single image clip.
///
/// Resolution: 100×50, FPS: 10, Duration: 2.0s (20 frames).
pub fn minimal_project() -> Project {
    Project {
        resolution: [100, 50],
        fps: 10,
        duration: 2.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio: None,
        encoding: EncodingConfig::default(),
        clips: vec![ClipDef {
            id: "clip1".into(),
            clip_type: ClipType::Image {
                path: "img.png".into(),
            },
            track: 0,
            start_time: 0.0,
            end_time: 2.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        }],
    }
}

/// Build a project with a known clip that has no image dependency.
///
/// This project has no clips — useful for testing the render loop
/// without needing to set up a fake image provider.
pub fn empty_project() -> Project {
    Project {
        resolution: [100, 50],
        fps: 10,
        duration: 2.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio: None,
        encoding: EncodingConfig::default(),
        clips: vec![],
    }
}
