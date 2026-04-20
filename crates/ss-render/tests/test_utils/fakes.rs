//! Creates test services with fake backends for render tests.

#![allow(dead_code)]

use std::sync::Arc;

use ss_compositor::{CompositorRenderer, FakeImageProvider, FrameRendererService};
use ss_render::FakeFrameEncoder;

/// Create a frame renderer service with a fake image provider.
///
/// The fake provider starts empty. Insert images via the returned provider
/// before rendering.
pub fn create_renderer() -> (FrameRendererService, Arc<FakeImageProvider>) {
    let provider = Arc::new(FakeImageProvider::new());
    let renderer = Arc::new(CompositorRenderer::new(provider.clone()));
    let service = FrameRendererService::new(renderer);
    (service, provider)
}

/// Create a fake frame encoder for testing.
pub fn create_encoder() -> FakeFrameEncoder {
    FakeFrameEncoder::new()
}
