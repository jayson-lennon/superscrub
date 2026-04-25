//! Integration tests for BackgroundPreviewCache.
//!
//! Uses `CompositorRenderer` with `FakeImageProvider` to verify
//! that frames are actually rendered and stored.

use std::sync::Arc;
use std::time::Duration;

use ss_compositor::{CompositorRenderer, FakeImageProvider};
use ss_core::project::{EncodingConfig, Project};
use ss_preview::{BackgroundPreviewCache, PreviewCache};

fn minimal_project() -> Project {
    let clip = ss_core::test_utils::fixtures::build_image_clip("test", "test.png", 0.0, 1.0);
    Project {
        resolution: [100, 100],
        fps: 10,
        duration: std::time::Duration::from_secs_f64(1.0),
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio_clips: vec![],
        encoding: EncodingConfig::default(),
        clips: vec![clip],
    }
}

fn wait_for_completion(cache: &BackgroundPreviewCache) {
    let mut waited = 0;
    while !cache.progress().is_complete() && waited < 100 {
        std::thread::sleep(Duration::from_millis(50));
        waited += 1;
    }
}

fn create_cache() -> Arc<BackgroundPreviewCache> {
    let mut provider = FakeImageProvider::new();
    provider.insert_solid("test.png", 50, 50, [255, 0, 0, 255]);
    let renderer = Arc::new(CompositorRenderer::new(Arc::new(provider)));
    Arc::new(BackgroundPreviewCache::new(renderer))
}

#[test]
fn background_cache_renders_frames() {
    // Given a cache with a minimal project.
    let cache = create_cache();
    let project = minimal_project();

    // When starting a render at 10fps, 100x100.
    cache
        .start_render(project, "project.json".into(), (100, 100), 10)
        .unwrap();

    // Then wait for completion and verify frame 0 exists.
    wait_for_completion(&cache);

    let frame = cache.get_frame(0);
    assert!(frame.is_some());
    let frame = frame.unwrap();
    assert_eq!(frame.width(), 100);
    assert_eq!(frame.height(), 100);
}

#[test]
fn background_cache_cancels_previous_render() {
    // Given a cache with a render in progress.
    let cache = create_cache();
    let project = minimal_project();
    cache
        .start_render(project.clone(), "project.json".into(), (100, 100), 10)
        .unwrap();

    // When starting a second render.
    cache
        .start_render(project, "project.json".into(), (50, 50), 10)
        .unwrap();

    // Then wait for the second render to complete and verify dimensions.
    wait_for_completion(&cache);

    let frame = cache.get_frame(0);
    assert!(frame.is_some());
    let frame = frame.unwrap();
    assert_eq!(frame.width(), 50);
    assert_eq!(frame.height(), 50);
}

#[test]
fn get_frame_during_render_does_not_panic() {
    // Given a cache.
    let cache = create_cache();
    let project = minimal_project();

    // When starting a render and immediately getting frame 0.
    cache
        .start_render(project, "project.json".into(), (100, 100), 10)
        .unwrap();

    // Then get_frame does not panic (may return None or Some).
    let _ = cache.get_frame(0);
}
