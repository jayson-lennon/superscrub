//! Preview frame cache.
//!
//! Pre-renders all frames at a configurable preview resolution and stores
//! them in memory for instant scrubbing and playback. Uses rayon for
//! parallel rendering and [`FrameRenderer`](ss_compositor::FrameRenderer)
//! from ss-compositor for frame generation.
//!
//! # Architecture
//!
//! - [`cache`] — Preview cache trait, progress tracking, and implementations
//! - [`frame_index`] — Time ↔ frame index conversion utilities

pub mod cache;
pub mod frame_index;

pub use cache::background::BackgroundPreviewCache;
pub use cache::fake::FakePreviewCache;
pub use cache::service::PreviewCacheService;
pub use cache::{PreviewCache, PreviewError, PreviewRenderProgress};
pub use frame_index::{frame_index_to_time, time_to_frame_index, total_frames};
