//! Test fixture builders for compositor tests.

use std::sync::Arc;

use ss_compositor::{CompositorRenderer, FakeImageProvider, FrameRenderer, Viewport};
use ss_core::animation::{Easing, Keyframe};
use ss_core::clip::{ClipDef, ClipType, Sizing};
use ss_core::project::{EncodingConfig, Project};

/// The project file path used in tests. Images resolve relative to this.
pub const PROJECT_FILE: &str = "/test/project.json";

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

/// Build a project with the given clips and default settings.
pub fn build_project(clips: Vec<ClipDef>) -> Project {
    Project {
        resolution: [100, 100],
        fps: 30,
        duration: 10.0,
        output: "output.mp4".to_string(),
        background: [0x2c, 0x2e, 0x34, 0xff],
        audio: None,
        encoding: EncodingConfig::default(),
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

/// Create a renderer backed by a shared fake image provider.
pub fn create_renderer(provider: Arc<FakeImageProvider>) -> CompositorRenderer {
    CompositorRenderer::new(provider)
}

/// Create a viewport matching the project resolution.
pub fn project_viewport(project: &Project) -> Viewport {
    Viewport::new_for_output((project.resolution[0], project.resolution[1]))
}

/// Render a frame for the project at the given time using a fake provider.
pub fn render_frame(
    project: &Project,
    provider: &Arc<FakeImageProvider>,
    time: f64,
) -> image::RgbaImage {
    let renderer = create_renderer(Arc::clone(provider));
    let viewport = project_viewport(project);
    let project_file = std::path::Path::new(PROJECT_FILE);
    renderer
        .render(project, project_file, time, &viewport)
        .expect("render should succeed")
}
