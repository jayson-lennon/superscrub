//! ffmpeg binary detection and audio muxing utilities.

use std::path::Path;
use std::process::Command;

use error_stack::{Report, ResultExt};
use tracing::debug;

/// ffmpeg was not found on the system.
#[derive(Debug, wherror::Error)]
#[error("ffmpeg not found")]
pub struct FfmpegNotFoundError;

/// Failed to mux audio into the video file.
#[derive(Debug, wherror::Error)]
#[error("audio muxing failed")]
pub struct MuxError;

/// Check that ffmpeg is available on the system.
///
/// Runs `ffmpeg -version` and checks for success. Call this at startup
/// before starting any render work.
///
/// # Errors
///
/// Returns [`FfmpegNotFoundError`] if ffmpeg is not in PATH or exits with
/// a non-zero status.
pub fn detect_ffmpeg() -> Result<(), Report<FfmpegNotFoundError>> {
    let status = Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| Report::new(FfmpegNotFoundError))?;

    if !status.success() {
        return Err(Report::new(FfmpegNotFoundError));
    }

    debug!("ffmpeg detected");

    Ok(())
}

/// Mux an audio file into a video file, producing the final output.
///
/// Runs a second ffmpeg pass:
/// ```sh
/// ffmpeg -y -i video.mp4 [-itsoffset START -i audio.mp3] -c:v copy -c:a aac -shortest output.mp4
/// ```
///
/// If `audio_start_time` > 0, the audio is delayed using `-itsoffset`.
///
/// # Arguments
///
/// * `video_path` - Path to the video-only file (first pass output)
/// * `audio_path` - Path to the audio file
/// * `output_path` - Path for the final muxed output
/// * `audio_start_time` - Time offset in seconds for the audio (0.0 = start with video)
///
/// # Errors
///
/// Returns [`MuxError`] if ffmpeg fails to mux the audio.
pub fn mux_audio(
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
    audio_start_time: f64,
) -> Result<(), Report<MuxError>> {
    debug!("audio mux started");
    let mut cmd = Command::new("ffmpeg");

    cmd.arg("-y").arg("-i").arg(video_path);

    // Apply audio offset if non-zero.
    if audio_start_time > 0.0 {
        cmd.arg("-itsoffset").arg(audio_start_time.to_string());
    }

    cmd.arg("-i")
        .arg(audio_path)
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(output_path);

    let status = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .change_context(MuxError)
        .attach("failed to launch ffmpeg for muxing")?;

    if !status.success() {
        return Err(
            Report::new(MuxError).attach(format!("ffmpeg mux exited with status {}", status))
        );
    }

    debug!("audio mux completed");

    Ok(())
}

/// Mux a pre-mixed audio file into a video file.
///
/// The audio file already has correct timing baked in (no offset needed).
/// Runs:
/// ```sh
/// ffmpeg -y -i video.mp4 -i audio.wav -c:v copy -c:a aac -shortest output.mp4
/// ```
///
/// # Arguments
///
/// * `video_path` - Path to the video-only file (first pass output)
/// * `audio_path` - Path to the pre-mixed audio file
/// * `output_path` - Path for the final muxed output
///
/// # Errors
///
/// Returns [`MuxError`] if ffmpeg fails to mux the audio.
pub fn mux_mixed_audio(
    video_path: &Path,
    audio_path: &Path,
    output_path: &Path,
) -> Result<(), Report<MuxError>> {
    debug!("audio mux started (pre-mixed)");

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path)
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(output_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .change_context(MuxError)
        .attach("failed to launch ffmpeg for muxing")?;

    if !status.success() {
        return Err(
            Report::new(MuxError).attach(format!("ffmpeg mux exited with status {}", status))
        );
    }

    debug!("audio mux completed (pre-mixed)");

    Ok(())
}
