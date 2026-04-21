//! Audio file decoder.
//!
//! Decodes audio files to raw f32 PCM samples using symphonia.
//! Resamples to a target sample rate using rubato.
//!
//! # Sample formats
//!
//! Internally, samples flow through two newtype wrappers:
//! - [`PlanarSamples`] — one `Vec<f32>` per channel (symphonia native, rubato input)
//! - [`InterleavedSamples`] — channels alternating in a single `Vec<f32>` (cpal output)
//!
//! Resampling operates on planar data. Interleaving is the final step.

use std::ops::Deref;
use std::path::Path;

use error_stack::{Report, ResultExt};
use rubato::{FftFixedInOut, Resampler};
use symphonia::core::audio::Signal;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// ---------------------------------------------------------------------------
// Sample newtypes
// ---------------------------------------------------------------------------

/// Planar audio samples — one `Vec<f32>` per channel.
///
/// Channel 0 is left, channel 1 is right (for stereo).
/// This is the format symphonia decodes into and rubato expects.
#[derive(Debug, Clone)]
pub struct PlanarSamples(pub Vec<Vec<f32>>);

impl Deref for PlanarSamples {
    type Target = Vec<Vec<f32>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PlanarSamples {
    /// Number of channels.
    pub fn channels(&self) -> u16 {
        self.0.len() as u16
    }

    /// Number of sample frames (samples per channel).
    pub fn frame_count(&self) -> usize {
        self.0.first().map_or(0, |ch| ch.len())
    }
}

/// Interleaved audio samples — channels alternating in a single buffer.
///
/// For stereo: `[L0, R0, L1, R1, L2, R2, ...]`.
/// This is the format cpal's output callback consumes.
#[derive(Debug, Clone)]
pub struct InterleavedSamples(pub Vec<f32>);

impl Deref for InterleavedSamples {
    type Target = Vec<f32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Convert planar samples to interleaved.
///
/// Panics if all channels don't have the same length.
impl From<PlanarSamples> for InterleavedSamples {
    fn from(planar: PlanarSamples) -> Self {
        let channels = planar.channels() as usize;
        let frames = planar.frame_count();
        let mut interleaved = Vec::with_capacity(frames * channels);

        for frame in 0..frames {
            for ch in 0..channels {
                interleaved.push(planar.0[ch][frame]);
            }
        }

        InterleavedSamples(interleaved)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Failed to decode an audio file.
#[derive(Debug, wherror::Error)]
#[error("audio decode failed")]
pub struct DecodeError;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decoded audio data, ready for playback.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Interleaved f32 samples (L R L R for stereo).
    pub samples: InterleavedSamples,
    /// Channel count (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Sample rate of the decoded (and possibly resampled) audio.
    pub sample_rate: u32,
    /// Duration in seconds.
    pub duration: f64,
}

/// Decode an audio file to interleaved f32 PCM at its native sample rate.
///
/// Reads the entire file and converts all samples to f32.
/// The output is interleaved (L R L R for stereo).
///
/// # Errors
///
/// Returns [`DecodeError`] if the file cannot be opened, the format is
/// unrecognized, or decoding fails.
pub fn decode_file(path: &Path) -> Result<DecodedAudio, Report<DecodeError>> {
    let (planar, channels, sample_rate) = decode_to_planar(path)?;
    let duration = planar.frame_count() as f64 / sample_rate as f64;
    let samples: InterleavedSamples = planar.into();

    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate,
        duration,
    })
}

/// Decode an audio file and resample to a target sample rate.
///
/// Decodes at native rate, resamples while still planar (rubato's natural
/// format), then interleaves for output.
///
/// # Errors
///
/// Returns [`DecodeError`] if decoding or resampling fails.
pub fn decode_file_with_sample_rate(
    path: &Path,
    target_sample_rate: u32,
) -> Result<DecodedAudio, Report<DecodeError>> {
    let (planar, channels, native_rate) = decode_to_planar(path)?;

    let resampled = if native_rate == target_sample_rate {
        planar
    } else {
        resample(planar, native_rate, target_sample_rate, channels)
            .change_context(DecodeError)
            .attach("resampling failed")?
    };

    let duration = resampled.frame_count() as f64 / target_sample_rate as f64;
    let samples: InterleavedSamples = resampled.into();

    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate: target_sample_rate,
        duration,
    })
}

// ---------------------------------------------------------------------------
// Channel remixing
// ---------------------------------------------------------------------------

