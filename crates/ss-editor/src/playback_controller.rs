//! Playback controller: coordinates time advancement, audio sync,
//! and preview cache frame lookups.
//!
//! The controller is the single point of truth for "what frame should
//! the viewport show right now?" It owns [`EditorState`] and drives
//! the audio engine and preview cache in response to user actions.
//! It also manages audio loading when a project is loaded or reloaded.

use error_stack::ResultExt;
use tracing::debug;

use ss_audio::{AudioClipInfo, AudioEngineService, AudioError, AudioPlaybackState};
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
        let duration = self.state.duration().as_secs_f64();
        if duration > 0.0 {
            self.seek_to(fraction * duration);
        }
    }

    /// Advance time by the given delta (call each frame when playing).
    ///
    /// Returns `true` if playback should continue.
    /// When playback reaches the end, auto-pauses and returns `false`.
    /// Also pauses if the audio engine auto-stopped (e.g., cpal callback
    /// reached the end of the sample buffer).
    pub fn advance(&mut self, dt: f64) -> bool {
        if !self.state.is_playing() {
            return false;
        }

        // Detect audio engine auto-stop. Only check when audio is loaded
        // (duration > 0 means audio was loaded). If the engine reports Paused
        // while the editor is Playing, the audio reached its end.
        let audio_loaded = self.audio.duration() > std::time::Duration::ZERO;
        if audio_loaded && self.audio.state() == AudioPlaybackState::Paused {
            self.pause();
            return false;
        }

        let continuing = self.state.advance_time(dt);
        if !continuing {
            self.pause();
        }
        false
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

    /// Get a snapshot of which frame indices are currently cached.
    pub fn cached_frames(&self) -> Vec<bool> {
        self.cache.cached_frames()
    }

    /// Update the preview fps (e.g., from settings panel).
    pub fn set_preview_fps(&mut self, fps: u32) {
        self.preview_fps = fps;
    }

    /// The current preview fps setting.
    pub fn preview_fps(&self) -> u32 {
        self.preview_fps
    }

    /// Load audio for the current project, if configured.
    ///
    /// Resolves all audio clip paths relative to the project file and loads
    /// them into the audio engine via `load_clips()`. Pauses audio if the
    /// project has no audio clips.
    /// No-op if no project is loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if any audio path cannot be resolved or any audio
    /// file cannot be loaded.
    pub fn load_project_audio(&self) -> Result<(), error_stack::Report<AudioError>> {
        let Some(project) = self.state.project() else {
            return Ok(());
        };
        let Some(project_file) = self.state.project_file() else {
            return Ok(());
        };

        if project.audio_clips.is_empty() {
            self.audio.pause();
            return Ok(());
        }

        let clip_infos: Vec<AudioClipInfo> = project
            .audio_clips
            .iter()
            .map(|clip| {
                let path = ss_core::path_resolve::resolve_path(project_file, &clip.path)
                    .change_context(AudioError)
                    .attach(format!("audio path: {}", clip.path))?;
                Ok::<_, error_stack::Report<AudioError>>(AudioClipInfo {
                    path,
                    start_time: clip.start_time,
                    end_time: clip.end_time,
                    volume: clip.volume,
                })
            })
            .collect::<Result<Vec<AudioClipInfo>, _>>()?;

        self.audio.load_clips(&clip_infos)?;
        Ok(())
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
