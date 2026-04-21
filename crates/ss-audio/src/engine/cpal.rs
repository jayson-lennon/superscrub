//! cpal-based audio output engine.
//!
//! Uses a raw audio callback to read from decoded sample buffers.
//! A global time counter (`AtomicU64` storing `f64` seconds as bits)
//! drives playback position, enabling sample-accurate seeking and
//! multi-track mixing.
//!
//! # Architecture
//!
//! The engine keeps all decoded audio clips in memory (`Vec<LoadedAudioClip>`).
//! A cpal output stream runs continuously; the audio callback checks a `playing`
//! flag and either mixes active clips (applying per-clip + master volume) or
//! outputs silence. Playback position is tracked by an atomic time counter
//! (seconds stored as `f64` bits in `AtomicU64`) that the callback advances
//! on every invocation.
//!
//! # Multi-clip mixing
//!
//! Each clip has a `[start_time, end_time)` range on the timeline. The
//! callback only mixes clips whose range contains the current position.
//! Multiple active clips are summed (additive mixing) with per-clip volume
//! applied independently.
//!
//! # Real-time safety
//!
//! The audio callback acquires a `parking_lot::RwLock` read lock on the
//! clip list. `parking_lot::RwLock` performs no allocations or syscalls
//! on uncontended paths, making it suitable for real-time audio.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use error_stack::{Report, ResultExt};
use tracing::trace;

use crate::decoder;
use crate::engine::{AudioClipInfo, AudioEngine, AudioError, AudioPlaybackState, LoadedAudioClip};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Failed to initialize the cpal audio engine.
#[derive(Debug, wherror::Error)]
#[error("failed to initialize cpal audio engine")]
pub struct CpalInitError;

/// State shared between the main thread and the cpal audio callback.
///
/// Uses `parking_lot::RwLock` for the clip list (real-time safe on
/// uncontended reads) and atomics for playback control to avoid lock
/// contention with the audio callback.
struct SharedState {
    /// Loaded audio clips, each with timeline metadata.
    clips: parking_lot::RwLock<Vec<LoadedAudioClip>>,

    /// Current playback position in seconds, stored as `f64::to_bits()`.
    /// Updated by the audio callback (advance) and `seek()` (main thread).
    position_secs_bits: AtomicU64,

    /// Whether playback is active.
    playing: AtomicBool,

    /// Volume in [0.0, 1.0], stored via `f32::to_bits()` for atomic access.
    volume: AtomicU32,

    /// Output sample rate (immutable after construction).
    output_sample_rate: u32,

    /// Output channel count (immutable after construction).
    output_channels: u16,
}

impl SharedState {
    fn new(output_sample_rate: u32, output_channels: u16) -> Self {
        Self {
            clips: parking_lot::RwLock::new(Vec::new()),
            position_secs_bits: AtomicU64::new(0.0f64.to_bits()),
            playing: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            output_sample_rate,
            output_channels,
        }
    }

    fn read_position_secs(&self) -> f64 {
        f64::from_bits(self.position_secs_bits.load(Ordering::Relaxed))
    }

    fn store_position_secs(&self, secs: f64) {
        self.position_secs_bits
            .store(secs.to_bits(), Ordering::Relaxed);
    }
}

