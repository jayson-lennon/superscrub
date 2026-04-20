//! Audio engine trait, playback state, and error types.
//!
//! The [`AudioEngine`] trait abstracts over audio backends, enabling
//! dependency injection for testing. [`AudioPlaybackState`] tracks whether
//! audio is playing or paused.

use std::path::Path;

use error_stack::Report;

/// Failed to perform an audio operation.
#[derive(Debug, wherror::Error)]
#[error("audio engine error")]
pub struct AudioError;

/// Audio playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioPlaybackState {
    /// Audio is paused (or no audio loaded).
    #[default]
    Paused,
    /// Audio is playing.
    Playing,
}

/// Audio playback engine with seeking for preview synchronization.
///
/// Wraps an audio backend (rodio) to provide play/pause/seek/position.
/// All operations are synchronous because rodio's sink operations are synchronous.
///
/// There is no `stop` method — the frontend can achieve stop semantics via
/// `pause()` + `seek(0.0)`.
pub trait AudioEngine: Send + Sync {
    /// Returns the name of this audio backend (for debugging).
    fn name(&self) -> &'static str;

    /// Load an audio file and prepare for playback.
    ///
    /// Any previously loaded audio is replaced. The engine enters `Paused` state
    /// at position 0.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or decoded.
    fn load(&self, path: &Path) -> Result<(), Report<AudioError>>;

    /// Begin or resume playback from the current position.
    ///
    /// No-op if already playing. No-op if no audio is loaded.
    fn play(&self);

    /// Pause playback. Position is retained.
    ///
    /// No-op if already paused.
    fn pause(&self);

    /// Seek to a specific time in seconds.
    ///
    /// Works regardless of playback state. Clamps to [0, duration].
    /// No-op if no audio is loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if seeking fails.
    fn seek(&self, time: f64) -> Result<(), Report<AudioError>>;

    /// Set the playback volume.
    ///
    /// `volume` is in [0.0, 1.0]. Values outside this range are clamped.
    fn set_volume(&self, volume: f32);

    /// Report the current playback position in seconds.
    ///
    /// Returns 0.0 if no audio is loaded.
    fn position(&self) -> f64;

    /// Report the total duration of the loaded audio in seconds.
    ///
    /// Returns 0.0 if no audio is loaded.
    fn duration(&self) -> f64;

    /// Report the current playback state.
    fn state(&self) -> AudioPlaybackState;
}

pub mod fake;
pub mod rodio;
pub mod service;
