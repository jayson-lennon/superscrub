use std::path::PathBuf;

use ss_core::clip::{ClipDef, ClipType, Sizing};
use ss_core::project::Project;
use ss_editor::EditorState;

fn minimal_project() -> Project {
    Project {
        resolution: [200, 100],
        fps: 30,
        duration: 10.0,
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio: None,
        clips: vec![ClipDef {
            id: "clip1".into(),
            clip_type: ClipType::Image {
                path: "img.png".into(),
            },
            track: 0,
            start_time: 0.0,
            end_time: 10.0,
            z_index: 0,
            sizing: Sizing::Natural,
            pivot: [0.5, 0.5],
            animations: vec![],
        }],
    }
}

#[test]
fn new_state_has_no_project() {
    // Given a new editor state.
    let state = EditorState::new();

    // Then no project is loaded.
    assert!(!state.has_project());
    assert!(state.project().is_none());
}

#[test]
fn new_state_time_is_zero() {
    // Given a new editor state.
    let state = EditorState::new();

    // Then current time is 0.
    assert_eq!(state.current_time(), 0.0);
}

#[test]
fn new_state_is_not_playing() {
    let state = EditorState::new();
    assert!(!state.is_playing());
}

#[test]
fn load_project_sets_has_project() {
    // Given an empty state.
    let mut state = EditorState::new();

    // When loading a project.
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));

    // Then the project is loaded.
    assert!(state.has_project());
    assert!(state.project().is_some());
}

#[test]
fn load_project_resets_time_to_zero() {
    // Given a state at time 5.0.
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.seek_to(5.0);

    // When loading a new project.
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));

    // Then time is reset to 0.
    assert_eq!(state.current_time(), 0.0);
}

#[test]
fn load_project_stops_playback() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.start_playback();
    assert!(state.is_playing());

    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    assert!(!state.is_playing());
}

#[test]
fn seek_to_clamps_to_duration() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));

    state.seek_to(15.0);
    assert_eq!(state.current_time(), 10.0);
}

#[test]
fn seek_to_clamps_to_zero() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));

    state.seek_to(-5.0);
    assert_eq!(state.current_time(), 0.0);
}

#[test]
fn advance_time_moves_forward() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.seek_to(3.0);

    let continuing = state.advance_time(2.0);
    assert!(continuing);
    assert_eq!(state.current_time(), 5.0);
}

#[test]
fn advance_time_clamps_at_duration() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.seek_to(9.5);

    let continuing = state.advance_time(1.0);
    assert!(!continuing);
    assert_eq!(state.current_time(), 10.0);
}

#[test]
fn start_playback_requires_project() {
    let mut state = EditorState::new();
    state.start_playback();
    assert!(!state.is_playing());
}

#[test]
fn stop_and_reset() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.seek_to(5.0);
    state.start_playback();

    state.stop_and_reset();
    assert!(!state.is_playing());
    assert_eq!(state.current_time(), 0.0);
}

#[test]
fn reload_project_keeps_time() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    state.seek_to(5.0);

    state.reload_project(minimal_project());
    assert_eq!(state.current_time(), 5.0);
}

#[test]
fn duration_returns_project_duration() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    assert_eq!(state.duration(), 10.0);
}

#[test]
fn duration_zero_when_no_project() {
    let state = EditorState::new();
    assert_eq!(state.duration(), 0.0);
}

#[test]
fn fps_returns_project_fps() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    assert_eq!(state.fps(), 30);
}

#[test]
fn resolution_returns_project_resolution() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    assert_eq!(state.resolution(), [200, 100]);
}

#[test]
fn project_file_is_stored() {
    let mut state = EditorState::new();
    state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
    assert_eq!(
        state.project_file(),
        Some(&PathBuf::from("/test/project.json"))
    );
}
