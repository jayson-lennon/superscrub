//! Playback controller: coordinates time advancement, audio sync,
//! and preview cache frame lookups.
//!
//! The controller is the single point of truth for "what frame should
//! the viewport show right now?" It owns `EditorState` and drives
//! the audio engine and preview cache in response to user actions.

use tracing::debug;

use ss_audio::AudioEngineService;
use ss_preview::PreviewCacheService;
use ss_preview::time_to_frame_index;

use crate::editor_state::EditorState;

/// Coordinates playback across editor state, audio, and preview cache.
pub struct PlaybackController {
    state: EditorState,
    audio: AudioEngineService,
    cache: PreviewCacheService,
    preview_fps: u32,
}

impl PlaybackController {
    /// Create a new playback controller.
    pub fn new(
        state: EditorState,
        audio: AudioEngineService,
        cache: PreviewCacheService,
        preview_fps: u32,
    ) -> Self {
        Self {
            state,
            audio,
            cache,
            preview_fps,
        }
    }

    /// Get the current editor state (read-only).
    pub fn state(&self) -> &EditorState {
        &self.state
    }

    /// Get the current editor state (mutable).
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }

    /// Start playback from the current position.
    pub fn play(&mut self) {
        debug!("playback play: time={}", self.state.current_time());
        self.state.start_playback();
        self.audio.play();
    }

    /// Pause playback. Position is retained.
    pub fn pause(&mut self) {
        debug!("playback pause: time={}", self.state.current_time());
        self.state.stop_playback();
        self.audio.pause();
    }

    /// Toggle between play and pause.
    pub fn toggle_playback(&mut self) {
        if self.state.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Stop and reset to the beginning.
    pub fn stop(&mut self) {
        self.state.stop_and_reset();
        self.audio.pause();
        let _ = self.audio.seek(0.0);
    }

    /// Seek to a specific time and update audio position.
    pub fn seek_to(&mut self, time: f64) {
        debug!("playback seek: time={}", time);
        self.state.seek_to(time);
        let _ = self.audio.seek(self.state.current_time());
    }

    /// Handle a timeline click at the given normalized position [0, 1].
    ///
    /// Converts the fraction to a time value and seeks.
    pub fn seek_to_fraction(&mut self, fraction: f64) {
        let duration = self.state.duration();
        if duration > 0.0 {
            self.seek_to(fraction * duration);
        }
    }

    /// Advance time by the given delta (call each frame when playing).
    ///
    /// Returns `true` if playback should continue.
    /// When playback reaches the end, auto-pauses and returns `false`.
    pub fn advance(&mut self, dt: f64) -> bool {
        if !self.state.is_playing() {
            return false;
        }

        let continuing = self.state.advance_time(dt);
        if !continuing {
            self.pause();
        }
        false // Let the caller re-check is_playing()
    }

    /// Get the current frame from the preview cache.
    ///
    /// Returns `None` if no project is loaded, the frame index is out of
    /// range, or the frame hasn't been rendered yet.
    pub fn current_frame(&self) -> Option<image::RgbaImage> {
        let project = self.state.project()?;
        let index = time_to_frame_index(
            self.state.current_time(),
            self.preview_fps,
            project.duration,
        )?;
        self.cache.get_frame(index)
    }

    /// Get the current render progress from the preview cache.
    pub fn render_progress(&self) -> ss_preview::PreviewRenderProgress {
        self.cache.progress()
    }

    /// Update the preview fps (e.g., from settings panel).
    pub fn set_preview_fps(&mut self, fps: u32) {
        self.preview_fps = fps;
    }

    /// The current preview fps setting.
    pub fn preview_fps(&self) -> u32 {
        self.preview_fps
    }

    /// Start a preview render for the current project.
    ///
    /// # Errors
    ///
    /// Returns an error if the render cannot be started.
    pub fn start_render(
        &mut self,
        preview_resolution: (u32, u32),
    ) -> Result<(), error_stack::Report<ss_preview::PreviewError>> {
        let project = self.state.project().cloned().ok_or_else(|| {
            error_stack::Report::new(ss_preview::PreviewError).attach("no project loaded")
        })?;
        let project_file = self.state.project_file().cloned().ok_or_else(|| {
            error_stack::Report::new(ss_preview::PreviewError).attach("no project file set")
        })?;

        self.cache
            .start_render(project, project_file, preview_resolution, self.preview_fps)
    }
}
