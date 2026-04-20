//! Verifies that service wrapper delegation methods call through to backends.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use ss_audio::{AudioEngineService, AudioPlaybackState, FakeAudioEngine};
use ss_preview::{FakePreviewCache, PreviewCacheService};

mod test_utils;

// --- AudioEngineService delegation ---

#[test]
fn audio_service_load_delegates() {
    let fake = Arc::new(FakeAudioEngine::new());
    let service = AudioEngineService::new(fake.clone());

    service
        .load(std::path::Path::new("/test/audio.mp3"))
        .unwrap();
    assert_eq!(fake.load_count.load(Ordering::SeqCst), 1);
}

#[test]
fn audio_service_play_delegates() {
    let fake = Arc::new(FakeAudioEngine::with_duration(10.0));
    let service = AudioEngineService::new(fake.clone());

    service.play();
    assert_eq!(fake.play_count.load(Ordering::SeqCst), 1);
}

#[test]
fn audio_service_pause_delegates() {
    let fake = Arc::new(FakeAudioEngine::with_duration(10.0));
    let service = AudioEngineService::new(fake.clone());

    service.pause();
    assert_eq!(fake.pause_count.load(Ordering::SeqCst), 1);
}

#[test]
fn audio_service_seek_delegates() {
    let fake = Arc::new(FakeAudioEngine::with_duration(10.0));
    let service = AudioEngineService::new(fake.clone());

    service.seek(5.0).unwrap();
    assert_eq!(fake.seek_count.load(Ordering::SeqCst), 1);
}

#[test]
fn audio_service_set_volume_delegates() {
    let fake = Arc::new(FakeAudioEngine::new());
    let service = AudioEngineService::new(fake);

    service.set_volume(0.5);
}

#[test]
fn audio_service_position_delegates() {
    let fake = Arc::new(FakeAudioEngine::with_duration(10.0));
    let service = AudioEngineService::new(fake);

    assert_eq!(service.position(), 0.0);
}

#[test]
fn audio_service_duration_delegates() {
    let fake = Arc::new(FakeAudioEngine::with_duration(30.0));
    let service = AudioEngineService::new(fake);

    assert_eq!(service.duration(), 30.0);
}

#[test]
fn audio_service_state_delegates() {
    let fake = Arc::new(FakeAudioEngine::new());
    let service = AudioEngineService::new(fake);

    assert_eq!(service.state(), AudioPlaybackState::Paused);
}

// --- PreviewCacheService delegation ---

#[test]
fn preview_service_start_render_delegates() {
    let fake = Arc::new(FakePreviewCache::new());
    let service = PreviewCacheService::new(fake.clone());

    let project = test_utils::fixtures::minimal_project();
    service
        .start_render(
            project,
            std::path::PathBuf::from("/test/p.json"),
            (100, 50),
            30,
        )
        .unwrap();
    assert_eq!(fake.start_render_count.load(Ordering::SeqCst), 1);
}

#[test]
fn preview_service_get_frame_delegates() {
    let fake = Arc::new(FakePreviewCache::new());
    let service = PreviewCacheService::new(fake.clone());

    assert!(service.get_frame(0).is_none());
}

#[test]
fn preview_service_progress_delegates() {
    let fake = Arc::new(FakePreviewCache::new());
    let service = PreviewCacheService::new(fake);

    let progress = service.progress();
    assert_eq!(progress.total, 0);
}

#[test]
fn preview_service_cancel_delegates() {
    let fake = Arc::new(FakePreviewCache::new());
    let service = PreviewCacheService::new(fake.clone());

    service.cancel();
    assert_eq!(fake.cancel_count.load(Ordering::SeqCst), 1);
}
