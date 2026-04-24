//! Test fixture builders for compositor tests.

use std::sync::Arc;

use ss_compositor::{CompositorRenderer, FakeImageProvider, FrameRenderer, Viewport};
use ss_core::project::{EncodingConfig, Project};

/// The project file path used in tests. Images resolve relative to this.
pub const PROJECT_FILE: &str = "/test/project.json";

/// Build a project with the given clips and default settings.
pub fn build_project(clips: Vec<ss_core::clip::ClipDef>) -> Project {
    Project {
        resolution: [100, 100],
        fps: 30,
        duration: 10.0,
        output: "output.mp4".to_string(),
        background: [0x2c, 0x2e, 0x34, 0xff],
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        clips,
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

/// Render a frame for the project at a custom viewport resolution.
pub fn render_frame_at_resolution(
    project: &Project,
    provider: &Arc<FakeImageProvider>,
    time: f64,
    viewport_resolution: (u32, u32),
) -> image::RgbaImage {
    let renderer = create_renderer(Arc::clone(provider));
    let viewport = Viewport::new_for_output(viewport_resolution);
    let project_file = std::path::Path::new(PROJECT_FILE);
    renderer
        .render(project, project_file, time, &viewport)
        .expect("render should succeed")
}
