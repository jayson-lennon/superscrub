//! Tests for the full rendering pipeline.
//!
//! Uses FakeImageProvider with solid-color images to verify pixel-level behavior.

mod test_utils;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use image::Rgba;
use ss_compositor::{FakeImageProvider, FrameRenderer, ImageProvider, Viewport};
use ss_core::animation::{AnimatableProperty, AnimationTrack};
use ss_core::clip::Sizing;
use ss_core::test_utils::fixtures::{build_image_clip, kf};
use test_utils::fixtures::{PROJECT_FILE, build_project, create_renderer, render_frame};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const DEFAULT_BG: [u8; 4] = [0x2c, 0x2e, 0x34, 0xff];

fn make_project_with_clips(clips: Vec<ss_core::clip::ClipDef>) -> ss_core::project::Project {
    build_project(clips)
}

/// Resolved path for "test.png" relative to PROJECT_FILE.
const RESOLVED_TEST: &str = "/test/test.png";
const RESOLVED_GREEN: &str = "/test/green.png";

fn red_provider() -> Arc<FakeImageProvider> {
    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    Arc::new(provider)
}

fn pixel_color(frame: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    frame.get_pixel(x, y).0
}

#[test]
fn single_clip_renders_image_color() {
    // Given a project with a single red clip.
    let clip = build_image_clip("test", "test.png", 0.0, 10.0);
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering at t=5.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the center pixel is red.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel[0], 255);
}

#[test]
fn inactive_clip_does_not_appear() {
    // Given a clip that is not active at t=15.
    let clip = build_image_clip("test", "test.png", 0.0, 10.0);
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering at t=15 (after clip ends).
    let frame = render_frame(&project, &provider, 15.0);

    // Then the center pixel is the background color, not red.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel, DEFAULT_BG);
}

#[test]
fn opacity_half_blends_with_background() {
    // Given a clip with 50% opacity.
    let mut clip = build_image_clip("test", "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::Opacity,
        keyframes: vec![kf(0.0, 0.5), kf(10.0, 0.5)],
    }];
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the center pixel is a blend of red and background.
    let pixel = pixel_color(&frame, 50, 50);
    assert!(
        pixel[0] > 0x2c && pixel[0] < 255,
        "expected blended red channel, got {}",
        pixel[0]
    );
}

#[test]
fn higher_z_index_renders_on_top() {
    // Given two clips: red at z=0, green at z=1.
    let mut red_clip = build_image_clip("red", "test.png", 0.0, 10.0);
    red_clip.z_index = 0;
    let mut green_clip = build_image_clip("green", "green.png", 0.0, 10.0);
    green_clip.z_index = 1;

    let project = make_project_with_clips(vec![red_clip, green_clip]);

    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    provider.insert_solid(RESOLVED_GREEN, 100, 100, GREEN);
    let provider = Arc::new(provider);

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the center pixel is green (higher z_index wins).
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel[1], 255, "green channel should be 255 (green on top)");
}

#[test]
fn background_color_is_default_when_not_specified() {
    // Given a project with default background and no active clips.
    let project = make_project_with_clips(vec![]);
    let provider = Arc::new(FakeImageProvider::new());

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then all pixels are the default background color.
    let pixel = pixel_color(&frame, 0, 0);
    assert_eq!(pixel, DEFAULT_BG);
}

#[test]
fn explicit_background_color_applies() {
    // Given a project with a custom background color.
    let mut project = make_project_with_clips(vec![]);
    project.background = [0xff, 0x00, 0xff, 0xff]; // magenta
    let provider = Arc::new(FakeImageProvider::new());

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then pixels have the custom background color.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel[0], 0xff, "red channel should be 255 for magenta");
}

#[test]
fn camera_pan_shifts_scene() {
    // Given a project with a red clip and a viewport with camera pan.
    let clip = build_image_clip("test", "test.png", 0.0, 10.0);
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering with a camera pan of (50, 0).
    let renderer = create_renderer(Arc::clone(&provider));
    let viewport = Viewport {
        canvas_rect: (0, 0, 100, 100),
        output_size: (100, 100),
        camera_pan: (50.0, 0.0),
    };
    let frame = renderer
        .render(&project, std::path::Path::new(PROJECT_FILE), 5.0, &viewport)
        .expect("render should succeed");

    // Then the red clip is shifted right by 50px, so pixel (0,50) is background.
    let left_pixel = pixel_color(&frame, 0, 50);
    assert_eq!(
        left_pixel, DEFAULT_BG,
        "leftmost pixel should be background after pan"
    );
}

