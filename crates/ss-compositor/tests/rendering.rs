//! Tests for the full rendering pipeline.
//!
//! Uses FakeImageProvider with solid-color images to verify pixel-level behavior.

mod test_utils;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use image::Rgba;
use ss_compositor::{FakeImageProvider, FrameRenderer, ImageProvider, Viewport};
use ss_core::animation::{AnimatableProperty, AnimationTrack};
use ss_core::item::Sizing;
use ss_core::project::{EncodingConfig, Project};
use ss_core::test_utils::fixtures::{build_group_item, build_image_item, kf};
use test_utils::fixtures::{
    PROJECT_FILE, build_project, create_renderer, render_frame, render_frame_at_resolution,
};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const DEFAULT_BG: [u8; 4] = [0x2c, 0x2e, 0x34, 0xff];

fn make_project_with_items(items: Vec<ss_core::item::ItemDef>) -> ss_core::project::Project {
    build_project(items)
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
    let clip = build_image_item("test", "test.png", 0.0, 10.0);
    let project = make_project_with_items(vec![clip]);
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
    let clip = build_image_item("test", "test.png", 0.0, 10.0);
    let project = make_project_with_items(vec![clip]);
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
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::Opacity,
        keyframes: vec![kf(0.0, 0.5), kf(10.0, 0.5)],
    }];
    let project = make_project_with_items(vec![clip]);
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
    let mut red_clip = build_image_item("red", "test.png", 0.0, 10.0);
    red_clip.z_index = 0;
    let mut green_clip = build_image_item("green", "green.png", 0.0, 10.0);
    green_clip.z_index = 1;

    let project = make_project_with_items(vec![red_clip, green_clip]);

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
    let project = make_project_with_items(vec![]);
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
    let mut project = make_project_with_items(vec![]);
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
    let clip = build_image_item("test", "test.png", 0.0, 10.0);
    let project = make_project_with_items(vec![clip]);
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
    let clip1 = build_image_item("clip1", "test.png", 0.0, 10.0);
    let clip2 = build_image_item("clip2", "test.png", 0.0, 10.0);
    let project = make_project_with_items(vec![clip1, clip2]);

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
    let first = provider.get(&img_path).expect("first load");
    let second = provider.get(&img_path).expect("second load");

    // Then both loads succeed and return images with matching dimensions.
    assert_eq!(first.dimensions(), second.dimensions());
    assert_eq!(first.dimensions(), (10, 10));
}

#[test]
fn translate_animation_shifts_clip() {
    // Given a clip with a translate_x animation of 50px.
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::TranslateX,
        keyframes: vec![kf(0.0, 50.0), kf(10.0, 50.0)],
    }];
    let project = make_project_with_items(vec![clip]);
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
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
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
    let project = make_project_with_items(vec![clip]);
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
    let clip = build_image_item("test", "test.png", 5.0, 10.0);
    let project = make_project_with_items(vec![clip]);
    let provider = red_provider();

    // When rendering at t=5 (exact start).
    let frame = render_frame(&project, &provider, 5.0);

    // Then the clip is active (red pixel at center).
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel[0], 255, "clip should be active at exact start time");
}

#[test]
fn half_resolution_renders_entire_frame() {
    // Given a 200×200 project with a single red clip filling the canvas.
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.sizing = Sizing::Explicit {
        width: 200,
        height: 200,
    };
    let project = Project {
        resolution: [200, 200],
        fps: 30,
        duration: std::time::Duration::from_secs_f64(10.0),
        output: "output.mp4".to_string(),
        background: DEFAULT_BG,
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        items: vec![clip],
    };
    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 200, 200, RED);
    let provider = Arc::new(provider);

    // When rendering at half resolution (100×100).
    let frame = render_frame_at_resolution(&project, &provider, 5.0, (100, 100));

    // Then the center pixel (50, 50) is red — the entire frame is visible, not cropped.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(
        pixel[0], 255,
        "center pixel should be red at half resolution"
    );
}

#[test]
fn quarter_resolution_renders_entire_frame() {
    // Given a 200×200 project with a single red clip filling the canvas.
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.sizing = Sizing::Explicit {
        width: 200,
        height: 200,
    };
    let project = Project {
        resolution: [200, 200],
        fps: 30,
        duration: std::time::Duration::from_secs_f64(10.0),
        output: "output.mp4".to_string(),
        background: DEFAULT_BG,
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        items: vec![clip],
    };
    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 200, 200, RED);
    let provider = Arc::new(provider);

    // When rendering at quarter resolution (50×50).
    let frame = render_frame_at_resolution(&project, &provider, 5.0, (50, 50));

    // Then the center pixel (25, 25) is red — the entire frame is visible at quarter resolution.
    let pixel = pixel_color(&frame, 25, 25);
    assert_eq!(
        pixel[0], 255,
        "center pixel should be red at quarter resolution"
    );
}

