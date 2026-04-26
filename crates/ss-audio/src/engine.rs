//! Audio engine trait, playback state, and error types.
//!
//! The [`AudioEngine`] trait abstracts over audio backends, enabling
//! dependency injection for testing. [`AudioPlaybackState`] tracks whether
//! audio is playing or paused.

use std::path::{Path, PathBuf};

use error_stack::Report;

use crate::decoder::DecodedAudio;

/// Metadata for a clip to be loaded by the audio engine.
///
/// Contains the file path and timeline scheduling information.
/// Passed to [`AudioEngine::load_clips()`].
#[derive(Debug, Clone)]
pub struct AudioClipInfo {
    /// Path to the audio file.
    pub path: PathBuf,
    /// When this clip starts on the timeline (seconds).
    pub start_time: f64,
    /// When this clip ends on the timeline (seconds).
    pub end_time: f64,
    /// Per-clip volume level [0.0, 1.0].
    pub volume: f32,
    /// Offset into the source audio where playback begins (seconds).
    /// 0.0 means play from the start of the source file.
    pub source_offset: f64,
    /// Offset into the source audio where playback ends (seconds).
    /// 0.0 means play to the end of the source file.
    pub trim_end: f64,
    /// Audio animation tracks (volume automation).
    pub animations: Vec<ss_core::animation::AudioAnimationTrack>,
}

/// A decoded audio clip ready for mixing.
///
/// Combines the decoded sample buffer with timeline metadata
/// needed to determine when and how to mix this clip.
#[derive(Debug, Clone)]
pub struct LoadedAudioClip {
    /// The decoded audio samples.
    pub audio: DecodedAudio,
    /// When this clip starts on the timeline (seconds).
    pub start_time: f64,
    /// When this clip ends on the timeline (seconds).
    pub end_time: f64,
    /// Per-clip volume level [0.0, 1.0].
    pub volume: f32,
    /// Offset into the source audio where playback begins (seconds).
    /// 0.0 means play from the start of the source file.
    pub source_offset: f64,
    /// Offset into the source audio where playback ends (seconds).
    /// 0.0 means play to the end of the source file.
    pub trim_end: f64,
    /// Audio animation tracks (volume automation).
    pub animations: Vec<ss_core::animation::AudioAnimationTrack>,
}

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
/// Wraps an audio backend to provide play/pause/seek/position.
/// All operations are synchronous.
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

    /// Report the total duration of the loaded audio.
    ///
    /// Returns a zero duration if no audio is loaded.
    fn duration(&self) -> std::time::Duration;

    /// Report the current playback state.
    fn state(&self) -> AudioPlaybackState;

    /// Load multiple audio clips for simultaneous mixing.
    ///
    /// Replaces any previously loaded clips. Each clip has its own
    /// timeline range and volume. During playback, only clips whose
    /// `[start_time, end_time)` range contains the current position
    /// are mixed into the output.
    ///
    /// The engine enters `Paused` state at position 0.
    ///
    /// # Errors
    ///
    /// Returns an error if any audio file cannot be opened or decoded.
    fn load_clips(&self, clips: &[AudioClipInfo]) -> Result<(), Report<AudioError>>;
}

pub mod cpal;
pub mod fake;
pub mod service;
