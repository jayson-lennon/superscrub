//! Preview frame cache.
//!
//! Pre-renders all frames at a configurable preview resolution and stores
//! them in memory for instant scrubbing and playback. Uses rayon for
//! parallel rendering and [`FrameRenderer`](ss_compositor::FrameRenderer)
//! from ss-compositor for frame generation.
//!
//! # Architecture
//!
//! - [`traits`] — `PreviewCache` trait + `RenderProgress` struct
//! - [`background_cache`] — Real implementation with rayon parallel rendering
//! - [`fake_cache`] — Test fake with call tracking
//! - [`frame_index`] — Time ↔ frame index conversion utilities
//! - [`service`] — `PreviewCacheService` wrapper
//! - [`errors`] — `PreviewError`

pub mod background_cache;
pub mod errors;
pub mod fake_cache;
pub mod frame_index;
pub mod service;
pub mod traits;

pub use background_cache::BackgroundPreviewCache;
pub use errors::PreviewError;
pub use fake_cache::FakePreviewCache;
pub use frame_index::{frame_index_to_time, time_to_frame_index, total_frames};
pub use service::PreviewCacheService;
pub use traits::{PreviewCache, RenderProgress};
