//! Core editor state: current time, loaded project, playback flag.
//!
//! `EditorState` holds the mutable state that changes during editing.
//! It is a plain data struct with semantic methods — no trivial setters.

use std::path::PathBuf;

use tracing::debug;

use ss_core::project::Project;

/// Whether the editor playhead is advancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportState {
    /// Playhead is paused.
    #[default]
    Paused,
    /// Playhead is advancing.
    Playing,
}

/// The current state of the editor.
///
/// Owns the loaded project and tracks the current playback position.
/// All mutations go through semantic methods.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// The currently loaded project.
    project: Option<Project>,
    /// Path to the project file on disk.
    project_file: Option<PathBuf>,
    /// Current playback position in seconds.
    current_time: f64,
    /// Transport state (playing or paused).
    transport: TransportState,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            project: None,
            project_file: None,
            current_time: 0.0,
            transport: TransportState::default(),
        }
    }
}

impl EditorState {
    /// Create a new empty editor state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a project into the editor.
    ///
    /// Resets playback position to 0 and stops playback.
    pub fn load_project(&mut self, project: Project, path: PathBuf) {
        self.project = Some(project);
        self.project_file = Some(path);
        self.current_time = 0.0;
        self.transport = TransportState::Paused;
        debug!("project loaded");
    }

    /// Whether a project is currently loaded.
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    /// Get a reference to the loaded project.
    ///
    /// Returns `None` if no project is loaded.
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
    }

    /// Get the project file path.
    pub fn project_file(&self) -> Option<&PathBuf> {
        self.project_file.as_ref()
    }

    /// Replace the project data (used by config watcher on file change).
    ///
    /// Keeps the same project file path. Does not reset playback position
    /// so the user sees changes at their current point in the timeline.
    pub fn reload_project(&mut self, project: Project) {
        self.project = Some(project);
    }

    /// The current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// The project duration in seconds (0.0 if no project).
    pub fn duration(&self) -> f64 {
        self.project.as_ref().map(|p| p.duration).unwrap_or(0.0)
    }

    /// Advance the current time by the given delta.
    ///
    /// Clamps to [0, duration). Returns `true` if playback should continue
    /// (i.e., time has not reached the end).
    pub fn advance_time(&mut self, dt: f64) -> bool {
        let duration = self.duration();
        self.current_time = (self.current_time + dt).clamp(0.0, duration);

        // If we've hit the end, stop.
        self.current_time < duration
    }

    /// Seek to a specific time in seconds.
    ///
    /// Clamps to [0, duration).
    pub fn seek_to(&mut self, time: f64) {
        let duration = self.duration();
        self.current_time = time.clamp(0.0, if duration > 0.0 { duration } else { 0.0 });
    }

    /// Whether the editor is currently playing.
    pub fn is_playing(&self) -> bool {
        matches!(self.transport, TransportState::Playing)
    }

    /// Start playback.
    pub fn start_playback(&mut self) {
        if self.project.is_some() {
            self.transport = TransportState::Playing;
            debug!("transport state changed: {:?}", self.transport);
        }
    }

    /// Stop playback (pause). Position is retained.
    pub fn stop_playback(&mut self) {
        self.transport = TransportState::Paused;
    }

    /// Stop playback and reset position to 0.
    pub fn stop_and_reset(&mut self) {
        self.transport = TransportState::Paused;
        self.current_time = 0.0;
        debug!("transport state changed: {:?}", self.transport);
    }

    /// The current transport state.
    pub fn transport_state(&self) -> TransportState {
        self.transport
    }

    /// The project's frames per second (0 if no project).
    pub fn fps(&self) -> u32 {
        self.project.as_ref().map(|p| p.fps).unwrap_or(0)
    }

    /// The project's resolution (0x0 if no project).
    pub fn resolution(&self) -> [u32; 2] {
        self.project
            .as_ref()
            .map(|p| p.resolution)
            .unwrap_or([0, 0])
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ss_core::project::{EncodingConfig, Project};
    use ss_core::test_utils::fixtures::build_image_clip;

    use super::EditorState;

    fn minimal_project() -> Project {
        let clip = build_image_clip("clip1", "img.png", 0.0, 10.0);
        Project {
            resolution: [200, 100],
            fps: 30,
            duration: 10.0,
            output: "out.mp4".into(),
            background: [0, 0, 0, 255],
            audio_clips: vec![],
            encoding: EncodingConfig::default(),
            clips: vec![clip],
        }
    }

    fn loaded_state() -> EditorState {
        let mut state = EditorState::new();
        state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
        state
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
        let mut state = loaded_state();
        state.seek_to(5.0);

        // When loading a new project.
        state.load_project(minimal_project(), PathBuf::from("/test/project.json"));

        // Then time is reset to 0.
        assert_eq!(state.current_time(), 0.0);
    }

    #[test]
    fn load_project_stops_playback() {
        let mut state = loaded_state();
        state.start_playback();
        assert!(state.is_playing());

        state.load_project(minimal_project(), PathBuf::from("/test/project.json"));
        assert!(!state.is_playing());
    }

    #[test]
    fn seek_to_clamps_to_duration() {
        let mut state = loaded_state();

        state.seek_to(15.0);
        assert_eq!(state.current_time(), 10.0);
    }

    #[test]
    fn seek_to_clamps_to_zero() {
        let mut state = loaded_state();

        state.seek_to(-5.0);
        assert_eq!(state.current_time(), 0.0);
    }

    #[test]
    fn advance_time_moves_forward() {
        let mut state = loaded_state();
        state.seek_to(3.0);

        let continuing = state.advance_time(2.0);
        assert!(continuing);
        assert_eq!(state.current_time(), 5.0);
    }

    #[test]
    fn advance_time_clamps_at_duration() {
        let mut state = loaded_state();
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
    fn stop_and_reset_stops_playback_and_resets_time_to_zero() {
        let mut state = loaded_state();
        state.seek_to(5.0);
        state.start_playback();

        state.stop_and_reset();
        assert!(!state.is_playing());
        assert_eq!(state.current_time(), 0.0);
    }

    #[test]
    fn reload_project_keeps_time() {
        let mut state = loaded_state();
        state.seek_to(5.0);

        state.reload_project(minimal_project());
        assert_eq!(state.current_time(), 5.0);
    }

}
