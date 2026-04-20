//! Fake audio engine for testing.
//!
//! Tracks calls and state without requiring audio hardware.
//! All position/state tracking is simulated.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use tracing::trace;

use crate::engine::{AudioEngine, AudioError, AudioPlaybackState};

/// Fake audio engine for testing.
///
/// Tracks calls and state without requiring audio hardware.
/// All position/state tracking is simulated.
pub struct FakeAudioEngine {
    state: Mutex<FakeState>,
    /// Number of times `load` has been called.
    pub load_count: AtomicUsize,
    /// Number of times `play` has been called.
    pub play_count: AtomicUsize,
    /// Number of times `pause` has been called.
    pub pause_count: AtomicUsize,
    /// Number of times `seek` has been called.
    pub seek_count: AtomicUsize,
    /// The path passed to the last `load` call.
    pub last_loaded_path: Mutex<Option<String>>,
}

#[derive(Default)]
struct FakeState {
    loaded: bool,
    playback: AudioPlaybackState,
    position: f64,
    duration: f64,
    volume: f32,
}

impl FakeAudioEngine {
    /// Create a new fake engine with no audio loaded.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeState::default()),
            load_count: AtomicUsize::new(0),
            play_count: AtomicUsize::new(0),
            pause_count: AtomicUsize::new(0),
            seek_count: AtomicUsize::new(0),
            last_loaded_path: Mutex::new(None),
        }
    }

    /// Create a fake that reports the given duration, as if audio is loaded.
    pub fn with_duration(duration: f64) -> Self {
        let state = FakeState {
            duration,
            loaded: true,
            ..Default::default()
        };
        Self {
            state: Mutex::new(state),
            load_count: AtomicUsize::new(0),
            play_count: AtomicUsize::new(0),
            pause_count: AtomicUsize::new(0),
            seek_count: AtomicUsize::new(0),
            last_loaded_path: Mutex::new(None),
        }
    }
}

impl Default for FakeAudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine for FakeAudioEngine {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn load(&self, path: &Path) -> Result<(), Report<AudioError>> {
        trace!("fake audio operation: load");
        self.load_count.fetch_add(1, Ordering::SeqCst);
        *self.last_loaded_path.lock().unwrap() = Some(path.display().to_string());
        let mut state = self.state.lock().unwrap();
        state.loaded = true;
        state.playback = AudioPlaybackState::Paused;
        state.position = 0.0;
        Ok(())
    }

    fn play(&self) {
        trace!("fake audio operation: play");
        self.play_count.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        if state.loaded && state.playback != AudioPlaybackState::Playing {
            state.playback = AudioPlaybackState::Playing;
        }
    }

    fn pause(&self) {
        trace!("fake audio operation: pause");
        self.pause_count.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        if state.playback == AudioPlaybackState::Playing {
            state.playback = AudioPlaybackState::Paused;
        }
    }

    fn seek(&self, time: f64) -> Result<(), Report<AudioError>> {
        trace!("fake audio operation: seek");
        self.seek_count.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        state.position = time.clamp(0.0, state.duration);
        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        self.state.lock().unwrap().volume = volume.clamp(0.0, 1.0);
    }

    fn position(&self) -> f64 {
        self.state.lock().unwrap().position
    }

    fn duration(&self) -> f64 {
        self.state.lock().unwrap().duration
    }

    fn state(&self) -> AudioPlaybackState {
        self.state.lock().unwrap().playback
    }
}
