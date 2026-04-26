//! Queens video example.
//!
//! Generates a project JSON for a video with:
//! - Ken Burns effect on `background-wide.png` cycling through zoom/pan directions
//! - `logo.png` positioned in the bottom-left corner
//! - `track.wav` as the audio track
//!
//! ```sh
//! cargo run --example queens -p ss-effects
//! ```

use ss_effects::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
use ss_project_builder::{
    AudioClipBuilder, AudioClipParams, ClipBuilder, ClipParams, ProjectBuilder, ProjectParams,
    clip_builder::sizing,
};

/// Project directory containing source assets.
const PROJECT_DIR: &str = "projects/queens";

/// Background image for Ken Burns segments.
const BACKGROUND: &str = "background-wide.png";

/// Logo overlay in the bottom-left corner.
const LOGO: &str = "logo.png";

/// Audio track.
const AUDIO: &str = "track.wav";

/// Audio duration in seconds (from `ffprobe -v error -show_entries format=duration
/// -of default=noprint_wrappers=1:nokey=1 projects/queens/track.wav`).
const DURATION: f64 = 219.92;

/// Output video resolution.
const RESOLUTION: [u32; 2] = [1920, 1080];

/// Output frame rate.
const FPS: u32 = 30;

/// Ken Burns hold duration per segment (seconds).
const HOLD: f64 = 10.0;

/// Ken Burns fade duration per segment (seconds).
const FADE: f64 = 2.0;

/// Directions to cycle through for Ken Burns segments.
const DIRECTIONS: &[KenBurnsDirection] = &[
    KenBurnsDirection::ZoomInCenter,
    KenBurnsDirection::ZoomInLeft,
    KenBurnsDirection::ZoomInRight,
    KenBurnsDirection::ZoomOutCenter,
    KenBurnsDirection::ZoomInUp,
    KenBurnsDirection::ZoomInDown,
    KenBurnsDirection::ZoomOutLeft,
    KenBurnsDirection::ZoomOutRight,
    KenBurnsDirection::ZoomOutUp,
    KenBurnsDirection::ZoomOutDown,
];

fn main() {
    let project_dir = std::path::Path::new(PROJECT_DIR);
    std::fs::create_dir_all(project_dir).expect("failed to create project directory");

    // Build Ken Burns clips.
    let kb_clips = build_ken_burns();
    println!("Built {} Ken Burns segments", kb_clips.len());

    // Build logo clip.
    let logo_clip = build_logo_clip();

    // Build audio clip.
    let audio_clip = AudioClipBuilder::new(
        AudioClipParams::builder()
            .id("audio")
            .path(AUDIO)
            .end_time(DURATION)
            .build(),
    );

    // Assemble project.
    let project_params = ProjectParams::builder()
        .resolution(RESOLUTION)
        .fps(FPS)
        .duration(DURATION)
        .output(format!("{PROJECT_DIR}/output.mp4"))
        .background([0, 0, 0, 255])
        .build();

    let project_path = project_dir.join("project.json");
    ProjectBuilder::new(project_params)
        .add_clips_at(0.0, kb_clips)
        .add_clip(logo_clip)
        .add_audio_clip(audio_clip)
        .to_json_file(&project_path)
        .unwrap_or_else(|e| {
            panic!("failed to write project.json: {e:?}");
        });

    println!("\n✓ Generated queens project:");
    println!("  {}", project_path.display());
    println!("\nPreview in editor:");
    println!(
        "  cargo run --release -p ss-editor -- {}",
        project_path.display()
    );
    println!("\nRender to MP4:");
    println!(
        "  cargo run --release -p ss-render -- {}",
        project_path.display()
    );
}

/// Builds the Ken Burns effect with segments cycling through directions.
///
/// All segments use `background-wide.png` with `hold=10s` and `fade=2s`.
/// The number of segments is determined by how many fit within the total duration.
fn build_ken_burns() -> Vec<ClipBuilder> {
    // Calculate how many segments fit: each segment occupies hold + fade seconds,
    // but the last segment doesn't need a full fade.
    let segment_time = HOLD + FADE;
    let num_segments = ((DURATION / segment_time).ceil() as usize).max(1);

    let kb_params = KenBurnsParams::builder()
        .duration(DURATION)
        .resolution(RESOLUTION)
        .default_hold_duration(HOLD)
        .default_fade_duration(FADE)
        .default_pan_fraction(0.15)
        .default_zoom_end(1.25)
        .build();

    let mut builder = KenBurnsBuilder::new(kb_params);
    for i in 0..num_segments {
        let direction = DIRECTIONS[i % DIRECTIONS.len()].clone();
        let segment = KenBurnsSegmentParams::builder()
            .image_path(BACKGROUND)
            .direction(direction)
            .build();
        builder = builder.add_segment(segment);
    }

    builder.build()
}

/// Builds the logo clip positioned in the bottom-left corner.
///
/// Uses `FitRect { Contain }` within a 300×130 rectangle at position (20, 920)
/// to place the logo in the bottom-left of the 1920×1080 canvas.
fn build_logo_clip() -> ClipBuilder {
    ClipBuilder::new(
        ClipParams::builder()
            .id("logo")
            .path(LOGO)
            .start_time(0.0)
            .end_time(DURATION)
            .z_index(100)
            .sizing(sizing::fit_rect_contain(20, 920, 300, 130))
            .build(),
    )
}
