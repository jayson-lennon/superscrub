//! FF7 Jenova video example.
//!
//! Generates two project JSONs:
//! - **Wide** (1920×1080) — full audio, alternating wide backgrounds, logo bottom-left.
//! - **Tall** (1080×1920) — 49s audio excerpt, cycling tall backgrounds, logo centered at bottom.
//!
//! ```sh
//! cargo run --example ff7-jenova -p ss-effects
//! ```

use ss_effects::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
use ss_project_builder::{
    AnimBuilder, AudioClipBuilder, AudioClipParams, ClipBuilder, ClipParams, ProjectBuilder,
    ProjectParams, clip_builder::sizing,
};

/// Project directory containing source assets.
const PROJECT_DIR: &str = "projects/ff7-jenova";

/// Logo overlay.
const LOGO: &str = "logo-with-background.png";

/// Audio track.
const AUDIO: &str = "track.wav";

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

    wide::generate(project_dir);
    tall::generate(project_dir);
}

/// Builds a Ken Burns effect cycling through the given images and directions.
fn build_ken_burns(
    resolution: [u32; 2],
    duration: f64,
    images: &[&str],
    first_segment: &dyn Fn(&str, KenBurnsDirection) -> KenBurnsSegmentParams,
) -> Vec<ClipBuilder> {
    let segment_time = HOLD + FADE;
    let num_segments = ((duration / segment_time).ceil() as usize).max(1);

    let kb_params = KenBurnsParams::builder()
        .duration(duration)
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
        let image = images[i % images.len()];

        let segment = if i == 0 {
            first_segment(image, direction)
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

// ---------------------------------------------------------------------------
// Wide project
// ---------------------------------------------------------------------------

mod wide {
    use super::*;

    const RESOLUTION: [u32; 2] = [1920, 1080];
    const DURATION: f64 = 207.52;

    const BACKGROUND_A: &str = "background-wide.png";
    const BACKGROUND_B: &str = "background-wide 1.png";

    const IMAGES: &[&str] = &[BACKGROUND_B, BACKGROUND_A];

    pub fn generate(project_dir: &std::path::Path) {
        let kb_clips = build_ken_burns(RESOLUTION, DURATION, IMAGES, &|image, direction| {
            KenBurnsSegmentParams::builder()
                .image_path(image)
                .direction(direction)
                .zoom_start(1.2)
                .zoom_end(3.5)
                .pan_fraction(0.35)
                .build()
        });

        let logo_clip = build_logo();

        let audio_clip = AudioClipBuilder::new(
            AudioClipParams::builder()
                .id("audio")
                .path(AUDIO)
                .end_time(DURATION)
                .build(),
        );

        write_project(
            project_dir,
            "project-wide.json",
            RESOLUTION,
            DURATION,
            "output-wide.mp4",
            kb_clips,
            logo_clip,
            None,
            audio_clip,
        );
    }

    fn build_logo() -> ClipBuilder {
        let rect_w = (RESOLUTION[0] as f32 * 0.46875) as u32;
        let rect_h = (rect_w as f32 / 2.3077) as u32;
        let rect_x = 20;
        let rect_y: i32 = RESOLUTION[1] as i32 - 30 - rect_h as i32;

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
}

// ---------------------------------------------------------------------------
// Tall project
// ---------------------------------------------------------------------------

mod tall {
    use super::*;

    const RESOLUTION: [u32; 2] = [1080, 1920];
    const DURATION: f64 = 49.0;

    /// Audio source offset (2:01 into the track).
    const SOURCE_OFFSET: f64 = 121.0;

    /// Hold duration per segment (shorter than wide to fit more cuts).
    const HOLD: f64 = 7.0;

    const BG_1: &str = "tall-bg-1.png";
    const BG_2: &str = "tall-bg-2.png";
    const BG_3: &str = "background-upscaled.png";

    const IMAGES: &[&str] = &[BG_1, BG_2, BG_3];

    const THUMB: &str = "thumb-short.png";

    pub fn generate(project_dir: &std::path::Path) {
        let kb_clips = build_tall_ken_burns();

        let logo_clip = build_logo();

        let thumb_clip = build_thumb();

        const FADE_DURATION: f64 = 2.0;

        let audio_clip = AudioClipBuilder::new(
            AudioClipParams::builder()
                .id("audio")
                .path(AUDIO)
                .start_time(0.0)
                .end_time(DURATION)
                .source_offset(SOURCE_OFFSET)
                .animations(vec![ss_core::AudioAnimationTrack {
                    property: ss_core::AudioAnimatableProperty::Volume,
                    keyframes: vec![
                        // Fade in over first 2 seconds.
                        ss_core::Keyframe {
                            time: 0.0,
                            value: 0.0,
                            easing: ss_core::Easing::Linear,
                        },
                        ss_core::Keyframe {
                            time: FADE_DURATION,
                            value: 1.0,
                            easing: ss_core::Easing::Linear,
                        },
                        // Hold at full volume until fade-out begins.
                        ss_core::Keyframe {
                            time: DURATION - FADE_DURATION,
                            value: 1.0,
                            easing: ss_core::Easing::Linear,
                        },
                        // Fade out over last 2 seconds.
                        ss_core::Keyframe {
                            time: DURATION,
                            value: 0.0,
                            easing: ss_core::Easing::Linear,
                        },
                    ],
                }])
                .build(),
        );

        write_project(
            project_dir,
            "project-tall.json",
            RESOLUTION,
            DURATION,
            "output-tall.mp4",
            kb_clips,
            logo_clip,
            Some(thumb_clip),
            audio_clip,
        );
    }

    fn build_tall_ken_burns() -> Vec<ClipBuilder> {
        let segment_time = HOLD + FADE;
        let num_segments = ((DURATION / segment_time).ceil() as usize).max(1);

        let kb_params = KenBurnsParams::builder()
            .duration(DURATION)
            .resolution(RESOLUTION)
            .default_hold_duration(HOLD)
            .default_fade_duration(FADE)
            .default_zoom_start(1.4)
            .default_zoom_end(2.0)
            .default_pan_fraction(0.2)
            .build();

        let mut builder = KenBurnsBuilder::new(kb_params);
        for i in 0..num_segments {
            let image = IMAGES[i % IMAGES.len()];

            let segment = if i == 0 {
                KenBurnsSegmentParams::builder()
                    .image_path(image)
                    .direction(DIRECTIONS[i % DIRECTIONS.len()].clone())
                    .zoom_start(1.2)
                    .zoom_end(1.0 + (3.5 - 1.0) * 0.3)
                    .pan_fraction(0.35)
                    .build()
            } else if image == BG_3 {
                // BG 3: pan from bottom-left to bottom-right.
                KenBurnsSegmentParams::builder()
                    .image_path(image)
                    .direction(KenBurnsDirection::FocalPoint {
                        start: [0.2, 0.65],
                        end: [0.8, 0.7],
                    })
                    .zoom_start(1.6)
                    .zoom_end(2.0)
                    .build()
            } else {
                KenBurnsSegmentParams::builder()
                    .image_path(image)
                    .direction(DIRECTIONS[i % DIRECTIONS.len()].clone())
                    .build()
            };

            builder = builder.add_segment(segment);
        }

        builder.build()
    }

    fn build_logo() -> ClipBuilder {
        let rect_w = (RESOLUTION[0] as f32 * 0.9375) as u32;
        let rect_h = (rect_w as f32 / 2.3077) as u32;
        let rect_x: i32 = (RESOLUTION[0] as i32 - rect_w as i32) / 2;
        let rect_y: i32 = RESOLUTION[1] as i32 - 30 - rect_h as i32;

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

    #[rustfmt::skip]
    fn build_thumb() -> ClipBuilder {
        // Visible-invisible-visible pattern using the clamp trick.
        // See AnimBuilder docs for the keyframe interpolation model.
        //
        // t=0..2     hold at 1.0 (pre-first-keyframe clamping)
        // t=2..4     fade out to 0.0
        // t=4..45    hold at 0.0 (adjacent keyframes with same value)
        // t=45..47   fade in to 1.0
        // t=47..48   hold at 1.0
        // t=48..49   post-last-keyframe hold at 1.0
        let fade_out_end = 4.0;
        let fade_in_start = DURATION - 2.0;
        ClipBuilder::new(
            ClipParams::builder()
                .id("thumb")
                .path(THUMB)
                .start_time(0.0)
                .end_time(DURATION)
                .z_index(200)
                .sizing(sizing::explicit(RESOLUTION[0], RESOLUTION[1]))
                .build(),
        )
        .add_animation(ss_project_builder::AnimBuilder::from_keyframes(
            ss_core::AnimatableProperty::Opacity,
            vec![
                // Clamp trick: first keyframe at t=2, so opacity holds at 1.0 before this.
                ss_core::Keyframe { time: 2.0, value: 1.0, easing: ss_core::Easing::Linear, },
                // Fade out over 2 seconds (t=2 → t=4).
                ss_core::Keyframe { time: fade_out_end, value: 0.0, easing: ss_core::Easing::Linear, },
                // Hold invisible: same value (0.0) from t=4 to t=45.
                ss_core::Keyframe { time: fade_in_start, value: 0.0, easing: ss_core::Easing::Linear, },
                // Fade in over 2 seconds (t=45 → t=47).
                ss_core::Keyframe { time: DURATION, value: 1.0, easing: ss_core::Easing::Linear, },
            ],
        ))
    }
}

// ---------------------------------------------------------------------------
// Shared project writer
// ---------------------------------------------------------------------------

fn write_project(
    project_dir: &std::path::Path,
    filename: &str,
    resolution: [u32; 2],
    duration: f64,
    output: &str,
    kb_clips: Vec<ClipBuilder>,
    logo_clip: ClipBuilder,
    thumb_clip: Option<ClipBuilder>,
    audio_clip: AudioClipBuilder,
) {
    let project_params = ProjectParams::builder()
        .resolution(resolution)
        .fps(FPS)
        .duration(duration)
        .output(format!("{PROJECT_DIR}/{output}"))
        .background([0, 0, 0, 255])
        .build();

    let project_path = project_dir.join(filename);
    let mut builder = ProjectBuilder::new(project_params)
        .add_clips_at(0.0, kb_clips)
        .add_clip(logo_clip);

    if let Some(thumb) = thumb_clip {
        builder = builder.add_clip(thumb);
    }

    builder
        .add_audio_clip(audio_clip)
        .to_json_file(&project_path)
        .unwrap_or_else(|e| {
            panic!("failed to write {filename}: {e:?}");
        });

    println!(
        "✓ {filename} ({}×{}, {:.1}s)",
        resolution[0], resolution[1], duration
    );
    println!("  {}", project_path.display());
    println!(
        "  cargo run --release -p ss-render -- {}",
        project_path.display()
    );
}
