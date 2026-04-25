//! Creates test services with fake backends.

#![allow(dead_code)]

use std::sync::Arc;

use ss_audio::{AudioEngineService, FakeAudioEngine};
use ss_compositor::{FakeImageProvider, FrameRendererService};
use ss_editor::Services;
use ss_preview::{FakePreviewCache, PreviewCacheService};

/// A fake config watcher that always reports no changes.
pub struct FakeConfigWatcher;

impl ss_core::ConfigWatcher for FakeConfigWatcher {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn watch(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), error_stack::Report<ss_core::ConfigWatchError>> {
        Ok(())
    }

    fn has_changed(&self) -> bool {
        false
    }
}

/// Create test services with fake backends.
pub fn create_test_services() -> (Services, Arc<FakeAudioEngine>, Arc<FakePreviewCache>) {
    let fake_audio = Arc::new(FakeAudioEngine::new());
    let fake_preview = Arc::new(FakePreviewCache::new());
    let fake_provider = Arc::new(FakeImageProvider::new());
    let fake_renderer = Arc::new(ss_compositor::CompositorRenderer::new(fake_provider));

    let services = Services {
        audio: AudioEngineService::new(fake_audio.clone()),
        preview: PreviewCacheService::new(fake_preview.clone()),
        renderer: FrameRendererService::new(fake_renderer),
        watcher: Arc::new(FakeConfigWatcher),
    };

    (services, fake_audio, fake_preview)
}
