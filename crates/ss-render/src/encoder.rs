//! Frame encoding abstraction.
//!
//! The [`FrameEncoder`] trait abstracts the video encoding pipeline.
//! [`FfmpegEncoder`] is the real implementation that pipes raw RGBA frames
//! to an ffmpeg subprocess for H.264 encoding.

use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;

use error_stack::{Report, ResultExt};
use image::RgbaImage;
use tracing::debug;

use ss_core::project::EncodingConfig;

/// Failed to encode a frame.
#[derive(Debug, wherror::Error)]
#[error("frame encoding failed")]
pub struct EncodeError;

/// Encodes a stream of RGBA frames into a video file.
///
/// Implementations manage their own output lifecycle. Frames are sent
/// sequentially via [`send_frame`](FrameEncoder::send_frame), then
/// the output is finalized with [`finish`](FrameEncoder::finish).
pub trait FrameEncoder: Send + Sync {
    /// Human-readable name for this encoder backend.
    fn name(&self) -> &'static str;

    /// Encode a single RGBA frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame cannot be written to the encoder.
    fn send_frame(&self, frame: &RgbaImage) -> Result<(), Report<EncodeError>>;

    /// Signal that all frames have been sent and finalize the output.
    ///
    /// # Errors
    ///
    /// Returns an error if finalization fails (e.g., ffmpeg exits with error).
    fn finish(&self) -> Result<(), Report<EncodeError>>;
}

/// Internal state for the ffmpeg subprocess.
struct EncoderState {
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
}

/// Frame encoder that pipes raw RGBA to ffmpeg for H.264 encoding.
///
/// Launches `ffmpeg` as a subprocess with raw video input on stdin.
/// Frames are written as raw RGBA bytes (width × height × 4).
/// Call [`finish`](FrameEncoder::finish) to close stdin and wait for
/// ffmpeg to complete encoding.
pub struct FfmpegEncoder {
    state: parking_lot::Mutex<Option<EncoderState>>,
    #[allow(dead_code)]
    output_path: Arc<Path>,
}

impl FfmpegEncoder {
    /// Create a new ffmpeg encoder writing to the given output path.
    ///
    /// Launches the ffmpeg subprocess immediately. The encoder is ready
    /// to receive frames via [`send_frame`](FrameEncoder::send_frame).
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path for the output video file
    /// * `resolution` - Frame dimensions (width, height)
    /// * `fps` - Frames per second
    /// * `encoding` - Encoding settings (CRF, preset, pixel format)
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg cannot be launched.
    pub fn new(
        output_path: &Path,
        resolution: (u32, u32),
        fps: u32,
        encoding: &EncodingConfig,
    ) -> Result<Self, Report<EncodeError>> {
        let mut child = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("rgba")
            .arg("-s")
            .arg(format!("{}x{}", resolution.0, resolution.1))
            .arg("-r")
            .arg(fps.to_string())
            .arg("-i")
            .arg("-")
            .arg("-c:v")
            .arg("libx264")
            .arg("-crf")
            .arg(encoding.crf.to_string())
            .arg("-preset")
            .arg(&encoding.preset)
            .arg("-pix_fmt")
            .arg(&encoding.pixel_format)
            .arg(output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .change_context(EncodeError)
            .attach("failed to launch ffmpeg")?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Report::new(EncodeError).attach("failed to get ffmpeg stdin"))?;

        debug!("ffmpeg encoder launched");

        Ok(Self {
            state: parking_lot::Mutex::new(Some(EncoderState { child, stdin })),
            output_path: Arc::from(output_path),
        })
    }
}

impl FrameEncoder for FfmpegEncoder {
    fn name(&self) -> &'static str {
        "ffmpeg"
    }

    fn send_frame(&self, frame: &RgbaImage) -> Result<(), Report<EncodeError>> {
        let mut guard = self.state.lock();
        let state = guard
            .as_mut()
            .ok_or_else(|| Report::new(EncodeError).attach("encoder already finished"))?;

        use std::io::Write;
        state
            .stdin
            .write_all(frame.as_raw())
            .change_context(EncodeError)
            .attach("failed to write frame to ffmpeg")?;

        Ok(())
    }

    fn finish(&self) -> Result<(), Report<EncodeError>> {
        let state = self.state.lock().take();

        let mut state =
            state.ok_or_else(|| Report::new(EncodeError).attach("encoder already finished"))?;

        // Close stdin to signal EOF to ffmpeg.
        drop(state.stdin);

        let exit_status = state
            .child
            .wait()
            .change_context(EncodeError)
            .attach("failed to wait for ffmpeg")?;

        if !exit_status.success() {
            return Err(Report::new(EncodeError)
                .attach(format!("ffmpeg exited with status {}", exit_status)));
        }

        debug!("encoder finished");

        Ok(())
    }
}
