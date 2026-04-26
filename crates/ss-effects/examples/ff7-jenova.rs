//! FF7 Jenova video example.
//!
//! Generates two project JSONs for a video with:
//! - Ken Burns effect alternating between two background images
//! - `logo-with-background.png` positioned in the bottom-left corner
//! - `track.wav` as the audio track
//!
//! Produces a wide (1920×1080) and tall (1080×1920) variant.
//!
//! ```sh
//! cargo run --example ff7-jenova -p ss-effects
//! ```

use ss_effects::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
use ss_project_builder::{
    AudioClipBuilder, AudioClipParams, ClipBuilder, ClipParams, ProjectBuilder, ProjectParams,
    clip_builder::sizing,
};

/// Project directory containing source assets.
const PROJECT_DIR: &str = "projects/ff7-jenova";

/// Background images for Ken Burns segments (alternated).
const BACKGROUND_A: &str = "background-wide.png";
const BACKGROUND_B: &str = "background-wide 1.png";

/// Logo overlay in the bottom-left corner.
const LOGO: &str = "logo-with-background.png";

/// Audio track.
const AUDIO: &str = "track.wav";

/// Audio duration in seconds (from `ffprobe -v error -show_entries format=duration
/// -of default=noprint_wrappers=1:nokey=1 projects/ff7-jenova/track.wav`).
const DURATION: f64 = 207.52;

/// Wide output video resolution.
const RESOLUTION_WIDE: [u32; 2] = [1920, 1080];

/// Tall output video resolution.
const RESOLUTION_TALL: [u32; 2] = [1080, 1920];

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

/// Tall project audio start time (2:01).
const TALL_AUDIO_START: f64 = 121.0;

/// Tall project audio end time (2:50).
const TALL_AUDIO_END: f64 = 170.0;

fn main() {
    let project_dir = std::path::Path::new(PROJECT_DIR);
    std::fs::create_dir_all(project_dir).expect("failed to create project directory");

    // Wide project: logo bottom-left, standard ken burns, full audio.
    generate_project(
        project_dir,
        "project-wide.json",
        RESOLUTION_WIDE,
        "output-wide.mp4",
        &|res| build_ken_burns(res, false),
        &build_logo_wide,
        0.0,
        DURATION,
    );

    // Tall project: logo centered at bottom, gentler first segment, trimmed audio.
    generate_project(
        project_dir,
        "project-tall.json",
        RESOLUTION_TALL,
        "output-tall.mp4",
        &|res| build_ken_burns(res, true),
        &build_logo_tall,
        TALL_AUDIO_START,
        TALL_AUDIO_END,
    );
}

/// Generates a single project file with the given resolution and output filename.
fn generate_project(
    project_dir: &std::path::Path,
    filename: &str,
    resolution: [u32; 2],
    output: &str,
    build_kb: &dyn Fn([u32; 2]) -> Vec<ClipBuilder>,
    build_logo: &dyn Fn([u32; 2]) -> ClipBuilder,
    audio_start: f64,
    audio_end: f64,
) {
    let kb_clips = build_kb(resolution);
    let logo_clip = build_logo(resolution);

    let audio_clip = AudioClipBuilder::new(
        AudioClipParams::builder()
            .id("audio")
            .path(AUDIO)
            .start_time(audio_start)
            .end_time(audio_end)
            .build(),
    );

    let project_params = ProjectParams::builder()
        .resolution(resolution)
        .fps(FPS)
        .duration(DURATION)
        .output(format!("{PROJECT_DIR}/{output}"))
        .background([0, 0, 0, 255])
        .build();

    let project_path = project_dir.join(filename);
    ProjectBuilder::new(project_params)
        .add_clips_at(0.0, kb_clips)
        .add_clip(logo_clip)
        .add_audio_clip(audio_clip)
        .to_json_file(&project_path)
        .unwrap_or_else(|e| {
            panic!("failed to write {filename}: {e:?}");
        });

    println!("✓ {filename} ({}×{})", resolution[0], resolution[1]);
    println!("  {}", project_path.display());
    println!(
        "  cargo run --release -p ss-render -- {}",
        project_path.display()
    );
}

/// Builds the Ken Burns effect with segments alternating between two images.
///
/// Even segments use `background-wide.png`, odd segments use `background-wide 1.png`.
/// Each segment has `hold=10s` and `fade=2s`, cycling through directions.
///
/// When `gentle_first` is true, the first segment's zoom is reduced to 30%.
fn build_ken_burns(resolution: [u32; 2], gentle_first: bool) -> Vec<ClipBuilder> {
    let segment_time = HOLD + FADE;
    let num_segments = ((DURATION / segment_time).ceil() as usize).max(1);

    let kb_params = KenBurnsParams::builder()
        .duration(DURATION)
        .resolution(resolution)
        .default_hold_duration(HOLD)
        .default_fade_duration(FADE)
        .default_zoom_start(1.4)
        .default_zoom_end(2.0)
        .default_pan_fraction(0.2)
        .build();

    let mut builder = KenBurnsBuilder::new(kb_params);
    for i in 0..num_segments {
        let direction = DIRECTIONS[i % DIRECTIONS.len()].clone();
        let image = if i % 2 == 0 { BACKGROUND_B } else { BACKGROUND_A };

        let segment = if i == 0 && gentle_first {
            // Tall variant: reduce first segment zoom to 30% of original.
            KenBurnsSegmentParams::builder()
                .image_path(image)
                .direction(direction)
                .zoom_start(1.2)
                .zoom_end(1.0 + (3.5 - 1.0) * 0.3)
                .pan_fraction(0.35)
                .build()
        } else if i == 0 {
            KenBurnsSegmentParams::builder()
                .image_path(image)
                .direction(direction)
                .zoom_start(1.2)
                .zoom_end(3.5)
                .pan_fraction(0.35)
                .build()
        } else {
            KenBurnsSegmentParams::builder()
                .image_path(image)
                .direction(direction)
                .build()
        };

        builder = builder.add_segment(segment);
    }

    builder.build()
}

/// Builds the logo clip for the wide variant — bottom-left corner.
fn build_logo_wide(resolution: [u32; 2]) -> ClipBuilder {
    let rect_w = (resolution[0] as f32 * 0.46875) as u32;
    let rect_h = (rect_w as f32 / 2.3077) as u32;
    let rect_x = 20;
    let rect_y: i32 = resolution[1] as i32 - 30 - rect_h as i32;

    ClipBuilder::new(
        ClipParams::builder()
            .id("logo")
            .path(LOGO)
            .start_time(0.0)
            .end_time(DURATION)
            .z_index(100)
            .sizing(sizing::fit_rect_contain(rect_x, rect_y, rect_w, rect_h))
            .build(),
    )
}

/// Builds the logo clip for the tall variant — centered at bottom, 2× size.
fn build_logo_tall(resolution: [u32; 2]) -> ClipBuilder {
    let rect_w = (resolution[0] as f32 * 0.9375) as u32;
    let rect_h = (rect_w as f32 / 2.3077) as u32;
    let rect_x: i32 = (resolution[0] as i32 - rect_w as i32) / 2;
    let rect_y: i32 = resolution[1] as i32 - 30 - rect_h as i32;

    ClipBuilder::new(
        ClipParams::builder()
            .id("logo")
            .path(LOGO)
            .start_time(0.0)
            .end_time(DURATION)
            .z_index(100)
            .sizing(sizing::fit_rect_contain(rect_x, rect_y, rect_w, rect_h))
            .build(),
    )
}