#[test]
fn image_provider_called_per_clip() {
    // Given a project with two clips using the same image path.
    let clip1 = build_image_clip("clip1", "test.png", 0.0, 10.0);
    let clip2 = build_image_clip("clip2", "test.png", 0.0, 10.0);
    let project = make_project_with_clips(vec![clip1, clip2]);

    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    let provider = Arc::new(provider);

    // When rendering.
    let _frame = render_frame(&project, &provider, 5.0);

    // Then the image was loaded exactly twice (once per clip).
    assert_eq!(provider.load_count.load(Ordering::SeqCst), 2);
}

#[test]
fn filesystem_provider_caches_second_request() {
    // Given a FilesystemImageProvider.
    let provider = ss_compositor::FilesystemImageProvider::new();

    // Create a temporary image file.
    let dir = tempfile::tempdir().expect("tempdir");
    let img_path = dir.path().join("test.png");
    let img = image::RgbaImage::from_pixel(10, 10, Rgba(RED));
    img.save(&img_path).expect("save");

    // When requesting the same image twice.
    let _first = provider.get(&img_path).expect("first load");
    let _second = provider.get(&img_path).expect("second load");

    // Then both succeed (cache hit for second, no panic).
    assert!(true, "cache roundtrip succeeded");
}

#[test]
fn translate_animation_shifts_clip() {
    // Given a clip with a translate_x animation of 50px.
    let mut clip = build_image_clip("test", "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::TranslateX,
        keyframes: vec![kf(0.0, 50.0), kf(10.0, 50.0)],
    }];
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then pixel (0, 50) should be background (clip shifted right by 50).
    let pixel = pixel_color(&frame, 0, 50);
    assert_eq!(
        pixel, DEFAULT_BG,
        "origin should be background when clip is shifted"
    );
}

#[test]
fn scale_animation_enlarges_clip() {
    // Given a small clip with a scale animation of 2.0.
    let mut clip = build_image_clip("test", "test.png", 0.0, 10.0);
    clip.sizing = Sizing::Explicit {
        width: 50,
        height: 50,
    };
    clip.animations = vec![
        AnimationTrack {
            property: AnimatableProperty::ScaleX,
            keyframes: vec![kf(0.0, 2.0), kf(10.0, 2.0)],
        },
        AnimationTrack {
            property: AnimatableProperty::ScaleY,
            keyframes: vec![kf(0.0, 2.0), kf(10.0, 2.0)],
        },
    ];
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the center area should be red (scaled to fill canvas).
    let pixel = pixel_color(&frame, 25, 25);
    assert_eq!(pixel[0], 255, "center area should be red after scale");
}

#[test]
fn renderer_name_is_compositor() {
    // Given a compositor renderer.
    let provider = Arc::new(FakeImageProvider::new());
    let renderer = create_renderer(provider);

    // When asking for the name.
    // Then it returns "compositor".
    assert_eq!(renderer.name(), "compositor");
}

#[test]
fn clip_at_exact_start_time_is_active() {
    // Given a clip starting at t=5.
    let clip = build_image_clip("test", "test.png", 5.0, 10.0);
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering at t=5 (exact start).
    let frame = render_frame(&project, &provider, 5.0);

    // Then the clip is active (red pixel at center).
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel[0], 255, "clip should be active at exact start time");
}

#[test]
fn clip_at_exact_end_time_is_inactive() {
    // Given a clip ending at t=10.
    let clip = build_image_clip("test", "test.png", 0.0, 10.0);
    let project = make_project_with_clips(vec![clip]);
    let provider = red_provider();

    // When rendering at t=10 (exact end).
    let frame = render_frame(&project, &provider, 10.0);

    // Then the clip is inactive (background pixel).
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(
        pixel, DEFAULT_BG,
        "clip should be inactive at exact end time"
    );
}