/// Convert decoded audio to the specified channel count.
///
/// - Mono → Stereo: duplicates the channel
/// - Stereo → Mono: averages the two channels
/// - Same count: no-op (returns the input)
///
/// Returns a new [`DecodedAudio`] with the requested channel count.
#[must_use]
pub fn remix_channels(audio: DecodedAudio, target_channels: u16) -> DecodedAudio {
    if audio.channels == target_channels {
        return audio;
    }

    let samples = &audio.samples;
    let frames = samples.len() / audio.channels as usize;
    let mut remixed = Vec::with_capacity(frames * target_channels as usize);

    match (audio.channels, target_channels) {
        (1, 2) => {
            // Mono → Stereo: duplicate.
            for &s in samples.iter() {
                remixed.push(s);
                remixed.push(s);
            }
        }
        (2, 1) => {
            // Stereo → Mono: average.
            for frame in 0..frames {
                let left = samples[frame * 2];
                let right = samples[frame * 2 + 1];
                remixed.push((left + right) * 0.5);
            }
        }
        _ => {
            // Unsupported conversion — copy first available channel.
            tracing::warn!(
                "unsupported channel conversion: {} -> {}, using first channel",
                audio.channels,
                target_channels
            );
            for frame in 0..frames {
                for ch in 0..target_channels {
                    let src_ch = (ch as usize).min(audio.channels as usize - 1);
                    remixed.push(samples[frame * audio.channels as usize + src_ch]);
                }
            }
        }
    }

    DecodedAudio {
        samples: InterleavedSamples(remixed),
        channels: target_channels,
        sample_rate: audio.sample_rate,
        duration: audio.duration,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Decode an audio file to planar f32 samples at the native sample rate.
///
/// Returns `(planar_samples, channel_count, sample_rate)`.
fn decode_to_planar(path: &Path) -> Result<(PlanarSamples, u16, u32), Report<DecodeError>> {
    // 1. Open file and wrap in MediaSourceStream.
    let file = std::fs::File::open(path)
        .change_context(DecodeError)
        .attach(format!("path: {}", path.display()))?;
    let mss = MediaSourceStream::new(
        Box::new(file),
        symphonia::core::io::MediaSourceStreamOptions::default(),
    );

    // 2. Create a Hint with the file extension for format probing.
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    // 3. Probe the format to get a FormatReader.
    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .change_context(DecodeError)
        .attach("format probing failed")?;

    let mut format_reader = probed.format;

    // 4. Find the first audio track.
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| Report::new(DecodeError).attach("no supported audio track found"))?;

    let track_id = track.id;
    let codec_params = &track.codec_params;
    let channels = codec_params
        .channels
        .map(|c| c.count() as u16)
        .ok_or_else(|| Report::new(DecodeError).attach("unknown channel count"))?;
    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| Report::new(DecodeError).attach("unknown sample rate"))?;

    // 5. Create the decoder via symphonia's codec registry.
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &decoder_opts)
        .change_context(DecodeError)
        .attach("decoder creation failed")?;

    // 6. Decode all packets, converting to planar f32.
    let mut channel_data: Vec<Vec<f32>> = vec![Vec::new(); channels as usize];

    loop {
        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(Report::new(DecodeError)
                    .attach(format!("decode error: {e}"))
                    .attach("packet decoding failed"));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet)
            .change_context(DecodeError)
            .attach("frame decoding failed")?;

        // Convert whatever sample format symphonia decoded into f32 planar.
        let mut convert_buf = decoded.make_equivalent::<f32>();
        decoded.convert(&mut convert_buf);

        for (ch_idx, ch_buf) in channel_data.iter_mut().enumerate() {
            ch_buf.extend_from_slice(convert_buf.chan(ch_idx));
        }
    }

    Ok((PlanarSamples(channel_data), channels, sample_rate))
}

