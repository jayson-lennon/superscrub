//! Test fixture builders for editor tests.

use ss_core::project::{EncodingConfig, Project};
use ss_core::test_utils::fixtures::build_image_clip;

/// The project file path used in tests.
pub const PROJECT_FILE: &str = "/test/project.json";

/// Build a minimal project with a single clip.
pub fn minimal_project() -> Project {
    let clip = build_image_clip("clip1", "img.png", 0.0, 10.0);
    Project {
        resolution: [200, 100],
        fps: 30,
        duration: 10.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        clips: vec![clip],
    }
}

/// Build a multi-track project for timeline tests.
#[allow(dead_code)]
pub fn multi_track_project() -> Project {
    let mut bg = build_image_clip("bg", "bg.png", 0.0, 10.0);
    bg.track = 0;
    bg.z_index = 0;
    let mut overlay = build_image_clip("overlay", "ov.png", 2.0, 8.0);
    overlay.track = 1;
    overlay.z_index = 1;
    Project {
        resolution: [200, 100],
        fps: 30,
        duration: 10.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        clips: vec![bg, overlay],
    }
}
