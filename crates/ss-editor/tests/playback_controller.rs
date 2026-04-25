#![allow(clippy::float_cmp)]
use std::sync::Arc;
use std::sync::atomic::Ordering;


use ss_audio::{AudioEngineService, AudioPlaybackState, FakeAudioEngine};
use ss_editor::EditorState;
use ss_editor::PlaybackController;
use ss_preview::{FakePreviewCache, PreviewCacheService};

mod test_utils;

fn create_controller(
    preview_fps: u32,
) -> (
    PlaybackController,
    Arc<FakeAudioEngine>,
    Arc<FakePreviewCache>,
) {
    let fake_audio = Arc::new(FakeAudioEngine::with_duration(10.0));
    let fake_cache = Arc::new(FakePreviewCache::new());

    let audio = AudioEngineService::new(fake_audio.clone());
    let cache = PreviewCacheService::new(fake_cache.clone());
    let state = EditorState::new();

    let controller = PlaybackController::new(state, audio, cache, preview_fps);
    (controller, fake_audio, fake_cache)
}

fn load_project(controller: &mut PlaybackController) {
    let project = test_utils::fixtures::minimal_project();
    controller.state_mut().load_project(
        project,
        std::path::PathBuf::from(test_utils::fixtures::PROJECT_FILE),
    );
}

fn load_project_with_audio(controller: &mut PlaybackController) {
    let mut project = test_utils::fixtures::minimal_project();
    project.audio_clips.push(ss_core::project::AudioClipDef {
        id: "audio".to_string(),
        path: "audio.wav".to_string(),
        track: 0,
        start_time: 0.0,
        end_time: 10.0,
        volume: 1.0,
    });
    controller.state_mut().load_project(
        project,
        std::path::PathBuf::from(test_utils::fixtures::PROJECT_FILE),
    );
}

fn load_project_with_multiple_audio(controller: &mut PlaybackController) {
    let mut project = test_utils::fixtures::minimal_project();
    project.audio_clips.push(ss_core::project::AudioClipDef {
        id: "audio1".to_string(),
        path: "audio1.wav".to_string(),
        track: 0,
        start_time: 0.0,
        end_time: 10.0,
        volume: 1.0,
    });
    project.audio_clips.push(ss_core::project::AudioClipDef {
        id: "audio2".to_string(),
        path: "audio2.wav".to_string(),
        track: 1,
        start_time: 5.0,
        end_time: 15.0,
        volume: 0.8,
    });
    controller.state_mut().load_project(
        project,
        std::path::PathBuf::from(test_utils::fixtures::PROJECT_FILE),
    );
}

#[test]
fn cached_frames_delegates_to_cache() {
    // Given a controller with a loaded project and started render.
    let (mut ctrl, _, _fake_cache) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.start_render((100, 50)).unwrap();

    // When calling cached_frames().
    let cached = ctrl.cached_frames();

    // Then the vec has the correct length and all entries are false.
    assert_eq!(cached.len(), 300);
    assert!(cached.iter().all(|b| !b));
}

#[test]
fn cached_frames_updates_with_inserted_frames() {
    // Given a controller with a loaded project and started render.
    let (mut ctrl, _, fake_cache) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.start_render((100, 50)).unwrap();

    // When inserting a frame via the fake cache.
    fake_cache.insert_frame(0, image::RgbaImage::new(10, 10));

    // Then cached_frames reflects the insertion.
    let cached = ctrl.cached_frames();
    assert!(cached[0]);
    assert!(cached[1..].iter().all(|b| !b));
}

#[test]
fn play_starts_audio() {
    // Given a controller with a loaded project.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);

    // When playing.
    ctrl.play();

    // Then audio was told to play.
    assert_eq!(fake_audio.play_count.load(Ordering::SeqCst), 1);
    assert!(ctrl.state().is_playing());
}

#[test]
fn pause_stops_audio() {
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.play();

    ctrl.pause();
    assert_eq!(fake_audio.pause_count.load(Ordering::SeqCst), 1);
    assert!(!ctrl.state().is_playing());
}

#[test]
fn toggle_playback_switches_state() {
    let (mut ctrl, _, _) = create_controller(30);
    load_project(&mut ctrl);

    ctrl.toggle_playback();
    assert!(ctrl.state().is_playing());

    ctrl.toggle_playback();
    assert!(!ctrl.state().is_playing());
}

#[test]
fn stop_resets_position() {
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.play();
    ctrl.seek_to(5.0);

    ctrl.stop();
    assert!(!ctrl.state().is_playing());
    assert_eq!(ctrl.state().current_time(), 0.0);
    assert_eq!(fake_audio.pause_count.load(Ordering::SeqCst), 1);
}

#[test]
fn seek_to_updates_time() {
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);

    ctrl.seek_to(5.0);
    assert_eq!(ctrl.state().current_time(), 5.0);
    assert_eq!(fake_audio.seek_count.load(Ordering::SeqCst), 1);
}

#[test]
fn seek_to_fraction_converts_to_time() {
    let (mut ctrl, _, _) = create_controller(30);
    load_project(&mut ctrl);

    ctrl.seek_to_fraction(0.5);
    assert_eq!(ctrl.state().current_time(), 5.0);
}

#[test]
fn advance_moves_time_forward() {
    let (mut ctrl, _, _) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.play();

    ctrl.advance(1.0);
    assert!(ctrl.state().current_time() > 0.0);
}

#[test]
fn advance_auto_pauses_at_end() {
    let (mut ctrl, _, _) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.play();
    ctrl.seek_to(9.5);

    ctrl.advance(1.0);
    assert!(!ctrl.state().is_playing());
}

