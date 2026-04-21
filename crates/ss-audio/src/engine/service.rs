//! Service wrapper for the [`AudioEngine`] trait.

use std::path::Path;
use std::sync::Arc;

use derive_more::Debug;

use crate::engine::{AudioClipInfo, AudioEngine, AudioError, AudioPlaybackState};

/// Service wrapper for [`AudioEngine`].
#[derive(Debug, Clone)]
pub struct AudioEngineService {
    #[debug("backend<{}>", self.backend.name())]
    backend: Arc<dyn AudioEngine>,
}

impl AudioEngineService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn AudioEngine>) -> Self {
        Self { backend }
    }

    /// Load an audio file and prepare for playback.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or decoded.
    pub fn load(&self, path: &Path) -> Result<(), error_stack::Report<AudioError>> {
        self.backend.load(path)
    }

    /// Begin or resume playback from the current position.
    pub fn play(&self) {
        self.backend.play();
    }

    /// Pause playback. Position is retained.
    pub fn pause(&self) {
        self.backend.pause();
    }

    /// Seek to a specific time in seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if seeking fails.
    pub fn seek(&self, time: f64) -> Result<(), error_stack::Report<AudioError>> {
        self.backend.seek(time)
    }

    /// Set the playback volume in [0.0, 1.0].
    pub fn set_volume(&self, volume: f32) {
        self.backend.set_volume(volume);
    }

    /// Report the current playback position in seconds.
    pub fn position(&self) -> f64 {
        self.backend.position()
    }

    /// Report the total duration of the loaded audio in seconds.
    pub fn duration(&self) -> f64 {
        self.backend.duration()
    }

    /// Report the current playback state.
    pub fn state(&self) -> AudioPlaybackState {
        self.backend.state()
    }

    /// Load multiple audio clips for simultaneous mixing.
    ///
    /// # Errors
    ///
    /// Returns an error if any audio file cannot be opened or decoded.
    pub fn load_clips(
        &self,
        clips: &[AudioClipInfo],
    ) -> Result<(), error_stack::Report<AudioError>> {
        self.backend.load_clips(clips)
    }
}
