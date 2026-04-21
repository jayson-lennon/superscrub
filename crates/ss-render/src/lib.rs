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
//! - [`mixer`] — Offline audio mixer (pure function, no I/O)
//! - [`wav_writer`] — Minimal WAV file writer (f32 → 16-bit PCM)
//! - [`audio_mixer`] — High-level audio rendering orchestration

pub mod audio_mixer;
pub mod encoder;
pub mod fake_encoder;
pub mod ffmpeg;
pub mod mixer;
pub mod progress;
pub mod render_job;
pub mod wav_writer;

pub use audio_mixer::{AudioRenderError, render_audio};
pub use encoder::{EncodeError, FfmpegEncoder, FrameEncoder};
pub use fake_encoder::FakeFrameEncoder;
pub use ffmpeg::{FfmpegNotFoundError, MuxError, detect_ffmpeg, mux_audio, mux_mixed_audio};
pub use mixer::MixedClip;
pub use progress::{ProgressTracker, RenderPhase, RenderProgress};
pub use render_job::{RenderError, RenderJob, clamp_time_range, range_frame_count};
pub use wav_writer::write_wav;