/// Audio callback that mixes active clips into the output buffer.
///
/// Called by cpal on the audio thread. Reads the current position (seconds),
/// iterates over all loaded clips, and mixes those whose `[start_time, end_time)`
/// range contains the current position. Per-clip and master volumes are applied.
/// Outputs silence when paused or when no clips are active.
fn audio_callback(output: &mut [f32], state: &SharedState) {
    let clips = state.clips.read();

    if clips.is_empty() || !state.playing.load(Ordering::Relaxed) {
        output.fill(0.0);
        return;
    }

    let pos_secs = state.read_position_secs();

    // Find maximum end time to detect when all clips are done.
    let max_end_time = clips.iter().map(|c| c.end_time).fold(0.0f64, f64::max);

    if pos_secs >= max_end_time {
        state.playing.store(false, Ordering::Relaxed);
        output.fill(0.0);
        return;
    }

    let master_volume = f32::from_bits(state.volume.load(Ordering::Relaxed));

    output.fill(0.0f32);

    for clip in clips.iter() {
        // Skip clips not active at the current position.
        if pos_secs < clip.start_time || pos_secs >= clip.end_time {
            continue;
        }

        let sample_rate = clip.audio.sample_rate as f64;
        let channels = clip.audio.channels as usize;

        // Time offset within this clip's timeline.
        let clip_offset_secs = pos_secs - clip.start_time;
        let sample_offset = (clip_offset_secs * sample_rate * channels as f64) as usize;

        let samples = &clip.audio.samples;
        if sample_offset >= samples.len() {
            continue; // This clip's samples are exhausted.
        }

        let available = (samples.len() - sample_offset).min(output.len());
        let clip_volume = clip.volume * master_volume;

        // Mix (add) into output buffer.
        for i in 0..available {
            output[i] += samples[sample_offset + i] * clip_volume;
        }
    }

    // Advance position by the duration of this buffer.
    let buffer_duration_secs =
        output.len() as f64 / (state.output_sample_rate as f64 * state.output_channels as f64);
    let new_pos_secs = pos_secs + buffer_duration_secs;
    state.store_position_secs(new_pos_secs);
}

/// cpal-based audio output engine.
///
/// Implements [`AudioEngine`] using a cpal audio callback that mixes
/// multiple clips from a shared clip list. The audio callback is the
/// authoritative source of the current playback position, tracked via
/// an atomic time counter (seconds stored as `f64` bits).
///
/// All audio is decoded up-front into memory on `load()` / `load_clips()`.
/// This is a known trade-off acceptable for preview playback.
///
/// # Errors
///
/// [`CpalAudioEngine::new`] returns [`CpalInitError`] if no audio output
/// device is available or the device does not support f32 sample format.
pub struct CpalAudioEngine {
    /// Shared state between main thread and audio callback.
    state: Arc<SharedState>,
    /// The device sample rate, used to decode audio at the correct rate.
    sample_rate: u32,
    /// The output channel count, used to remix clips on load.
    output_channels: u16,
    /// cpal output stream. Kept alive for the lifetime of the engine.
    _stream: cpal::Stream,
}

impl CpalAudioEngine {
    /// Create a new cpal audio engine using the default output device.
    ///
    /// Queries the device's sample rate and channel count so that
    /// subsequent `load()` / `load_clips()` calls decode and remix
    /// audio to match the output configuration.
    ///
    /// # Errors
    ///
    /// Returns [`CpalInitError`] if:
    /// - No default output device is available
    /// - The device's default config cannot be queried
    /// - The device does not support f32 sample format
    /// - The output stream cannot be built
    pub fn new() -> Result<Self, Report<CpalInitError>> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| Report::new(CpalInitError).attach("no output device available"))?;

        let supported_config = device
            .default_output_config()
            .change_context(CpalInitError)
            .attach("failed to query default output config")?;

        if supported_config.sample_format() != cpal::SampleFormat::F32 {
            return Err(
                Report::new(CpalInitError).attach("device does not support f32 sample format")
            );
        }

        let sample_rate = supported_config.sample_rate();
        let config = supported_config.config();
        let output_channels = config.channels;
        let state = Arc::new(SharedState::new(sample_rate, output_channels));

        let state_clone = state.clone();
        let stream = device
            .build_output_stream(
                &config,
                move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    audio_callback(output, &state_clone);
                },
                |err| {
                    tracing::error!("cpal audio error: {err}");
                },
                None,
            )
            .change_context(CpalInitError)
            .attach("failed to build output stream")?;

        // Keep the stream running. The callback outputs silence when paused
        // or when no audio is loaded.
        stream
            .play()
            .change_context(CpalInitError)
            .attach("failed to start output stream")?;

        Ok(Self {
            state,
            sample_rate,
            output_channels,
            _stream: stream,
        })
    }
}

