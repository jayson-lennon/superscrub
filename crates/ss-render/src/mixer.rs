//! Offline audio mixer for the render pipeline.
//!
//! Mixes multiple audio clips into a single output buffer, applying
//! per-clip volume and timeline positioning. This is a pure function
//! with no I/O or audio device dependency — fully testable.

/// A decoded audio clip ready for mixing.
///
/// Callers must ensure `samples` are already at the target sample rate
/// and channel count (via resampling/remixing before constructing).
#[derive(Debug, Clone)]
pub struct MixedClip {
    /// Interleaved f32 samples.
    pub samples: Vec<f32>,
    /// Channel count.
    pub channels: u16,
    /// Sample rate.
    pub sample_rate: u32,
    /// When this clip starts on the timeline (seconds).
    pub start_time: f64,
    /// Per-clip volume [0.0, 1.0].
    pub volume: f32,
}

/// Mix multiple clips into a single interleaved f32 buffer.
///
/// Each clip is positioned at its `start_time` on the timeline.
/// Per-clip volume is applied independently. Overlapping clips are
/// mixed additively (summed).
///
/// Output format: `output_channels` channels at `output_sample_rate` Hz.
/// Buffer length covers `duration` seconds.
///
/// No clipping protection — the caller handles saturation if needed.
#[must_use]
pub fn mix_clips(
    clips: &[MixedClip],
    output_sample_rate: u32,
    output_channels: u16,
    duration: f64,
) -> Vec<f32> {
    let total_samples = (duration * output_sample_rate as f64 * output_channels as f64) as usize;
    let mut output = vec![0.0f32; total_samples];

    for clip in clips {
        mix_single_clip(&mut output, clip, output_sample_rate, output_channels);
    }

    output
}

/// Position one clip's samples into the output buffer.
///
/// Applies per-clip volume and offsets samples by the clip's `start_time`.
/// Samples that fall outside the output buffer are truncated.
fn mix_single_clip(
    output: &mut [f32],
    clip: &MixedClip,
    output_sample_rate: u32,
    output_channels: u16,
) {
    let start_sample =
        (clip.start_time * output_sample_rate as f64 * output_channels as f64) as usize;
    let clip_volume = clip.volume;

    for (i, &sample) in clip.samples.iter().enumerate() {
        let out_idx = start_sample + i;
        if out_idx >= output.len() {
            break;
        }
        output[out_idx] += sample * clip_volume;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 44100;
    const CHANNELS: u16 = 2;

    fn make_clip(samples: Vec<f32>, start_time: f64, volume: f32) -> MixedClip {
        MixedClip {
            samples,
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            start_time,
            volume,
        }
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_empty_clips_produces_silence() {
        // Given no clips and a 1-second duration.
        let clips: Vec<MixedClip> = vec![];
        let duration = 1.0;

        // When mixing.
        let output = mix_clips(&clips, SAMPLE_RATE, CHANNELS, duration);

        // Then the output is all zeros with the correct length.
        let expected_len = (1.0 * SAMPLE_RATE as f64 * CHANNELS as f64) as usize;
        assert_eq!(output.len(), expected_len);
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_single_clip_copies_samples() {
        // Given one clip at start_time=0 with volume=1.0.
        let samples = vec![0.5, -0.3, 0.8, -0.1];
        let clip = make_clip(samples.clone(), 0.0, 1.0);
        let duration = 1.0;

        // When mixing.
        let output = mix_clips(&[clip], SAMPLE_RATE, CHANNELS, duration);

        // Then the first 4 samples match the input exactly.
        assert_eq!(output[0], 0.5);
        assert_eq!(output[1], -0.3);
        assert_eq!(output[2], 0.8);
        assert_eq!(output[3], -0.1);
        // Remaining samples are zero.
        assert!(output[4..].iter().all(|&s| s == 0.0));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_clip_with_volume_scales_samples() {
        // Given one clip at volume=0.5.
        let samples = vec![1.0, -1.0, 0.8, -0.8];
        let clip = make_clip(samples, 0.0, 0.5);
        let duration = 1.0;

        // When mixing.
        let output = mix_clips(&[clip], SAMPLE_RATE, CHANNELS, duration);

        // Then each sample is halved.
        assert_eq!(output[0], 0.5);
        assert_eq!(output[1], -0.5);
        assert_eq!(output[2], 0.4);
        assert_eq!(output[3], -0.4);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_two_overlapping_clips_adds_samples() {
        // Given two clips at the same start_time.
        let clip_a = make_clip(vec![1.0, 0.0, 0.0, 0.0], 0.0, 1.0);
        let clip_b = make_clip(vec![0.5, 0.0, 0.0, 0.0], 0.0, 1.0);
        let duration = 1.0;

        // When mixing.
        let output = mix_clips(&[clip_a, clip_b], SAMPLE_RATE, CHANNELS, duration);

        // Then the overlapping samples are summed.
        assert_eq!(output[0], 1.5);
        assert_eq!(output[1], 0.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_clip_with_start_time_offsets_correctly() {
        // Given a clip starting at a known offset.
        // Use a low sample rate so offset math is trivial.
        // 10 Hz, 1 channel, clip at start_time=1.0s → offset = 10 samples.
        let clip = make_clip(vec![1.0, 2.0, 3.0], 1.0, 1.0);
        let duration = 2.0;

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then first 10 samples are silence, then the clip starts.
        assert_eq!(output.len(), 20);
        assert!(output[..10].iter().all(|&s| s == 0.0));
        assert_eq!(output[10], 1.0);
        assert_eq!(output[11], 2.0);
        assert_eq!(output[12], 3.0);
        assert!(output[13..].iter().all(|&s| s == 0.0));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn mix_clip_truncated_at_duration_boundary() {
        // Given a clip that extends beyond the output duration.
        // 10 Hz, 1 channel, 2-second output = 20 samples.
        // Clip has 30 samples starting at offset 0.
        let clip = make_clip(vec![1.0; 30], 0.0, 1.0);
        let duration = 2.0;

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then output has exactly 20 samples and the last is 1.0.
        assert_eq!(output.len(), 20);
        assert!(output.iter().all(|&s| s == 1.0));
    }
}