#[test]
fn current_frame_returns_none_without_project() {
    let (ctrl, _, _) = create_controller(30);
    assert!(ctrl.current_frame().is_none());
}

#[test]
fn start_render_delegates_to_cache() {
    let (mut ctrl, _, fake_cache) = create_controller(30);
    load_project(&mut ctrl);

    ctrl.start_render((100, 50)).unwrap();
    assert_eq!(fake_cache.start_render_count.load(Ordering::SeqCst), 1);
}

#[test]
fn render_progress_delegates_to_cache() {
    let (mut ctrl, _, _fake_cache) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.start_render((100, 50)).unwrap();

    let progress = ctrl.render_progress();
    assert_eq!(progress.total, 300); // 10s * 30fps
}

#[test]
fn set_preview_fps_updates_value() {
    let (mut ctrl, _, _) = create_controller(30);
    ctrl.set_preview_fps(60);
    assert_eq!(ctrl.preview_fps(), 60);
}

#[test]
fn seek_to_clamps_to_duration() {
    let (mut ctrl, _, _) = create_controller(30);
    load_project(&mut ctrl);

    ctrl.seek_to(100.0);
    assert_eq!(ctrl.state().current_time(), 10.0);
}

#[test]
fn load_project_audio_loads_when_config_present() {
    // Given a controller with a project that has audio config.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project_with_audio(&mut ctrl);

    // When calling load_project_audio().
    let _ = ctrl.load_project_audio();

    // Then load_clips_count is incremented (uses load_clips, not load).
    assert_eq!(fake_audio.load_clips_count.load(Ordering::SeqCst), 1);
}

#[test]
fn load_project_audio_noop_without_project() {
    // Given a controller with no project loaded.
    let (ctrl, fake_audio, _) = create_controller(30);

    // When calling load_project_audio().
    let result = ctrl.load_project_audio();

    // Then no error is returned and load_clips_count is 0.
    assert!(result.is_ok());
    assert_eq!(fake_audio.load_clips_count.load(Ordering::SeqCst), 0);
}

#[test]
fn load_project_audio_pauses_when_no_audio_config() {
    // Given a controller with a project that has no audio config.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);

    // When calling load_project_audio().
    let _ = ctrl.load_project_audio();

    // Then the fake's pause_count is incremented.
    assert_eq!(fake_audio.pause_count.load(Ordering::SeqCst), 1);
}

#[test]
fn advance_detects_audio_auto_stop() {
    // Given a controller that is playing, with a FakeAudioEngine that has duration > 0.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project(&mut ctrl);
    ctrl.play();

    // When the fake's state is externally set to Paused (simulating auto-stop).
    fake_audio.set_state(AudioPlaybackState::Paused);

    // And advance() is called.
    ctrl.advance(0.1);

    // Then the controller's state is Paused (playback stopped).
    assert!(!ctrl.state().is_playing());
}

#[test]
fn advance_ignores_auto_stop_when_no_audio_loaded() {
    // Given a controller that is playing, with a FakeAudioEngine that has duration 0 (no audio).
    let fake_audio = Arc::new(FakeAudioEngine::new());
    let fake_cache = Arc::new(FakePreviewCache::new());
    let audio = AudioEngineService::new(fake_audio.clone());
    let cache = PreviewCacheService::new(fake_cache.clone());
    let mut ctrl = PlaybackController::new(EditorState::new(), audio, cache, 30);
    load_project(&mut ctrl);
    ctrl.play();

    // When advance() is called.
    ctrl.advance(0.1);

    // Then playback continues (state is still Playing).
    assert!(ctrl.state().is_playing());
}

// ================================================================
// Multi-clip integration tests
// ================================================================

#[test]
fn load_project_audio_loads_all_clips() {
    // Given a controller with a project that has 2 audio clips.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project_with_multiple_audio(&mut ctrl);

    // When calling load_project_audio().
    let _ = ctrl.load_project_audio();

    // Then load_clips was called with 2 clips.
    assert_eq!(fake_audio.load_clips_count.load(Ordering::SeqCst), 1);
    let clips = fake_audio.last_loaded_clips.lock().unwrap();
    assert_eq!(clips.len(), 2);
}

#[test]
fn load_project_audio_resolves_paths() {
    // Given a controller with a project that has audio clips with relative paths.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project_with_multiple_audio(&mut ctrl);

    // When calling load_project_audio().
    let _ = ctrl.load_project_audio();

    // Then the clip paths are resolved relative to the project file.
    let clips = fake_audio.last_loaded_clips.lock().unwrap();
    // PROJECT_FILE = "/test/project.json", so "audio1.wav" → "/test/audio1.wav"
    assert_eq!(clips[0].path, std::path::PathBuf::from("/test/audio1.wav"));
    assert_eq!(clips[1].path, std::path::PathBuf::from("/test/audio2.wav"));
}

#[test]
fn load_project_audio_forwards_clip_metadata() {
    // Given a controller with a project that has audio clips with specific metadata.
    let (mut ctrl, fake_audio, _) = create_controller(30);
    load_project_with_multiple_audio(&mut ctrl);

    // When calling load_project_audio().
    let _ = ctrl.load_project_audio();

    // Then the clip metadata (start_time, end_time, volume) is forwarded.
    let clips = fake_audio.last_loaded_clips.lock().unwrap();
    assert_eq!(clips[0].start_time, 0.0);
    assert_eq!(clips[0].end_time, 10.0);
    assert!((clips[0].volume - 1.0).abs() < f32::EPSILON);
    assert_eq!(clips[1].start_time, 5.0);
    assert_eq!(clips[1].end_time, 15.0);
    assert!((clips[1].volume - 0.8).abs() < f32::EPSILON);
}
