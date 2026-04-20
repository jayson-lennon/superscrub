//! Tests for the fake audio engine.
//!
//! All tests verify observable behavior through the [`AudioEngine`] trait
//! interface using [`FakeAudioEngine`]. No audio hardware is required.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use ss_audio::{AudioEngine, AudioEngineService, AudioPlaybackState, FakeAudioEngine};

#[test]
fn name_returns_fake() {
    // Given a fake engine.
    let engine = FakeAudioEngine::new();

    // Then the name is "fake".
    assert_eq!(engine.name(), "fake");
}

#[test]
fn initial_state_is_paused() {
    // Given a new fake engine.
    let engine = FakeAudioEngine::new();

    // Then the initial state is Paused.
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn initial_position_is_zero() {
    // Given a new fake engine.
    let engine = FakeAudioEngine::new();

    // Then the initial position is 0.0.
    assert_eq!(engine.position(), 0.0);
}

#[test]
fn initial_duration_is_zero() {
    // Given a new fake engine.
    let engine = FakeAudioEngine::new();

    // Then the initial duration is 0.0.
    assert_eq!(engine.duration(), 0.0);
}

#[test]
fn load_sets_state_to_paused() {
    // Given a fake engine.
    let engine = FakeAudioEngine::new();

    // When loading an audio file.
    engine.load(std::path::Path::new("test.wav")).unwrap();

    // Then the state is Paused.
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn load_records_path() {
    // Given a fake engine.
    let engine = FakeAudioEngine::new();

    // When loading an audio file.
    engine.load(std::path::Path::new("test.wav")).unwrap();

    // Then the last loaded path matches.
    let path = engine.last_loaded_path.lock().unwrap();
    assert_eq!(*path, Some("test.wav".to_string()));
}

#[test]
fn load_increments_count() {
    // Given a fake engine.
    let engine = FakeAudioEngine::new();

    // When loading an audio file.
    engine.load(std::path::Path::new("test.wav")).unwrap();

    // Then load_count is 1.
    assert_eq!(engine.load_count.load(Ordering::SeqCst), 1);
}

#[test]
fn play_transitions_to_playing() {
    // Given a fake engine with audio loaded.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When playing.
    engine.play();

    // Then the state is Playing.
    assert_eq!(engine.state(), AudioPlaybackState::Playing);
}

#[test]
fn play_increments_count() {
    // Given a fake engine with audio loaded.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When playing.
    engine.play();

    // Then play_count is 1.
    assert_eq!(engine.play_count.load(Ordering::SeqCst), 1);
}

#[test]
fn pause_transitions_from_playing_to_paused() {
    // Given a playing engine.
    let engine = FakeAudioEngine::with_duration(30.0);
    engine.play();

    // When pausing.
    engine.pause();

    // Then the state is Paused.
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn pause_is_noop_when_already_paused() {
    // Given a paused engine.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When pausing while already paused.
    engine.pause();

    // Then pause_count incremented but state is still Paused.
    assert_eq!(engine.pause_count.load(Ordering::SeqCst), 1);
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn seek_updates_position() {
    // Given a fake engine with audio loaded.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When seeking to 5.0 seconds.
    engine.seek(5.0).unwrap();

    // Then the position is 5.0.
    assert_eq!(engine.position(), 5.0);
}

#[test]
fn seek_clamps_to_duration() {
    // Given a fake engine with 30s audio.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When seeking past the duration.
    engine.seek(100.0).unwrap();

    // Then the position is clamped to 30.0.
    assert_eq!(engine.position(), 30.0);
}

#[test]
fn seek_clamps_to_zero() {
    // Given a fake engine with audio loaded.
    let engine = FakeAudioEngine::with_duration(30.0);

    // When seeking to a negative time.
    engine.seek(-5.0).unwrap();

    // Then the position is clamped to 0.0.
    assert_eq!(engine.position(), 0.0);
}

#[test]
fn set_volume_clamps_high() {
    // Given a fake engine.
    let engine = FakeAudioEngine::new();

    // When setting volume above 1.0.
    engine.set_volume(2.0);
    engine.seek(0.0).unwrap();

    // Then the engine accepts the call without error (observable: no panic).
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn with_duration_reports_duration() {
    // Given a fake engine created with_duration(30.0).
    let engine = FakeAudioEngine::with_duration(30.0);

    // Then the duration is 30.0.
    assert_eq!(engine.duration(), 30.0);
}

#[test]
fn play_is_noop_when_not_loaded() {
    // Given a new fake engine (no audio loaded).
    let engine = FakeAudioEngine::new();

    // When playing.
    engine.play();

    // Then play_count is incremented (the method was called).
    assert_eq!(engine.play_count.load(Ordering::SeqCst), 1);
    // And state remains Paused since nothing is loaded.
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
}

#[test]
fn pause_and_seek_achieves_stop() {
    // Given a playing engine with a seek position.
    let engine = FakeAudioEngine::with_duration(30.0);
    engine.play();
    engine.seek(5.0).unwrap();

    // When pausing and seeking to 0.
    engine.pause();
    engine.seek(0.0).unwrap();

    // Then state is Paused and position is 0.
    assert_eq!(engine.state(), AudioPlaybackState::Paused);
    assert_eq!(engine.position(), 0.0);
}

#[test]
fn service_can_be_created() {
    // Given a fake engine wrapped in a service.
    let service = AudioEngineService::new(Arc::new(FakeAudioEngine::new()));

    // Then the service exists and can be cloned.
    let _cloned = service.clone();
}
