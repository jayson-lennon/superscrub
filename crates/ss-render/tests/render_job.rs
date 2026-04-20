use std::path::PathBuf;
use std::sync::Arc;

use ss_compositor::{CompositorRenderer, FakeImageProvider, FrameRendererService};
use ss_core::clip::ClipType;
use ss_render::{FakeFrameEncoder, ProgressTracker, RenderJob, RenderPhase, range_frame_count};

mod test_utils;

/// Create a render job with a fake image provider pre-loaded with solid images
/// for every clip in the project.
///
/// Images are inserted before wrapping in `Arc` because `insert_solid` takes `&mut self`.
fn create_job_for(project: &ss_core::project::Project) -> RenderJob {
    let project_file = PathBuf::from(test_utils::fixtures::PROJECT_FILE);
    let mut provider = FakeImageProvider::new();
    for clip in &project.clips {
        if let ClipType::Image { path } = &clip.clip_type {
            let resolved =
                ss_core::path_resolve::resolve_path(&project_file, path).expect("resolve path");
            provider.insert_solid(
                &resolved.to_string_lossy(),
                project.resolution[0],
                project.resolution[1],
                [128, 128, 128, 255],
            );
        }
    }
    let provider = Arc::new(provider);
    let renderer = Arc::new(CompositorRenderer::new(provider));
    let service = FrameRendererService::new(renderer);
    RenderJob::new(service)
}

fn make_cancel_channel() -> (kanal::Sender<()>, kanal::Receiver<()>) {
    kanal::bounded(1)
}

#[test]
fn renders_all_frames_in_range() {
    // Given a project with 2.0s duration at 10 fps = 20 frames.
    let project = test_utils::fixtures::minimal_project();
    let job = create_job_for(&project);

    let encoder = FakeFrameEncoder::new();
    let progress = ProgressTracker::new(20);
    let (_, cancel_rx) = make_cancel_channel();

    // When rendering the full range.
    job.render(
        &encoder,
        &project,
        &PathBuf::from(test_utils::fixtures::PROJECT_FILE),
        0.0,
        2.0,
        &progress,
        &cancel_rx,
    )
    .unwrap();

    // Then all 20 frames were encoded.
    assert_eq!(encoder.frame_count(), 20);
    assert!(encoder.is_finished());
}

#[test]
fn reports_progress_for_each_frame() {
    let project = test_utils::fixtures::minimal_project();
    let job = create_job_for(&project);

    let encoder = FakeFrameEncoder::new();
    let progress = ProgressTracker::new(20);
    let (_, cancel_rx) = make_cancel_channel();

    job.render(
        &encoder,
        &project,
        &PathBuf::from(test_utils::fixtures::PROJECT_FILE),
        0.0,
        2.0,
        &progress,
        &cancel_rx,
    )
    .unwrap();

    // Then progress shows all frames rendered.
    let snap = progress.snapshot();
    assert_eq!(snap.frames_rendered, 20);
    assert_eq!(snap.total_frames, 20);
}

#[test]
fn respects_start_end_time_range() {
    // Given a project with 2.0s at 10 fps.
    let project = test_utils::fixtures::minimal_project();
    let job = create_job_for(&project);

    let encoder = FakeFrameEncoder::new();
    let total = range_frame_count(0.5, 1.5, 10);
    let progress = ProgressTracker::new(total);
    let (_, cancel_rx) = make_cancel_channel();

    // When rendering a sub-range.
    job.render(
        &encoder,
        &project,
        &PathBuf::from(test_utils::fixtures::PROJECT_FILE),
        0.5,
        1.5,
        &progress,
        &cancel_rx,
    )
    .unwrap();

    // Then only the sub-range frames were encoded (1.0s * 10fps = 10 frames).
    assert_eq!(encoder.frame_count(), 10);
}

#[test]
fn cancellation_stops_rendering() {
    let project = test_utils::fixtures::minimal_project();
    let job = create_job_for(&project);

    let encoder = FakeFrameEncoder::new();
    let progress = ProgressTracker::new(20);
    let (cancel_tx, cancel_rx) = make_cancel_channel();

    // Send cancel signal before rendering.
    cancel_tx.send(()).unwrap();

    let result = job.render(
        &encoder,
        &project,
        &PathBuf::from(test_utils::fixtures::PROJECT_FILE),
        0.0,
        2.0,
        &progress,
        &cancel_rx,
    );

    // Then the render was cancelled.
    assert!(result.is_err());
    assert_eq!(progress.snapshot().phase, RenderPhase::Cancelled);
    // No frames were encoded (cancelled before first frame).
    assert_eq!(encoder.frame_count(), 0);
}

#[test]
fn empty_project_renders_zero_clips_successfully() {
    // Given a project with no clips.
    let project = test_utils::fixtures::empty_project();
    let job = create_job_for(&project);

    let encoder = FakeFrameEncoder::new();
    let total = range_frame_count(0.0, 2.0, 10); // 20 frames of background only
    let progress = ProgressTracker::new(total);
    let (_, cancel_rx) = make_cancel_channel();

    job.render(
        &encoder,
        &project,
        &PathBuf::from(test_utils::fixtures::PROJECT_FILE),
        0.0,
        2.0,
        &progress,
        &cancel_rx,
    )
    .unwrap();

    // Then background-only frames are still encoded and the job completes successfully.
    assert_eq!(encoder.frame_count(), 20);
    assert!(encoder.is_finished());
}