/// Resample planar audio from one sample rate to another.
///
/// Feeds input in chunks through rubato's FFT resampler. The last chunk uses
/// rubato's `process_partial_into_buffer` which handles zero-padding
/// internally. Output is trimmed to the expected output frame count.
fn resample(
    input: PlanarSamples,
    from_rate: u32,
    to_rate: u32,
    channels: u16,
) -> Result<PlanarSamples, Report<DecodeError>> {
    let frame_count = input.frame_count();
    let expected_output_frames =
        (frame_count as f64 * to_rate as f64 / from_rate as f64).round() as usize;

    let chunk_size = 1024;
    let mut resampler = FftFixedInOut::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        channels as usize,
    )
    .change_context(DecodeError)
    .attach("resampler creation failed")?;

    let actual_chunk_in = resampler.input_frames_next();
    let actual_chunk_out = resampler.output_frames_next();

    let mut output_channels: Vec<Vec<f32>> = vec![Vec::new(); channels as usize];
    let input_buffers = input.0;

    // Process full chunks.
    let mut pos = 0;
    while pos + actual_chunk_in <= frame_count {
        let in_bufs: Vec<Vec<f32>> = input_buffers
            .iter()
            .map(|ch| ch[pos..pos + actual_chunk_in].to_vec())
            .collect();

        let mut out_bufs: Vec<Vec<f32>> = output_channels
            .iter()
            .map(|_| vec![0.0f32; actual_chunk_out])
            .collect();

        resampler
            .process_into_buffer(&in_bufs, &mut out_bufs, None)
            .change_context(DecodeError)
            .attach("resampling failed")?;

        for (ch_idx, out_buf) in out_bufs.into_iter().enumerate() {
            output_channels[ch_idx].extend_from_slice(&out_buf);
        }

        pos += actual_chunk_in;
    }

    // Process remaining partial chunk (rubato zero-pads internally).
    if pos < frame_count {
        let in_bufs: Vec<Vec<f32>> = input_buffers.iter().map(|ch| ch[pos..].to_vec()).collect();

        let mut out_bufs: Vec<Vec<f32>> = output_channels
            .iter()
            .map(|_| vec![0.0f32; actual_chunk_out])
            .collect();

        resampler
            .process_partial_into_buffer(Some(&in_bufs), &mut out_bufs, None)
            .change_context(DecodeError)
            .attach("resampling partial chunk failed")?;

        for (ch_idx, out_buf) in out_bufs.into_iter().enumerate() {
            output_channels[ch_idx].extend_from_slice(&out_buf);
        }
    }

    // Trim output to expected length (removes zero-padded artifacts).
    for ch_buf in &mut output_channels {
        ch_buf.truncate(expected_output_frames);
    }

    Ok(PlanarSamples(output_channels))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Generate a minimal valid WAV file.
    ///
    /// Produces a 1-second, 440 Hz sine wave, mono, 44100 Hz sample rate, 16-bit PCM.
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

        let data_size = num_samples * 2; // i16 = 2 bytes
        let file_size = 36 + data_size; // RIFF header - 8

        let path = dir.join("test.wav");
        let mut file = std::fs::File::create(&path).unwrap();

        // RIFF header.
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(file_size as u32).to_le_bytes()).unwrap();
        file.write_all(b"WAVE").unwrap();

        // fmt chunk.
        file.write_all(b"fmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
        file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        let byte_rate = sample_rate * channels as u32 * 2;
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        let block_align = channels * 2;
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample

        // data chunk.
        file.write_all(b"data").unwrap();
        file.write_all(&(data_size as u32).to_le_bytes()).unwrap();
        for sample in &samples {
            file.write_all(&sample.to_le_bytes()).unwrap();
        }

        path
    }

    // ================================================================
    // decode_file tests
    // ================================================================

    #[test]
    fn decode_wav_produces_samples() {
        // Given a valid WAV file.
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());

        // When decoding it.
        let decoded = decode_file(&wav_path).unwrap();

        // Then samples are non-empty with correct metadata.
        assert!(!decoded.samples.is_empty());
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.sample_rate, 44100);
        assert!(decoded.duration > 0.0);
    }

    #[test]
    fn decode_nonexistent_file_returns_error() {
        // Given a path to a file that does not exist.
        let path = std::path::Path::new("/nonexistent/audio.wav");

        // When decoding it.
        let result = decode_file(path);

        // Then an error is returned.
        assert!(result.is_err());
    }

    #[test]
    fn decode_non_audio_file_returns_error() {
        // Given a file that is not audio.
        let dir = tempfile::tempdir().unwrap();
        let txt_path = dir.path().join("not_audio.txt");
        std::fs::write(&txt_path, b"this is not audio").unwrap();

        // When decoding it.
        let result = decode_file(&txt_path);

        // Then an error is returned.
        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    fn decode_sample_count_matches_duration() {
        // Given a decoded WAV file.
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        let decoded = decode_file(&wav_path).unwrap();

        // Then sample count ≈ channels × sample_rate × duration.
        let expected = (decoded.channels as usize)
            * (decoded.sample_rate as usize)
            * (decoded.duration as usize);
        let actual = decoded.samples.len();
        assert!(
            (actual as i64 - expected as i64).unsigned_abs() <= 1,
            "actual={actual}, expected={expected}"
        );
    }

    // ================================================================
    // decode_file_with_sample_rate tests
    // ================================================================

    #[test]
    fn resample_changes_sample_rate() {
        // Given a WAV file decoded at native 44100 Hz.
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());
        let native = decode_file(&wav_path).unwrap();
        assert_eq!(native.sample_rate, 44100);

        // When resampling to 48000 Hz.
        let resampled = decode_file_with_sample_rate(&wav_path, 48000).unwrap();

        // Then the sample rate is 48000 and duration is approximately preserved.
        assert_eq!(resampled.sample_rate, 48000);
        assert!(!resampled.samples.is_empty());
        assert!((resampled.duration - native.duration).abs() < 0.1);
    }

    #[test]
    fn resample_to_same_rate_returns_same_samples() {
        // Given a WAV file at 44100 Hz.
        let dir = tempfile::tempdir().unwrap();
        let wav_path = generate_test_wav(dir.path());

        // When resampling to 44100 Hz (same rate).
        let native = decode_file(&wav_path).unwrap();
        let resampled = decode_file_with_sample_rate(&wav_path, 44100).unwrap();

        // Then the samples are identical (no resampling occurred).
        assert_eq!(resampled.samples.0, native.samples.0);
    }

    // ================================================================
    // PlanarSamples / InterleavedSamples tests
    // ================================================================

    #[test]
    fn planar_to_interleaved_stereo() {
        // Given planar stereo data.
        let planar = PlanarSamples(vec![
            vec![1.0, 2.0, 3.0], // left
            vec![4.0, 5.0, 6.0], // right
        ]);

        // When converting to interleaved.
        let interleaved: InterleavedSamples = planar.into();

        // Then samples alternate L R L R.
        assert_eq!(interleaved.0, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn planar_to_interleaved_mono() {
        // Given planar mono data.
        let planar = PlanarSamples(vec![vec![10.0, 20.0, 30.0]]);

        // When converting to interleaved.
        let interleaved: InterleavedSamples = planar.into();

        // Then samples are unchanged (single channel).
        assert_eq!(interleaved.0, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn planar_channels_reports_count() {
        // Given planar stereo data.
        let planar = PlanarSamples(vec![vec![0.0; 100], vec![0.0; 100]]);

        // Then channels() returns 2.
        assert_eq!(planar.channels(), 2);
    }

    #[test]
    fn planar_frame_count_reports_frames() {
        // Given planar data with 100 frames.
        let planar = PlanarSamples(vec![vec![0.0; 100], vec![0.0; 100]]);

        // Then frame_count() returns 100.
        assert_eq!(planar.frame_count(), 100);
    }

    #[test]
    fn planar_frame_count_empty_is_zero() {
        // Given empty planar data.
        let planar = PlanarSamples(vec![]);

        // Then frame_count() returns 0.
        assert_eq!(planar.frame_count(), 0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn deref_allows_vec_methods() {
        // Given an InterleavedSamples wrapper.
        let samples = InterleavedSamples(vec![1.0, 2.0, 3.0]);

        // Then Vec methods are accessible via Deref.
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[1], 2.0);
    }

    // ================================================================
    // remix_channels tests
    // ================================================================

    #[test]
    #[allow(clippy::float_cmp)]
    fn remix_mono_to_stereo_duplicates_channel() {
        // Given mono audio.
        let mono = DecodedAudio {
            samples: InterleavedSamples(vec![1.0, 2.0, 3.0]),
            channels: 1,
            sample_rate: 44100,
            duration: 3.0 / 44100.0,
        };

        // When remixing to stereo.
        let stereo = remix_channels(mono, 2);

        // Then each sample is duplicated (L=R).
        assert_eq!(stereo.channels, 2);
        assert_eq!(stereo.samples.0, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
        assert_eq!(stereo.sample_rate, 44100);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn remix_stereo_to_mono_averages_channels() {
        // Given stereo audio.
        let stereo = DecodedAudio {
            samples: InterleavedSamples(vec![
                1.0, 3.0, // frame 0: L=1, R=3 → avg=2
                4.0, 6.0, // frame 1: L=4, R=6 → avg=5
            ]),
            channels: 2,
            sample_rate: 44100,
            duration: 2.0 / 44100.0,
        };

        // When remixing to mono.
        let mono = remix_channels(stereo, 1);

        // Then each frame is the average of L and R.
        assert_eq!(mono.channels, 1);
        assert_eq!(mono.samples.0, vec![2.0, 5.0]);
    }

    #[test]
    fn remix_same_channels_is_noop() {
        // Given stereo audio.
        let original = DecodedAudio {
            samples: InterleavedSamples(vec![1.0, 2.0, 3.0, 4.0]),
            channels: 2,
            sample_rate: 44100,
            duration: 2.0 / 44100.0,
        };

        // When remixing to stereo (same channel count).
        let result = remix_channels(original.clone(), 2);

        // Then the data is unchanged.
        assert_eq!(result.samples.0, original.samples.0);
        assert_eq!(result.channels, 2);
    }
}
