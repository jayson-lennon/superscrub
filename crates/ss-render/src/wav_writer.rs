//! Minimal WAV file writer.
//!
//! Writes interleaved f32 samples as a 16-bit PCM WAV file.
//! Used by the render pipeline to write the mixed audio output
//! before muxing with ffmpeg.

use std::io::Write;
use std::path::Path;

use error_stack::{Report, ResultExt};

/// Failed to write a WAV file.
#[derive(Debug, wherror::Error)]
#[error("wav write failed")]
pub struct WavWriteError;

/// Write interleaved f32 samples as a 16-bit PCM WAV file.
///
/// Samples are clamped to [-1.0, 1.0] and scaled to the i16 range.
/// The output is a standard 44-byte RIFF header followed by raw PCM data.
///
/// # Arguments
///
/// * `path` - Output file path
/// * `samples` - Interleaved f32 samples
/// * `channels` - Channel count (1 = mono, 2 = stereo)
/// * `sample_rate` - Sample rate in Hz
///
/// # Errors
///
/// Returns [`WavWriteError`] if the file cannot be created or written.
pub fn write_wav(
    path: &Path,
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
) -> Result<(), Report<WavWriteError>> {
    let mut file = std::fs::File::create(path)
        .change_context(WavWriteError)
        .attach("failed to create WAV file")?;

    let num_samples = samples.len();
    let data_size = (num_samples * 2) as u32; // i16 = 2 bytes each
    let file_size = 36 + data_size; // RIFF header - 8

    let byte_rate = sample_rate * channels as u32 * 2; // 16-bit = 2 bytes
    let block_align = channels * 2;

    // RIFF header.
    file.write_all(b"RIFF").change_context(WavWriteError)?;
    file.write_all(&file_size.to_le_bytes())
        .change_context(WavWriteError)?;
    file.write_all(b"WAVE").change_context(WavWriteError)?;

    // fmt chunk.
    file.write_all(b"fmt ").change_context(WavWriteError)?;
    file.write_all(&16u32.to_le_bytes()) // chunk size
        .change_context(WavWriteError)?;
    file.write_all(&1u16.to_le_bytes()) // PCM format
        .change_context(WavWriteError)?;
    file.write_all(&channels.to_le_bytes())
        .change_context(WavWriteError)?;
    file.write_all(&sample_rate.to_le_bytes())
        .change_context(WavWriteError)?;
    file.write_all(&byte_rate.to_le_bytes())
        .change_context(WavWriteError)?;
    file.write_all(&block_align.to_le_bytes())
        .change_context(WavWriteError)?;
    file.write_all(&16u16.to_le_bytes()) // bits per sample
        .change_context(WavWriteError)?;

    // data chunk.
    file.write_all(b"data").change_context(WavWriteError)?;
    file.write_all(&data_size.to_le_bytes())
        .change_context(WavWriteError)?;

    // Write clamped f32 → i16 samples.
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_val = (clamped * i16::MAX as f32) as i16;
        file.write_all(&i16_val.to_le_bytes())
            .change_context(WavWriteError)?;
    }

    file.flush().change_context(WavWriteError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_wav_header(data: &[u8]) -> (u32, u16, u32, u32) {
        // Returns (file_size, channels, sample_rate, data_size)
        let riff = &data[0..4];
        assert_eq!(riff, b"RIFF");
        let file_size = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let wave = &data[8..12];
        assert_eq!(wave, b"WAVE");

        let fmt_tag = &data[12..16];
        assert_eq!(fmt_tag, b"fmt ");
        // fmt chunk size = 16 (bytes 16..20)
        let format = u16::from_le_bytes(data[20..22].try_into().unwrap());
        assert_eq!(format, 1); // PCM
        let channels = u16::from_le_bytes(data[22..24].try_into().unwrap());
        let sample_rate = u32::from_le_bytes(data[24..28].try_into().unwrap());

        // data chunk starts at offset 36
        let data_tag = &data[36..40];
        assert_eq!(data_tag, b"data");
        let data_size = u32::from_le_bytes(data[40..44].try_into().unwrap());

        (file_size, channels, sample_rate, data_size)
    }

    #[test]
    fn write_wav_produces_valid_header() {
        // Given a set of samples.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.wav");
        let samples = vec![0.0f32, 0.0, 0.0, 0.0]; // 2 stereo frames

        // When writing a WAV file.
        write_wav(&path, &samples, 2, 44100).unwrap();

        // Then the file has a valid RIFF header with correct metadata.
        let data = std::fs::read(&path).unwrap();
        let (file_size, channels, sample_rate, data_size) = read_wav_header(&data);
        assert_eq!(channels, 2);
        assert_eq!(sample_rate, 44100);
        assert_eq!(data_size, 8); // 4 samples × 2 bytes
        assert_eq!(file_size, 36 + data_size);
    }

    #[test]
    fn write_wav_clamps_samples() {
        // Given samples exceeding [-1.0, 1.0].
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clamp.wav");
        let samples = vec![2.0f32, -2.0, 0.5];

        // When writing a WAV file.
        write_wav(&path, &samples, 1, 44100).unwrap();

        // Then the written i16 values are clamped.
        let data = std::fs::read(&path).unwrap();
        let pcm_data = &data[44..]; // after header

        let val0 = i16::from_le_bytes(pcm_data[0..2].try_into().unwrap());
        let val1 = i16::from_le_bytes(pcm_data[2..4].try_into().unwrap());
        let val2 = i16::from_le_bytes(pcm_data[4..6].try_into().unwrap());

        assert_eq!(val0, i16::MAX); // 2.0 clamped to 1.0 → i16::MAX
        assert_eq!(val1, i16::MIN + 1); // -2.0 clamped to -1.0 → -32767
        let expected2 = (0.5f32 * i16::MAX as f32) as i16;
        assert_eq!(val2, expected2);
    }

    #[test]
    fn write_wav_empty_samples_produces_header_only() {
        // Given no samples.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        let samples: Vec<f32> = vec![];

        // When writing a WAV file.
        write_wav(&path, &samples, 2, 44100).unwrap();

        // Then the file has the correct header with data_size=0.
        let data = std::fs::read(&path).unwrap();
        let (_, channels, sample_rate, data_size) = read_wav_header(&data);
        assert_eq!(channels, 2);
        assert_eq!(sample_rate, 44100);
        assert_eq!(data_size, 0);
        assert_eq!(data.len(), 44); // header only
    }

    #[test]
    fn write_wav_roundtrip_preserves_sample_count() {
        // Given 100 samples.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roundtrip.wav");
        let samples: Vec<f32> = (0..100).map(|i| (i as f32 / 100.0) * 2.0 - 1.0).collect();

        // When writing a WAV file.
        write_wav(&path, &samples, 2, 44100).unwrap();

        // Then the written file has exactly 100 i16 samples (200 bytes of PCM).
        let data = std::fs::read(&path).unwrap();
        let (_, _, _, data_size) = read_wav_header(&data);
        assert_eq!(data_size, 200); // 100 samples × 2 bytes
        assert_eq!(data.len(), 44 + 200);
    }
}
