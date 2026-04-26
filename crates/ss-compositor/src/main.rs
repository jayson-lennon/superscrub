//! CLI binary for rendering a single frame from a project.
//!
//! Usage: `ss-compositor <project.json> --time <seconds> --output <frame.png>`

use std::path::Path;
use std::sync::Arc;

use ss_compositor::{CompositorRenderer, FilesystemImageProvider, FrameRenderer, Viewport};
use ss_core::project::Project;

fn main() {
    tracing_subscriber::fmt()
        .with_target(true)
        .init();
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        eprintln!("Usage: ss-compositor <project.json> --time <seconds> --output <frame.png>");
        std::process::exit(1);
    }

    let project_path = &args[1];
    let time = parse_time_arg(&args);
    let output_path = parse_output_arg(&args);

    let project_file = Path::new(project_path);
    let json = std::fs::read_to_string(project_file).expect("failed to read project file");
    let project: Project = serde_json::from_str(&json).expect("failed to parse project JSON");

    let renderer = CompositorRenderer::new(Arc::new(FilesystemImageProvider::new()));
    let viewport = Viewport::new_for_output((project.resolution[0], project.resolution[1]));

    let frame = renderer
        .render(&project, project_file, time, &viewport)
        .expect("failed to render frame");

    frame.save(&output_path).expect("failed to save frame");
    eprintln!("Saved frame to {output_path}");
}

fn parse_time_arg(args: &[String]) -> f64 {
    let idx = args
        .iter()
        .position(|a| a == "--time")
        .expect("--time argument required");
    args[idx + 1].parse().expect("--time must be a number")
}

fn parse_output_arg(args: &[String]) -> String {
    let idx = args
        .iter()
        .position(|a| a == "--output")
        .expect("--output argument required");
    args[idx + 1].clone()
}
