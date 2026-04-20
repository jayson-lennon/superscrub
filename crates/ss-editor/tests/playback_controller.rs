use std::sync::Arc;
use std::sync::atomic::Ordering;

use ss_audio::{AudioEngineService, FakeAudioEngine};
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
    let (mut ctrl, _, fake_cache) = create_controller(30);
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
