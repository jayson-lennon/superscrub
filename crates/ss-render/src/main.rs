//! Binary entry point for the ss-render headless renderer.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use error_stack::{Report, ResultExt};

use ss_compositor::{CompositorRenderer, FilesystemImageProvider, FrameRendererService};
use ss_core::project::Project;
use ss_render::{
    FfmpegEncoder, ProgressTracker, RenderError, RenderJob, RenderPhase, clamp_time_range,
    detect_ffmpeg, mux_mixed_audio, render_audio,
};

/// Headless renderer for SuperScrub projects.
#[derive(Parser)]
#[command(
    name = "ss-render",
    version,
    about = "Render a SuperScrub project to MP4"
)]
struct Cli {
    /// Path to the project JSON file.
    project: PathBuf,

    /// Start time in seconds (default: 0.0).
    #[arg(long)]
    start: Option<f64>,

    /// End time in seconds (default: project duration).
    #[arg(long)]
    end: Option<f64>,

    /// Output file path (default: project's output field).
    #[arg(long)]
    output: Option<String>,
}

fn main() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();
    if let Err(e) = run() {
        eprintln!("Error: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Report<RenderError>> {
    let cli = Cli::parse();

    // Detect ffmpeg at startup.
    detect_ffmpeg()
        .change_context(RenderError)
        .attach("ffmpeg is required for rendering")?;

    // Load project.
    let project_json = std::fs::read_to_string(&cli.project)
        .change_context(RenderError)
        .attach("failed to read project file")?;
    let project: Project = serde_json::from_str(&project_json)
        .change_context(RenderError)
        .attach("failed to parse project JSON")?;

    // Compute time range.
    let (start_time, end_time) = clamp_time_range(cli.start, cli.end, project.duration);

    // Determine output path.
    let output_path = cli
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&project.output));

    // Create renderer service.
    let image_provider = Arc::new(FilesystemImageProvider::new());
    let renderer = Arc::new(CompositorRenderer::new(image_provider));
    let renderer_service = FrameRendererService::new(renderer);

    // Create progress tracker.
    let fps = project.fps;
    let total_frames = ss_render::range_frame_count(start_time, end_time, fps);
    let progress = ProgressTracker::new(total_frames);

    // Set up cancel channel and ctrl+c handler.
    let cancel_receiver = {
        let (cancel_sender, cancel_receiver) = kanal::bounded(1);
        ctrlc::set_handler(move || {
            let _ = cancel_sender.send(());
        })
        .change_context(RenderError)
        .attach("failed to set ctrl+c handler")?;
        cancel_receiver
    };

    // Determine the encoder output path.
    // With audio: write to a temp file, then mux into the final output.
    // Without audio: write directly to the final output — no temp needed.
    let needs_audio_mux = !project.audio_clips.is_empty();
    let temp_video = if needs_audio_mux {
        Some(
            tempfile::Builder::new()
                .suffix(".mp4")
                .prefix("superscrub-")
                .tempfile()
                .change_context(RenderError)
                .attach("failed to create temp file")?,
        )
    } else {
        None
    };

    let encoder_output = temp_video
        .as_ref()
        .map(|t| t.path().to_path_buf())
        .unwrap_or_else(|| output_path.clone());

    // Spawn render thread.
    let render_handle = {
        let progress_monitor = progress.clone();
        let project_clone = project.clone();
        let project_file_clone = cli.project.clone();
        let encoding_clone = project.encoding.clone();

        std::thread::spawn(move || {
            let encoder = FfmpegEncoder::new(
                &encoder_output,
                (project_clone.resolution[0], project_clone.resolution[1]),
                project_clone.fps,
                &encoding_clone,
            )
            .change_context(RenderError)
            .attach("failed to create ffmpeg encoder")?;

            let job = RenderJob::new(renderer_service);
            job.render(
                &encoder,
                &project_clone,
                &project_file_clone,
                start_time,
                end_time,
                &progress_monitor,
                &cancel_receiver,
            )
        })
    };

    // Monitor progress from main thread.
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let snap = progress.snapshot();
        println!(
            "  {}/{} frames ({:.0}%)",
            snap.frames_rendered,
            snap.total_frames,
            snap.fraction() * 100.0
        );
        if !matches!(snap.phase, RenderPhase::Rendering)
            || snap.frames_rendered >= snap.total_frames
        {
            break;
        }
    }

    // Join render thread.
    render_handle
        .join()
        .unwrap_or_else(|_| Err(Report::new(RenderError).attach("render thread panicked")))?;

    // Mix and mux audio if needed (temp_video auto-cleaned on drop).
    if !project.audio_clips.is_empty() {
        progress.set_phase(RenderPhase::MuxingAudio);
        println!("Mixing {} audio tracks...", project.audio_clips.len());

        let temp_dir = tempfile::tempdir()
            .change_context(RenderError)
            .attach("failed to create temp dir for audio")?;

        let mixed_wav = render_audio(
            &project,
            &cli.project,
            temp_dir.path(),
            start_time,
            end_time,
        )
        .change_context(RenderError)
        .attach("audio mixing failed")?;

        println!("Muxing audio into {}...", output_path.display());

        let temp = temp_video.expect("temp_video must exist when audio is present");
        mux_mixed_audio(temp.path(), &mixed_wav, &output_path)
            .change_context(RenderError)
            .attach("audio muxing failed")?;
    }

    progress.set_phase(RenderPhase::Complete);
    println!("Done: {}", output_path.display());

    Ok(())
}