impl AudioEngine for CpalAudioEngine {
    fn name(&self) -> &'static str {
        "cpal"
    }

    fn load(&self, path: &Path) -> Result<(), Report<AudioError>> {
        trace!(path = %path.display(), "loading audio file");

        let decoded = decoder::decode_file_with_sample_rate(path, self.sample_rate)
            .change_context(AudioError)
            .attach(format!("path: {}", path.display()))?;
        let decoded = decoder::remix_channels(decoded, self.output_channels);

        let duration = decoded.duration;
        let clip = LoadedAudioClip {
            audio: decoded,
            start_time: 0.0,
            end_time: duration,
            volume: 1.0,
        };

        {
            let mut clips = self.state.clips.write();
            *clips = vec![clip];
        }

        self.state.store_position_secs(0.0);
        self.state.playing.store(false, Ordering::Relaxed);

        Ok(())
    }

    fn load_clips(&self, clips: &[AudioClipInfo]) -> Result<(), Report<AudioError>> {
        let mut loaded = Vec::with_capacity(clips.len());
        for info in clips {
            trace!(path = %info.path.display(), "loading audio clip");
            let decoded = decoder::decode_file_with_sample_rate(&info.path, self.sample_rate)
                .change_context(AudioError)
                .attach(format!("audio path: {}", info.path.display()))?;
            let decoded = decoder::remix_channels(decoded, self.output_channels);
            loaded.push(LoadedAudioClip {
                audio: decoded,
                start_time: info.start_time,
                end_time: info.end_time,
                volume: info.volume,
            });
        }

        {
            let mut state_clips = self.state.clips.write();
            *state_clips = loaded;
        }

        self.state.store_position_secs(0.0);
        self.state.playing.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn play(&self) {
        let clips = self.state.clips.read();
        if !clips.is_empty() {
            self.state.playing.store(true, Ordering::Relaxed);
        }
    }

    fn pause(&self) {
        self.state.playing.store(false, Ordering::Relaxed);
    }

    fn seek(&self, time: f64) -> Result<(), Report<AudioError>> {
        let clips = self.state.clips.read();
        let duration = clips.iter().map(|c| c.end_time).fold(0.0f64, f64::max);
        drop(clips);
        let clamped = time.clamp(0.0, duration);
        self.state.store_position_secs(clamped);
        Ok(())
    }

    fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        self.state
            .volume
            .store(clamped.to_bits(), Ordering::Relaxed);
    }

    fn position(&self) -> f64 {
        self.state.read_position_secs()
    }

    fn duration(&self) -> f64 {
        let clips = self.state.clips.read();
        if clips.is_empty() {
            return 0.0;
        }
        clips.iter().map(|c| c.end_time).fold(0.0f64, f64::max)
    }

    fn state(&self) -> AudioPlaybackState {
        if self.state.playing.load(Ordering::Relaxed) {
            AudioPlaybackState::Playing
        } else {
            AudioPlaybackState::Paused
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::decoder::{DecodedAudio, InterleavedSamples};

    // Helper: create DecodedAudio directly (no file I/O needed).
    fn make_decoded_audio(samples: Vec<f32>, channels: u16, sample_rate: u32) -> DecodedAudio {
        let frame_count = samples.len() / channels as usize;
        let duration = frame_count as f64 / sample_rate as f64;
        DecodedAudio {
            samples: InterleavedSamples(samples),
            channels,
            sample_rate,
            duration,
        }
    }

    /// Helper: build a `LoadedAudioClip` with the given samples and timeline range.
    fn make_clip(
        samples: Vec<f32>,
        channels: u16,
        sample_rate: u32,
        start_time: f64,
        end_time: f64,
        volume: f32,
    ) -> LoadedAudioClip {
        LoadedAudioClip {
            audio: make_decoded_audio(samples, channels, sample_rate),
            start_time,
            end_time,
            volume,
        }
    }

    /// Generate a minimal valid WAV file (1 second, 440 Hz sine, mono, 44100 Hz, 16-bit).
    fn generate_test_wav(dir: &std::path::Path) -> std::path::PathBuf {
        let sample_rate: u32 = 44100;
        let channels: u16 = 1;
        let duration_secs: f64 = 1.0;
        let num_samples = (sample_rate as f64 * duration_secs) as usize;

        let samples: Vec<i16> = (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate as f64;
                (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * i16::MAX as f64 * 0.5) as i16
            })
            .collect();

        let data_size = num_samples * 2;
        let file_size = 36 + data_size;

        let path = dir.join("test.wav");
        let mut file = std::fs::File::create(&path).unwrap();

        file.write_all(b"RIFF").unwrap();
        file.write_all(&(file_size as u32).to_le_bytes()).unwrap();
        file.write_all(b"WAVE").unwrap();

        file.write_all(b"fmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        let byte_rate = sample_rate * channels as u32 * 2;
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        let block_align = channels * 2;
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap();

        file.write_all(b"data").unwrap();
        file.write_all(&(data_size as u32).to_le_bytes()).unwrap();
        for sample in &samples {
            file.write_all(&sample.to_le_bytes()).unwrap();
        }

        path
    }

    /// Create a CpalAudioEngine, returning None (to skip the test) if no
    /// audio device is available.
    fn create_engine_or_skip() -> Option<CpalAudioEngine> {
        match CpalAudioEngine::new() {
            Ok(engine) => Some(engine),
            Err(e) => {
                eprintln!("skipping test: no audio device available: {e:?}");
                None
            }
        }
    }

    // ================================================================
    // Callback unit tests (no audio device needed)
    // ================================================================

    #[test]
    fn callback_fills_silence_when_no_clips_loaded() {
        // Given a SharedState with no clips loaded.
        let state = SharedState::new(44100, 1);
        let mut output = vec![1.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then all output samples are silence.
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn callback_fills_silence_when_paused() {
        // Given a SharedState with a clip loaded but not playing.
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(vec![0.5; 1024], 1, 44100, 0.0, 1.0, 1.0)];
        let mut output = vec![1.0f32; 256];

        // When the audio callback is invoked while paused.
        audio_callback(&mut output, &state);

        // Then all output samples are silence.
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn callback_fills_silence_when_past_end() {
        // Given a SharedState with position past all clips' end times.
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(vec![0.5; 100], 1, 44100, 0.0, 0.01, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        state.store_position_secs(1.0); // past end_time of 0.01
        let mut output = vec![1.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then all output is silence and playing is stopped.
        assert!(output.iter().all(|&s| s == 0.0));
        assert!(!state.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn callback_copies_samples_when_playing() {
        // Given a SharedState with a single playing clip.
        let samples: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(samples.clone(), 1, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the first 256 samples are copied to the output.
        assert_eq!(&output[..], &samples[..256]);
    }

    #[test]
    fn callback_applies_volume() {
        // Given a SharedState with master volume at 50%.
        let samples: Vec<f32> = (0..512).map(|i| i as f32 / 512.0).collect();
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(samples.clone(), 1, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        state.volume.store(0.5f32.to_bits(), Ordering::Relaxed);
        let mut output = vec![0.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then each sample is scaled by 0.5 (master * clip volume = 0.5 * 1.0).
        for (i, &s) in output.iter().enumerate() {
            let expected = samples[i] * 0.5;
            assert!(
                (s - expected).abs() < f32::EPSILON,
                "sample {i}: {s} != {expected}"
            );
        }
    }

    #[test]
    fn callback_fills_remainder_with_silence_at_end() {
        // Given a SharedState with fewer remaining samples than the output buffer.
        let state = SharedState::new(44100, 1);
        // 10 samples, mono, 44100 Hz → duration ≈ 0.00023s
        *state.clips.write() = vec![make_clip(vec![0.7f32; 10], 1, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        // Position at sample 5 → 5 / 44100 ≈ 0.000113s
        state.store_position_secs(5.0 / 44100.0);
        let mut output = vec![1.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the first 5 samples are audio and the rest is silence.
        assert!(output[..5].iter().all(|&s| (s - 0.7).abs() < f32::EPSILON));
        assert!(output[5..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn callback_advances_position() {
        // Given a SharedState at position 0 with playing audio.
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(vec![0.5; 1024], 1, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        assert_eq!(state.read_position_secs(), 0.0);
        let mut output = vec![0.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the position advanced by 256 mono samples at 44100 Hz.
        let expected_advance = 256.0 / 44100.0;
        let pos = state.read_position_secs();
        assert!(
            (pos - expected_advance).abs() < f64::EPSILON,
            "position should be {expected_advance}, got {pos}"
        );
    }

    #[test]
    fn callback_stops_playing_at_end() {
        // Given a SharedState where position will reach the end exactly.
        let state = SharedState::new(44100, 1);
        *state.clips.write() = vec![make_clip(vec![0.5; 256], 1, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 256];

        // When the audio callback is invoked (consuming all 256 samples).
        audio_callback(&mut output, &state);

        // Then all samples are output.
        assert!(output.iter().all(|&s| (s - 0.5).abs() < f32::EPSILON));

        // And the next callback outputs silence and stops playing
        // (position now past clip end_time since clip.duration < 1.0
        // but end_time = 1.0 — callback consumes 256 samples advancing
        // to 256/44100 ≈ 0.0058s, which is < 1.0, so it keeps playing).
        // For auto-stop to trigger, we need position >= end_time.
        // Advance position to past end_time.
        state.store_position_secs(1.0);
        let mut output2 = vec![1.0f32; 256];
        audio_callback(&mut output2, &state);
        assert!(output2.iter().all(|&s| s == 0.0));
        assert!(!state.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn callback_handles_stereo() {
        // Given stereo audio (L R interleaved).
        let samples: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 3 frames
        let state = SharedState::new(44100, 2);
        *state.clips.write() = vec![make_clip(samples.clone(), 2, 44100, 0.0, 1.0, 1.0)];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 6];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then all interleaved samples are copied in order.
        assert_eq!(&output[..], &samples[..]);
    }

    // ================================================================
    // Multi-clip callback tests
    // ================================================================

    #[test]
    fn callback_mixes_two_overlapping_clips() {
        // Given two clips active at the same time.
        let state = SharedState::new(44100, 1);
        let clip_a = make_clip(vec![1.0; 256], 1, 44100, 0.0, 1.0, 1.0);
        let clip_b = make_clip(vec![0.5; 256], 1, 44100, 0.0, 1.0, 1.0);
        *state.clips.write() = vec![clip_a, clip_b];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 128];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the output is the sum of both clips (1.0 + 0.5 = 1.5).
        assert!(output.iter().all(|&s| (s - 1.5).abs() < f32::EPSILON));
    }

    #[test]
    fn callback_skips_clip_before_start_time() {
        // Given a clip that starts at 1.0s and position at 0.0s.
        let state = SharedState::new(44100, 1);
        let clip = make_clip(vec![1.0; 1024], 1, 44100, 1.0, 2.0, 1.0);
        *state.clips.write() = vec![clip];
        state.playing.store(true, Ordering::Relaxed);
        state.store_position_secs(0.0);
        let mut output = vec![0.0f32; 128];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the output is silence (clip not yet active).
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn callback_skips_clip_after_end_time() {
        // Given a clip ending at 0.5s and position at 0.5s.
        let state = SharedState::new(44100, 1);
        let clip = make_clip(vec![1.0; 1024], 1, 44100, 0.0, 0.5, 1.0);
        *state.clips.write() = vec![clip];
        state.playing.store(true, Ordering::Relaxed);
        state.store_position_secs(0.5);
        let mut output = vec![0.0f32; 128];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the output is silence (clip no longer active, position >= end_time).
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn callback_applies_per_clip_volume() {
        // Given two clips with different volumes.
        let state = SharedState::new(44100, 1);
        let clip_a = make_clip(vec![1.0; 256], 1, 44100, 0.0, 1.0, 0.8);
        let clip_b = make_clip(vec![1.0; 256], 1, 44100, 0.0, 1.0, 0.2);
        *state.clips.write() = vec![clip_a, clip_b];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 128];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the output is (1.0 * 0.8) + (1.0 * 0.2) = 1.0.
        assert!(output.iter().all(|&s| (s - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn callback_auto_stops_when_all_clips_end() {
        // Given two clips that both end before position.
        let state = SharedState::new(44100, 1);
        let clip_a = make_clip(vec![0.5; 256], 1, 44100, 0.0, 0.5, 1.0);
        let clip_b = make_clip(vec![0.3; 256], 1, 44100, 0.3, 0.8, 1.0);
        *state.clips.write() = vec![clip_a, clip_b];
        state.playing.store(true, Ordering::Relaxed);
        state.store_position_secs(1.0); // past both end_times
        let mut output = vec![0.0f32; 128];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then output is silence and playing stopped.
        assert!(output.iter().all(|&s| s == 0.0));
        assert!(!state.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn callback_single_clip_behaves_as_before() {
        // Given a single clip with start_time=0, end_time=duration, volume=1.0.
        let samples: Vec<f32> = (0..512).map(|i| i as f32 / 512.0).collect();
        let decoded = make_decoded_audio(samples.clone(), 1, 44100);
        let state = SharedState::new(44100, 1);
        let clip = LoadedAudioClip {
            audio: decoded.clone(),
            start_time: 0.0,
            end_time: decoded.duration,
            volume: 1.0,
        };
        *state.clips.write() = vec![clip];
        state.playing.store(true, Ordering::Relaxed);
        let mut output = vec![0.0f32; 256];

        // When the audio callback is invoked.
        audio_callback(&mut output, &state);

        // Then the output matches the raw samples (same as old single-clip path).
        assert_eq!(&output[..], &samples[..256]);
    }

    #[test]
    fn callback_handles_different_start_times() {
        // Given two clips: clip_a starts at 0s, clip_b starts at 0.5s.
        // Position is at 0.0s.
        let state = SharedState::new(44100, 1);
        let clip_a = make_clip(vec![1.0; 2048], 1, 44100, 0.0, 2.0, 1.0);
        let clip_b = make_clip(vec![2.0; 2048], 1, 44100, 0.5, 2.0, 1.0);
        *state.clips.write() = vec![clip_a, clip_b];
        state.playing.store(true, Ordering::Relaxed);
        state.store_position_secs(0.0);
        let mut output = vec![0.0f32; 128];

        // When the callback is invoked at position 0.0.
        audio_callback(&mut output, &state);

        // Then only clip_a contributes (clip_b hasn't started).
        assert!(output.iter().all(|&s| (s - 1.0).abs() < f32::EPSILON));
    }

    // ================================================================
    // Integration tests (audio device needed, skip in CI)
    // ================================================================

    #[test]
    fn name_returns_cpal() {
        // Given a CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // Then the name is "cpal".
        assert_eq!(engine.name(), "cpal");
    }

    #[test]
    fn initial_state_is_paused() {
        // Given a new CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // Then the initial state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn initial_position_is_zero() {
        // Given a new CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // Then the initial position is 0.0.
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn initial_duration_is_zero() {
        // Given a new CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // Then the initial duration is 0.0.
        assert_eq!(engine.duration(), 0.0);
    }

    #[test]
    fn load_sets_state_to_paused() {
        // Given a CpalAudioEngine and a valid WAV file.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());

        // When loading the audio file.
        engine.load(&wav_path).unwrap();

        // Then the state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn load_resets_position_to_zero() {
        // Given a loaded engine with a seeked position.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();
        engine.seek(0.5).unwrap();

        // When loading a new file.
        engine.load(&wav_path).unwrap();

        // Then the position is reset to 0.0.
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn load_sets_duration() {
        // Given a CpalAudioEngine and a valid 1-second WAV file.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());

        // When loading the audio file.
        engine.load(&wav_path).unwrap();

        // Then the duration is approximately 1.0 second.
        let dur = engine.duration();
        assert!(
            (dur - 1.0).abs() < 0.05,
            "duration should be ~1.0s, got {dur}"
        );
    }

    #[test]
    fn play_sets_state_to_playing() {
        // Given a CpalAudioEngine with audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();

        // When playing.
        engine.play();

        // Then the state is Playing.
        assert_eq!(engine.state(), AudioPlaybackState::Playing);
    }

    #[test]
    fn play_is_noop_when_nothing_loaded() {
        // Given a CpalAudioEngine with no audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // When playing.
        engine.play();

        // Then the state remains Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn pause_sets_state_to_paused() {
        // Given a playing CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();
        engine.play();

        // When pausing.
        engine.pause();

        // Then the state is Paused.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn seek_updates_position() {
        // Given a CpalAudioEngine with 1-second audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();

        // When seeking to 0.5 seconds.
        engine.seek(0.5).unwrap();

        // Then the position is approximately 0.5 seconds.
        let pos = engine.position();
        assert!(
            (pos - 0.5).abs() < 0.01,
            "position should be ~0.5s, got {pos}"
        );
    }

    #[test]
    fn seek_clamps_to_zero() {
        // Given a CpalAudioEngine with audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();

        // When seeking to a negative time.
        engine.seek(-5.0).unwrap();

        // Then the position is clamped to 0.0.
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn seek_clamps_to_duration() {
        // Given a CpalAudioEngine with 1-second audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();
        let dur = engine.duration();

        // When seeking past the duration.
        engine.seek(100.0).unwrap();

        // Then the position is clamped to the duration.
        let pos = engine.position();
        assert!(
            (pos - dur).abs() < 0.01,
            "position should be ~{dur}s, got {pos}"
        );
    }

    #[test]
    fn seek_is_noop_when_nothing_loaded() {
        // Given a CpalAudioEngine with no audio loaded.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // When seeking.
        let result = engine.seek(5.0);

        // Then it succeeds and position remains 0.
        assert!(result.is_ok());
        assert_eq!(engine.position(), 0.0);
    }

    #[test]
    fn set_volume_is_accepted() {
        // Given a CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // When setting volume to 50%.
        engine.set_volume(0.5);

        // Then the engine accepts the call without error (observable: no panic).
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn set_volume_clamps_above_one() {
        // Given a CpalAudioEngine.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };

        // When setting volume above 1.0.
        engine.set_volume(2.0);

        // Then the engine accepts the call without error.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        // Given a CpalAudioEngine and a nonexistent file path.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let path = std::path::Path::new("/nonexistent/audio.wav");

        // When loading the file.
        let result = engine.load(path);

        // Then an error is returned.
        assert!(result.is_err());
    }

    #[test]
    fn position_advances_after_playback() {
        // Given a CpalAudioEngine with 1-second audio loaded and playing.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();
        assert_eq!(engine.position(), 0.0);

        // When playing for 200ms.
        engine.play();
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Then the position has advanced past 0.
        let pos = engine.position();
        assert!(pos > 0.0, "position should have advanced past 0, got {pos}");
    }

    #[test]
    fn pause_and_seek_achieves_stop() {
        // Given a playing engine with a seek position.
        let Some(engine) = create_engine_or_skip() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        engine.load(&wav_path).unwrap();
        engine.play();
        std::thread::sleep(std::time::Duration::from_millis(50));
        engine.seek(0.5).unwrap();

        // When pausing and seeking to 0.
        engine.pause();
        engine.seek(0.0).unwrap();

        // Then state is Paused and position is 0.
        assert_eq!(engine.state(), AudioPlaybackState::Paused);
        assert_eq!(engine.position(), 0.0);
    }
}
