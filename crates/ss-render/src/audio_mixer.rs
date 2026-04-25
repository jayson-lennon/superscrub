//! High-level audio mixing for the render pipeline.
//!
//! Decodes all audio clips from a project, resamples to a fixed output
//! format (44100 Hz stereo), mixes them into a single buffer, and writes
//! the result as a WAV file.

use std::path::{Path, PathBuf};

use error_stack::{Report, ResultExt};

use ss_audio::decoder::{decode_file_with_sample_rate, remix_channels};
use ss_core::path_resolve::resolve_path;
use ss_core::project::Project;

use crate::mixer::{MixedClip, mix_clips};
use crate::wav_writer::write_wav;

/// Output sample rate for the mixed audio.
const OUTPUT_SAMPLE_RATE: u32 = 44100;
/// Output channel count (stereo).
const OUTPUT_CHANNELS: u16 = 2;

/// Failed to render audio for the project.
#[derive(Debug, wherror::Error)]
#[error("audio render failed")]
pub struct AudioRenderError;

/// Decode, mix, and write all project audio clips to a WAV file.
///
/// For each audio clip in the project:
/// 1. Resolve the clip's path relative to the project file
/// 2. Decode and resample to 44100 Hz via `decode_file_with_sample_rate()`
/// 3. Remix to stereo via `remix_channels()`
///
/// Then mix all clips into a single buffer (additive mixing with per-clip
/// volume and timeline positioning) and write as a 16-bit PCM WAV file.
///
/// Returns the path to the written WAV file.
///
/// # Arguments
///
/// * `project` - The project containing audio clip definitions
/// * `project_file` - Path to the project JSON file (for resolving relative paths)
/// * `output_dir` - Directory to write the mixed WAV file into
/// * `start_time` - Render range start (seconds)
/// * `end_time` - Render range end (seconds)
///
/// # Errors
///
/// Returns [`AudioRenderError`] if the project has no audio clips, any audio
/// file cannot be decoded, or the WAV file cannot be written.
pub fn render_audio(
    project: &Project,
    project_file: &Path,
    output_dir: &Path,
    start_time: f64,
    end_time: f64,
) -> Result<PathBuf, Report<AudioRenderError>> {
    if project.audio_clips.is_empty() {
        return Err(Report::new(AudioRenderError).attach("no audio clips"));
    }

    let mut mixed_clips = Vec::new();

    for clip_def in &project.audio_clips {
        let audio_path = resolve_path(project_file, &clip_def.path)
            .change_context(AudioRenderError)
            .attach(format!("failed to resolve path for clip '{}'", clip_def.id))?;

        let decoded = decode_file_with_sample_rate(&audio_path, OUTPUT_SAMPLE_RATE)
            .change_context(AudioRenderError)
            .attach(format!("failed to decode audio for clip '{}'", clip_def.id))?;

        let decoded = remix_channels(decoded, OUTPUT_CHANNELS);

        // Adjust start_time relative to the render range.
        let adjusted_start = clip_def.start_time.max(start_time) - start_time;

        mixed_clips.push(MixedClip {
            samples: decoded.samples.0,
            channels: decoded.channels,
            sample_rate: decoded.sample_rate,
            start_time: adjusted_start,
            volume: clip_def.volume,
        });
    }

    let duration = std::time::Duration::from_secs_f64(end_time - start_time);
    let mixed = mix_clips(&mixed_clips, OUTPUT_SAMPLE_RATE, OUTPUT_CHANNELS, duration);

    let wav_path = output_dir.join("mixed_audio.wav");
    write_wav(&wav_path, &mixed, OUTPUT_CHANNELS, OUTPUT_SAMPLE_RATE)
        .change_context(AudioRenderError)
        .attach("failed to write mixed audio WAV")?;

    Ok(wav_path)
}
