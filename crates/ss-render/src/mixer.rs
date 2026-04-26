//! Offline audio mixer for the render pipeline.
//!
//! Mixes multiple audio clips into a single output buffer, applying
//! per-clip volume (static or keyframed) and timeline positioning.
//! This is a pure function with no I/O or audio device dependency —
//! fully testable.

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
    /// Offset into the source audio where playback begins (seconds).
    /// 0.0 means play from the start of the source file.
    pub source_offset: f64,
    /// Offset into the source audio where playback ends (seconds).
    /// 0.0 means play to the end of the source file.
    pub trim_end: f64,
    /// Audio animation tracks (volume automation).
    pub animations: Vec<ss_core::animation::AudioAnimationTrack>,
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
    duration: std::time::Duration,
) -> Vec<f32> {
    let total_samples =
        (duration.as_secs_f64() * output_sample_rate as f64 * output_channels as f64) as usize;
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
    use ss_core::animation::AudioAnimatableProperty;
    use ss_core::interpolation::interpolate_keyframes;

    let start_sample =
        (clip.start_time * output_sample_rate as f64 * output_channels as f64) as usize;
    let clip_volume = clip.volume;

    // Compute the first source sample to read (source_offset).
    let source_start =
        (clip.source_offset * clip.sample_rate as f64 * clip.channels as f64) as usize;

    // Compute the upper bound on source samples to read (trim_end).
    let source_end = if clip.trim_end > 0.0 {
        ((clip.trim_end * clip.sample_rate as f64 * clip.channels as f64) as usize)
            .min(clip.samples.len())
    } else {
        clip.samples.len()
    };

    if source_start >= source_end {
        return; // Nothing to mix (offset past trim or past source).
    }

    // Find the volume animation track.
    let volume_keyframes: &[ss_core::animation::Keyframe] = clip
        .animations
        .iter()
        .find(|t| t.property == AudioAnimatableProperty::Volume)
        .map(|t| t.keyframes.as_slice())
        .unwrap_or(&[]);

    for i in source_start..source_end {
        let out_idx = start_sample + (i - source_start);
        if out_idx >= output.len() {
            break;
        }

        let vol = if volume_keyframes.is_empty() {
            clip_volume
        } else {
            let local_time =
                (i - source_start) as f64 / (clip.sample_rate as f64 * clip.channels as f64);
            interpolate_keyframes(volume_keyframes, local_time).unwrap_or(clip_volume)
        };

        output[out_idx] += clip.samples[i] * vol;
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
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
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![],
        }
    }

    #[test]
    fn mix_empty_clips_produces_silence() {
        // Given no clips and a 1-second duration.
        let clips: Vec<MixedClip> = vec![];
        let duration = std::time::Duration::from_secs_f64(1.0);

        // When mixing.
        let output = mix_clips(&clips, SAMPLE_RATE, CHANNELS, duration);

        // Then the output is all zeros with the correct length.
        let expected_len = (1.0 * SAMPLE_RATE as f64 * CHANNELS as f64) as usize;
        assert_eq!(output.len(), expected_len);
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_single_clip_copies_samples() {
        // Given one clip at start_time=0 with volume=1.0.
        let samples = vec![0.5, -0.3, 0.8, -0.1];
        let clip = make_clip(samples.clone(), 0.0, 1.0);
        let duration = std::time::Duration::from_secs_f64(1.0);

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
    fn mix_clip_with_volume_scales_samples() {
        // Given one clip at volume=0.5.
        let samples = vec![1.0, -1.0, 0.8, -0.8];
        let clip = make_clip(samples, 0.0, 0.5);
        let duration = std::time::Duration::from_secs_f64(1.0);

        // When mixing.
        let output = mix_clips(&[clip], SAMPLE_RATE, CHANNELS, duration);

        // Then each sample is halved.
        assert_eq!(output[0], 0.5);
        assert_eq!(output[1], -0.5);
        assert_eq!(output[2], 0.4);
        assert_eq!(output[3], -0.4);
    }

    #[test]
    fn mix_two_overlapping_clips_adds_samples() {
        // Given two clips at the same start_time.
        let clip_a = make_clip(vec![1.0, 0.0, 0.0, 0.0], 0.0, 1.0);
        let clip_b = make_clip(vec![0.5, 0.0, 0.0, 0.0], 0.0, 1.0);
        let duration = std::time::Duration::from_secs_f64(1.0);

        // When mixing.
        let output = mix_clips(&[clip_a, clip_b], SAMPLE_RATE, CHANNELS, duration);

        // Then the overlapping samples are summed.
        assert_eq!(output[0], 1.5);
        assert_eq!(output[1], 0.0);
    }

    #[test]
    fn mix_clip_with_start_time_offsets_correctly() {
        // Given a clip starting at a known offset.
        // Use a low sample rate so offset math is trivial.
        // 10 Hz, 1 channel, clip at start_time=1.0s → offset = 10 samples.
        let clip = make_clip(vec![1.0, 2.0, 3.0], 1.0, 1.0);
        let duration = std::time::Duration::from_secs_f64(2.0);

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
    fn mix_clip_truncated_at_duration_boundary() {
        // Given a clip that extends beyond the output duration.
        // 10 Hz, 1 channel, 2-second output = 20 samples.
        // Clip has 30 samples starting at offset 0.
        let clip = make_clip(vec![1.0; 30], 0.0, 1.0);
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then output has exactly 20 samples and the last is 1.0.
        assert_eq!(output.len(), 20);
        assert!(output.iter().all(|&s| s == 1.0));
    }

    // ================================================================
    // source_offset and trim_end tests
    // ================================================================

    #[test]
    fn mix_clip_with_source_offset_skips_samples() {
        // Given a clip with source_offset=0.5s at 10 Hz mono.
        // Source: [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        // source_offset=0.5s at 10 Hz mono = skip 5 samples.
        let samples: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.5,
            trim_end: 0.0,
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the first 5 output samples are source[5..10].
        assert_eq!(output[0], 5.0);
        assert_eq!(output[1], 6.0);
        assert_eq!(output[2], 7.0);
        assert_eq!(output[3], 8.0);
        assert_eq!(output[4], 9.0);
        // Remaining output is silence.
        assert!(output[5..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_clip_with_trim_end_stops_early() {
        // Given a clip with trim_end=0.5s at 10 Hz mono.
        // Source: [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        // trim_end=0.5s at 10 Hz mono = only first 5 samples.
        let samples: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.0,
            trim_end: 0.5,
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the first 5 output samples are source[0..5].
        assert_eq!(output[0], 0.0);
        assert_eq!(output[1], 1.0);
        assert_eq!(output[2], 2.0);
        assert_eq!(output[3], 3.0);
        assert_eq!(output[4], 4.0);
        // Remaining output is silence.
        assert!(output[5..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_clip_with_source_offset_and_trim_end_selects_window() {
        // Given a clip with source_offset=0.2s, trim_end=0.7s at 10 Hz mono.
        // Source: [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]
        // source_offset=0.2s → start at sample 2
        // trim_end=0.7s → stop at sample 7
        // Window: source[2..7] = [2.0, 3.0, 4.0, 5.0, 6.0]
        let samples: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.2,
            trim_end: 0.7,
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the output has the 5 windowed samples.
        assert_eq!(output[0], 2.0);
        assert_eq!(output[1], 3.0);
        assert_eq!(output[2], 4.0);
        assert_eq!(output[3], 5.0);
        assert_eq!(output[4], 6.0);
        assert!(output[5..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_clip_with_offset_past_trim_produces_silence() {
        // Given a clip with source_offset=0.8s, trim_end=0.5s at 10 Hz mono.
        // source_start=8, source_end=5 → source_start >= source_end → nothing to mix.
        let samples: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.8,
            trim_end: 0.5,
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the output is all silence.
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_clip_with_trim_end_past_source_plays_to_end() {
        // Given a clip with trim_end=5.0s but source is only 10 samples (1.0s at 10 Hz).
        // trim_end clamps to source length → plays all 10 samples.
        let samples: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.0,
            trim_end: 5.0, // way past actual source
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing at 10 Hz, 1 channel.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then all 10 source samples appear.
        assert_eq!(output[0], 0.0);
        assert_eq!(output[9], 9.0);
        assert!(output[10..].iter().all(|&s| s == 0.0));
    }

    // ================================================================
    // Volume animation tests
    // ================================================================

    #[test]
    fn mix_clip_with_volume_fade_in_applies_ramp() {
        // Given a 10 Hz mono clip with constant source=1.0 and volume keyframes
        // ramping from 0.0 at t=0 to 1.0 at t=1.0.
        use ss_core::animation::{AudioAnimatableProperty, AudioAnimationTrack, Keyframe};

        let samples = vec![1.0f32; 10]; // 1 second at 10 Hz mono
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![AudioAnimationTrack {
                property: AudioAnimatableProperty::Volume,
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: 0.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                    Keyframe {
                        time: 1.0,
                        value: 1.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                ],
            }],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then first sample is near 0.0 and last sample is near 0.9.
        // At sample 0: local_time=0.0, interpolated volume=0.0
        // At sample 9: local_time=0.9, interpolated volume=0.9
        assert!(
            output[0].abs() < 0.01,
            "first sample should be near 0.0, got {}",
            output[0]
        );
        assert!(
            (output[9] - 0.9).abs() < 0.01,
            "last sample should be near 0.9, got {}",
            output[9]
        );
        // Volume should increase monotonically.
        for w in output.windows(2) {
            if w[0] > 0.0 && w[1] > 0.0 {
                assert!(w[1] >= w[0], "volume should increase: {} -> {}", w[0], w[1]);
            }
        }
    }

    #[test]
    fn mix_clip_with_volume_fade_out_applies_ramp() {
        // Given a 10 Hz mono clip with constant source=1.0 and volume keyframes
        // ramping from 1.0 at t=0 to 0.0 at t=1.0.
        use ss_core::animation::{AudioAnimatableProperty, AudioAnimationTrack, Keyframe};

        let samples = vec![1.0f32; 10]; // 1 second at 10 Hz mono
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![AudioAnimationTrack {
                property: AudioAnimatableProperty::Volume,
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: 1.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                    Keyframe {
                        time: 1.0,
                        value: 0.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                ],
            }],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then first sample is near 1.0 and last sample is near 0.1.
        // At sample 0: local_time=0.0, interpolated volume=1.0
        // At sample 9: local_time=0.9, interpolated volume=0.1
        assert!(
            (output[0] - 1.0).abs() < 0.01,
            "first sample should be near 1.0, got {}",
            output[0]
        );
        assert!(
            (output[9] - 0.1).abs() < 0.01,
            "last sample should be near 0.1, got {}",
            output[9]
        );
        // Volume should decrease monotonically.
        for w in output.windows(2) {
            if w[0] > 0.0 && w[1] > 0.0 {
                assert!(w[1] <= w[0], "volume should decrease: {} -> {}", w[0], w[1]);
            }
        }
    }

    #[test]
    fn mix_clip_with_no_animations_uses_static_volume() {
        // Given a clip with empty animations and volume=0.5.
        let samples = vec![1.0f32; 4];
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 0.5,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![],
        };
        let duration = std::time::Duration::from_secs_f64(1.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then every sample is exactly 0.5 (static volume * source 1.0).
        assert!(output[..4].iter().all(|&s| (s - 0.5).abs() < 1e-6));
        assert!(output[4..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mix_clip_with_volume_keyframes_and_source_offset() {
        // Given a 10 Hz mono clip with source_offset=0.2s and volume keyframes
        // ramping from 0.0 to 1.0 over 0.5s.
        // source_offset=0.2s means source_start=2. The clip plays samples 2..10.
        // local_time for sample i is (i - source_start) / (sample_rate * channels).
        use ss_core::animation::{AudioAnimatableProperty, AudioAnimationTrack, Keyframe};

        let samples: Vec<f32> = (0..10).map(|_| 1.0f32).collect();
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 1.0,
            source_offset: 0.2,
            trim_end: 0.0,
            animations: vec![AudioAnimationTrack {
                property: AudioAnimatableProperty::Volume,
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: 0.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                    Keyframe {
                        time: 0.5,
                        value: 1.0,
                        easing: ss_core::animation::Easing::Linear,
                    },
                ],
            }],
        };
        let duration = std::time::Duration::from_secs_f64(2.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the first output sample (source sample 2, local_time=0.0) has volume 0.0.
        // The 5th output sample (source sample 6, local_time=0.4) has volume 0.8.
        // The 6th output sample (source sample 7, local_time=0.5) has volume 1.0.
        assert!(
            output[0].abs() < 0.01,
            "first sample should be near 0.0, got {}",
            output[0]
        );
        assert!(
            (output[4] - 0.8).abs() < 0.01,
            "5th sample should be near 0.8, got {}",
            output[4]
        );
        assert!(
            (output[5] - 1.0).abs() < 0.01,
            "6th sample should be near 1.0, got {}",
            output[5]
        );
    }

    #[test]
    fn mix_clip_with_empty_volume_keyframes_falls_back_to_static() {
        // Given a clip with a volume animation track that has empty keyframes.
        use ss_core::animation::{AudioAnimatableProperty, AudioAnimationTrack};

        let samples = vec![1.0f32; 4];
        let clip = MixedClip {
            samples,
            channels: 1,
            sample_rate: 10,
            start_time: 0.0,
            volume: 0.75,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![AudioAnimationTrack {
                property: AudioAnimatableProperty::Volume,
                keyframes: vec![],
            }],
        };
        let duration = std::time::Duration::from_secs_f64(1.0);

        // When mixing.
        let output = mix_clips(&[clip], 10, 1, duration);

        // Then the empty keyframes vec means volume_keyframes is empty, so
        // the static volume fallback is used (clip.volume=0.75 per sample).
        assert!(output[..4].iter().all(|&s| (s - 0.75).abs() < 1e-6));
    }
}