#[test]
fn half_resolution_with_translate_animation_produces_correct_position() {
    // Given a 200×200 project with a 100×100 red clip translated to x=100.
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.sizing = Sizing::Explicit {
        width: 100,
        height: 100,
    };
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::TranslateX,
        keyframes: vec![kf(0.0, 100.0), kf(10.0, 100.0)],
    }];
    let project = Project {
        resolution: [200, 200],
        fps: 30,
        duration: std::time::Duration::from_secs_f64(10.0),
        output: "output.mp4".to_string(),
        background: DEFAULT_BG,
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        items: vec![clip],
    };
    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    let provider = Arc::new(provider);

    // When rendering at full resolution, pixel (25, 100) is background (left of the clip).
    let full_frame = render_frame_at_resolution(&project, &provider, 5.0, (200, 200));
    let full_pixel = pixel_color(&full_frame, 25, 100);
    assert_eq!(
        full_pixel, DEFAULT_BG,
        "full-res pixel (25, 100) should be background (left of clip at x=100)"
    );

    // When rendering at half resolution, pixel (12, 50) is also background.
    // The clip starts at project x=100, which scales to viewport x=50.
    let half_frame = render_frame_at_resolution(&project, &provider, 5.0, (100, 100));
    let half_pixel = pixel_color(&half_frame, 12, 50);
    assert_eq!(
        half_pixel, DEFAULT_BG,
        "half-res pixel (12, 50) should be background (left of scaled clip at x=50)"
    );
}

#[test]
fn clip_at_exact_end_time_is_inactive() {
    // Given a clip ending at t=10.
    let clip = build_image_item("test", "test.png", 0.0, 10.0);
    let project = make_project_with_items(vec![clip]);
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

#[test]
fn zero_opacity_clip_does_not_appear() {
    // Given a clip with opacity animated to exactly 0.
    let mut clip = build_image_item("test", "test.png", 0.0, 10.0);
    clip.animations = vec![AnimationTrack {
        property: AnimatableProperty::Opacity,
        keyframes: vec![kf(0.0, 0.0), kf(10.0, 0.0)],
    }];
    let project = make_project_with_items(vec![clip]);
    let provider = red_provider();

    // When rendering at t=5.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the frame is the background color — the clip contributed nothing.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(pixel, DEFAULT_BG, "clip with zero opacity should not appear");
}

// ============================================================
// Group z-ordering rendering tests
// ============================================================

const BLUE: [u8; 4] = [0, 0, 255, 255];

#[test]
fn group_with_two_children_renders_in_z_order() {
    // Given a group with two children: red at z=0, green at z=1.
    // Both cover the entire canvas.
    let mut red_child = build_image_item("red", "test.png", 0.0, 10.0);
    red_child.z_index = 0;
    let mut green_child = build_image_item("green", "green.png", 0.0, 10.0);
    green_child.z_index = 1;
    let group = build_group_item("group", vec![red_child, green_child], vec![]);

    let project = make_project_with_items(vec![group]);

    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    provider.insert_solid(RESOLVED_GREEN, 100, 100, GREEN);
    let provider = Arc::new(provider);

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then green (higher z) is on top of red.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(
        pixel[1], 255,
        "green channel should be 255 (green child on top)"
    );
}

#[test]
fn group_children_not_interleaved_with_standalone() {
    // Given a group at z=5 with a blue child at z=0, and a standalone
    // green item at z=3. The group is above the standalone, so blue is
    // on top of green.
    let mut blue_child = build_image_item("blue", "blue.png", 0.0, 10.0);
    blue_child.z_index = 0;
    let mut group = build_group_item("group", vec![blue_child], vec![]);
    group.z_index = 5;

    let mut green_standalone = build_image_item("green", "green.png", 0.0, 10.0);
    green_standalone.z_index = 3;

    let project = make_project_with_items(vec![group, green_standalone]);

    let mut provider = FakeImageProvider::new();
    provider.insert_solid(RESOLVED_TEST, 100, 100, RED);
    provider.insert_solid(RESOLVED_GREEN, 100, 100, GREEN);
    provider.insert_solid("/test/blue.png", 100, 100, BLUE);
    let provider = Arc::new(provider);

    // When rendering.
    let frame = render_frame(&project, &provider, 5.0);

    // Then the blue group child (z_path [5,0]) renders on top of green
    // standalone (z_path [3]). The center pixel should be blue.
    let pixel = pixel_color(&frame, 50, 50);
    assert_eq!(
        pixel[2], 255,
        "blue channel should be 255 (group child on top of standalone)"
    );
}
