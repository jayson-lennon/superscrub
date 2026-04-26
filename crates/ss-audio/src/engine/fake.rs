//! Fake audio engine for testing.
//!
//! Tracks calls and state without requiring audio hardware.
//! All position/state tracking is simulated.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use tracing::trace;

use crate::engine::{AudioClipInfo, AudioEngine, AudioError, AudioPlaybackState};

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
    /// Number of times `load_clips` has been called.
    pub load_clips_count: AtomicUsize,
    /// The clips passed to the last `load_clips` call.
    pub last_loaded_clips: Mutex<Vec<AudioClipInfo>>,
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
            load_clips_count: AtomicUsize::new(0),
            last_loaded_clips: Mutex::new(Vec::new()),
        }
    }

    /// Create a fake that reports the given duration, as if audio is loaded.
    pub fn with_duration(duration: std::time::Duration) -> Self {
        let state = FakeState {
            duration: duration.as_secs_f64(),
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
            load_clips_count: AtomicUsize::new(0),
            last_loaded_clips: Mutex::new(Vec::new()),
        }
    }

    /// Set the playback state directly (for testing auto-stop scenarios).
    pub fn set_state(&self, state: AudioPlaybackState) {
        self.state.lock().unwrap().playback = state;
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

    fn load_clips(&self, clips: &[AudioClipInfo]) -> Result<(), Report<AudioError>> {
        trace!("fake audio operation: load_clips");
        self.load_clips_count.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        *self.last_loaded_clips.lock().unwrap() = clips.to_vec();
        state.loaded = true;
        state.playback = AudioPlaybackState::Paused;
        state.position = 0.0;
        // Use the max end_time as the fake's duration.
        state.duration = clips.iter().map(|c| c.end_time).fold(0.0, f64::max);
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

    fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(self.state.lock().unwrap().duration)
    }

    fn state(&self) -> AudioPlaybackState {
        self.state.lock().unwrap().playback
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::super::AudioEngine;
    use super::super::AudioPlaybackState;
    use super::FakeAudioEngine;
    use crate::engine::service::AudioEngineService;

    #[test]
    fn name_returns_fake() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();

        // Then the name is "fake".
        assert_eq!(engine.name(), "fake");
    }

    #[test]
    fn initial_state_is_paused() {
        // Given a new fake engine.
        let engine = FakeAudioEngine::new();

        // Then the initial state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn initial_position_is_zero() {
        // Given a new fake engine.
        let engine = FakeAudioEngine::new();

        // Then the initial position is 0.0.
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn initial_duration_is_zero() {
        // Given a new fake engine.
        let engine = FakeAudioEngine::new();

        // Then the initial duration is zero.
        assert_eq!(engine.duration(), std::time::Duration::ZERO);
    }

    #[test]
    fn load_sets_state_to_paused() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();

        // When loading an audio file.
        engine.load(std::path::Path::new("test.wav")).unwrap();

        // Then the state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn load_stores_path() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();

        // When loading an audio file.
        engine.load(std::path::Path::new("test.wav")).unwrap();

        // Then the last loaded path matches.
        let path = engine.last_loaded_path.lock().unwrap();
        assert_eq!(*path, Some("test.wav".to_string()));
    }

    #[test]
    fn load_increments_count() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();

        // When loading an audio file.
        engine.load(std::path::Path::new("test.wav")).unwrap();

        // Then load_count is 1.
        assert_eq!(engine.load_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn play_transitions_to_playing() {
        // Given a fake engine with audio loaded.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When playing.
        engine.play();

        // Then the state is Playing.
        assert_eq!(engine.state(), AudioPlaybackState::Playing);
    }

    #[test]
    fn play_increments_count() {
        // Given a fake engine with audio loaded.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When playing.
        engine.play();

        // Then play_count is 1.
        assert_eq!(engine.play_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn pause_transitions_from_playing_to_paused() {
        // Given a playing engine.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));
        engine.play();

        // When pausing.
        engine.pause();

        // Then the state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn pause_is_noop_when_already_paused() {
        // Given a paused engine.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When pausing while already paused.
        engine.pause();

        // Then pause_count incremented but state is still Paused.
        assert_eq!(engine.pause_count.load(Ordering::SeqCst), 1);
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn seek_updates_position() {
        // Given a fake engine with audio loaded.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When seeking to 5.0 seconds.
        engine.seek(5.0).unwrap();

        // Then the position is 5.0.
        assert_eq!(engine.position(), 5.0);
    }

    #[test]
    fn seek_clamps_to_duration() {
        // Given a fake engine with 30s audio.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When seeking past the duration.
        engine.seek(100.0).unwrap();

        // Then the position is clamped to 30.0.
        assert_eq!(engine.position(), 30.0);
    }

    #[test]
    fn seek_clamps_to_zero() {
        // Given a fake engine with audio loaded.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // When seeking to a negative time.
        engine.seek(-5.0).unwrap();

        // Then the position is clamped to 0.0.
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn set_volume_above_one_does_not_panic() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();

        // When setting volume above 1.0.
        engine.set_volume(2.0);
        engine.seek(0.0).unwrap();

        // Then the engine accepts the call without error (observable: no panic).
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn with_duration_reports_duration() {
        // Given a fake engine created with_duration(30s).
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));

        // Then the duration is 30.0 seconds.
        assert_eq!(engine.duration().as_secs_f64(), 30.0);
    }

    #[test]
    fn play_is_noop_when_not_loaded() {
        // Given a new fake engine (no audio loaded).
        let engine = FakeAudioEngine::new();

        // When playing.
        engine.play();

        // Then play_count is incremented (the method was called).
        assert_eq!(engine.play_count.load(Ordering::SeqCst), 1);
        // And state remains Paused since nothing is loaded.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn pause_and_seek_achieves_stop() {
        // Given a playing engine with a seek position.
        let engine = FakeAudioEngine::with_duration(std::time::Duration::from_secs_f64(30.0));
        engine.play();
        engine.seek(5.0).unwrap();

        // When pausing and seeking to 0.
        engine.pause();
        engine.seek(0.0).unwrap();

        // Then state is Paused and position is 0.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn service_wraps_engine_and_clones() {
        // Given a fake engine wrapped in a service.
        let service = AudioEngineService::new(Arc::new(FakeAudioEngine::new()));

        // Then the service exists and can be cloned.
        let _cloned = service.clone();
    }

    // ================================================================
    // load_clips tests
    // ================================================================

    #[test]
    fn load_clips_increments_count() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();
        let clips = vec![crate::engine::AudioClipInfo {
            path: std::path::PathBuf::from("a.wav"),
            start_time: 0.0,
            end_time: 10.0,
            volume: 1.0,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![],
        }];

        // When calling load_clips().
        engine.load_clips(&clips).unwrap();

        // Then load_clips_count is 1.
        assert_eq!(engine.load_clips_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn load_clips_stores_clip_infos() {
        // Given a fake engine.
        let engine = FakeAudioEngine::new();
        let clips = vec![
            crate::engine::AudioClipInfo {
                path: std::path::PathBuf::from("a.wav"),
                start_time: 0.0,
                end_time: 10.0,
                volume: 0.8,
                source_offset: 0.0,
                trim_end: 0.0,
                animations: vec![],
            },
            crate::engine::AudioClipInfo {
                path: std::path::PathBuf::from("b.wav"),
                start_time: 5.0,
                end_time: 15.0,
                volume: 0.5,
                source_offset: 0.0,
                trim_end: 0.0,
                animations: vec![],
            },
        ];

        // When calling load_clips().
        engine.load_clips(&clips).unwrap();

        // Then the clips are stored.
        let stored = engine.last_loaded_clips.lock().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].path, std::path::PathBuf::from("a.wav"));
        assert_eq!(stored[1].path, std::path::PathBuf::from("b.wav"));
    }

    #[test]
    fn load_clips_duration_is_max_end_time() {
        // Given a fake engine and clips with different end times.
        let engine = FakeAudioEngine::new();
        let clips = vec![
            crate::engine::AudioClipInfo {
                path: std::path::PathBuf::from("a.wav"),
                start_time: 0.0,
                end_time: 10.0,
                volume: 1.0,
                source_offset: 0.0,
                trim_end: 0.0,
                animations: vec![],
            },
            crate::engine::AudioClipInfo {
                path: std::path::PathBuf::from("b.wav"),
                start_time: 5.0,
                end_time: 20.0,
                volume: 1.0,
                source_offset: 0.0,
                trim_end: 0.0,
                animations: vec![],
            },
        ];

        // When calling load_clips().
        engine.load_clips(&clips).unwrap();

        // Then the duration is the max end_time.
        assert_eq!(engine.duration().as_secs_f64(), 20.0);
    }
}
