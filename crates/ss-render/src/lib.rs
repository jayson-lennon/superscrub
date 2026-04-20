//! Headless renderer for final video export.
//!
//! Renders all frames at full resolution via the compositor and produces
//! a final MP4 via ffmpeg. Supports time range clipping, progress tracking,
//! and cancellation.
//!
//! # Architecture
//!
//! - [`encoder`] — [`FrameEncoder`] trait + [`FfmpegEncoder`] implementation
//! - [`fake_encoder`] — [`FakeFrameEncoder`] for testing
//! - [`progress`] — [`ProgressTracker`] (shared progress state)
//! - [`render_job`] — [`RenderJob`] (frame iteration + encoding orchestration)
//! - [`ffmpeg`] — ffmpeg detection and audio muxing utilities

pub mod encoder;
pub mod fake_encoder;
pub mod ffmpeg;
pub mod progress;
pub mod render_job;

pub use encoder::{EncodeError, FfmpegEncoder, FrameEncoder};
pub use fake_encoder::FakeFrameEncoder;
pub use ffmpeg::{FfmpegNotFoundError, MuxError, detect_ffmpeg, mux_audio};
pub use progress::{ProgressTracker, RenderPhase, RenderProgress};
pub use render_job::{RenderError, RenderJob, clamp_time_range, range_frame_count};
