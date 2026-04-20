//! Rodio-based audio engine for real audio playback.
//!
//! Uses rodio 0.22's `MixerDeviceSink` + `Player` API.
//! Position tracking uses `Player::get_pos()` which tracks position
//! through the source pipeline.

use std::path::Path;
use std::sync::Mutex;

use error_stack::{Report, ResultExt};
use rodio::{Decoder, DeviceSinkBuilder, Player, Source, source::SeekError};
use tracing::{debug, info};

use crate::engine::{AudioEngine, AudioError, AudioPlaybackState};

/// Rodio-based audio engine.
///
/// Manages the rodio `MixerDeviceSink` and `Player` lifecycle.
pub struct RodioAudioEngine {
    /// The rodio output stream. Must live as long as the engine.
    _stream: rodio::MixerDeviceSink,
    /// Inner state protected by a mutex.
    inner: Mutex<RodioInner>,
}

struct RodioInner {
    /// Current player, if audio is loaded.
    player: Option<Player>,
    /// Total duration of the loaded audio.
    duration: f64,
    /// Current playback state.
    state: AudioPlaybackState,
    /// Volume, stored so we can apply it to new players.
    volume: f32,
}

impl RodioAudioEngine {
    /// Create a new rodio audio engine.
    ///
    /// # Errors
    ///
    /// Returns an error if no audio output device is available.
    pub fn new() -> Result<Self, Report<AudioError>> {
        let stream = DeviceSinkBuilder::open_default_sink()
            .change_context(AudioError)
            .attach("failed to get default audio output")?;

        Ok(Self {
            _stream: stream,
            inner: Mutex::new(RodioInner {
                player: None,
                duration: 0.0,
                state: AudioPlaybackState::Paused,
                volume: 1.0,
            }),
        })
    }
}

impl AudioEngine for RodioAudioEngine {
    fn name(&self) -> &'static str {
        "rodio"
    }

    fn load(&self, path: &Path) -> Result<(), Report<AudioError>> {
        let file = std::fs::File::open(path)
            .change_context(AudioError)
            .attach(path.display().to_string())?;

        let source = Decoder::try_from(file)
            .change_context(AudioError)
            .attach(path.display().to_string())?;

        let duration = source
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        let mixer = self._stream.mixer();
        let player = Player::connect_new(mixer);
        player.append(source);
        player.pause(); // Start paused — user must call play().
        player.set_volume(self.inner.lock().unwrap().volume);

        let mut inner = self.inner.lock().unwrap();
        // Stop and drop the old player.
        if let Some(old) = inner.player.take() {
            old.stop();
        }
        inner.player = Some(player);
        inner.duration = duration;
        inner.state = AudioPlaybackState::Paused;

        info!("audio loaded: {}", path.display());

        Ok(())
    }

    fn play(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(player) = &inner.player
            && inner.state != AudioPlaybackState::Playing
        {
            player.play();
            inner.state = AudioPlaybackState::Playing;
            debug!("audio play: position={}", self.position());
        }
    }

    fn pause(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.state == AudioPlaybackState::Playing
            && let Some(player) = &inner.player
        {
            player.pause();
            inner.state = AudioPlaybackState::Paused;
            debug!("audio pause: position={}", self.position());
        }
    }

    fn seek(&self, time: f64) -> Result<(), Report<AudioError>> {
        let inner = self.inner.lock().unwrap();
        if let Some(player) = &inner.player {
            let clamped = time.clamp(0.0, inner.duration);
            player
                .try_seek(std::time::Duration::from_secs_f64(clamped))
                .map_err(|e| match e {
                    SeekError::NotSupported { .. } => {
                        Report::new(AudioError).attach("source does not support seeking")
                    }
                    _ => Report::new(AudioError).attach("seek failed"),
                })?;
        }
        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        let mut inner = self.inner.lock().unwrap();
        inner.volume = clamped;
        if let Some(player) = &inner.player {
            player.set_volume(clamped as rodio::Float);
        }
    }

    fn position(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        match &inner.player {
            Some(player) => player.get_pos().as_secs_f64().min(inner.duration),
            None => 0.0,
        }
    }

    fn duration(&self) -> f64 {
        self.inner.lock().unwrap().duration
    }

    fn state(&self) -> AudioPlaybackState {
        self.inner.lock().unwrap().state
    }
}
