//! Service container for all injectable dependencies.
//!
//! Holds service wrappers for audio, preview, compositor, and config watcher.
//! Created once at startup and shared via `Arc` or cloning.

use std::sync::Arc;

use ss_audio::AudioEngineService;
use ss_compositor::FrameRendererService;
use ss_core::ConfigWatcher;
use ss_preview::PreviewCacheService;

/// Container for all injectable service dependencies.
#[derive(Clone)]
pub struct Services {
    /// Audio playback engine.
    pub audio: AudioEngineService,
    /// Preview frame cache.
    pub preview: PreviewCacheService,
    /// Frame renderer (used by preview cache, available for direct render if needed).
    pub renderer: FrameRendererService,
    /// Project file change watcher.
    pub watcher: Arc<dyn ConfigWatcher>,
}
