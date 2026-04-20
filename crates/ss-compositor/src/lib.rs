//! Frame compositing and rendering.
//!
//! This crate handles rendering individual frames by compositing clips
//! with transforms, alpha blending, and z-ordering.
//!
//! # Architecture
//!
//! - [`traits`] — `FrameRenderer` and `ImageProvider` trait definitions
//! - [`renderer`] — Main rendering pipeline (`CompositorRenderer`)
//! - [`sizing`] — Sizing mode → placement rectangle computation
//! - [`viewport`] — Viewport description (canvas rect, output size, camera)
//! - [`image_provider`] — Filesystem image loading with caching
//! - [`fake_image_provider`] — Test fake for image loading
//! - [`service`] — Service wrappers (`FrameRendererService`, `ImageProviderService`)
//! - [`errors`] — Error types (`CompositorError`, `ImageLoadError`)

pub mod errors;
pub mod fake_image_provider;
pub mod image_provider;
pub mod renderer;
pub mod service;
pub mod sizing;
pub mod traits;
pub mod viewport;

pub use errors::{CompositorError, ImageLoadError};
pub use fake_image_provider::FakeImageProvider;
pub use image_provider::FilesystemImageProvider;
pub use renderer::CompositorRenderer;
pub use service::{FrameRendererService, ImageProviderService};
pub use sizing::PlacedRect;
pub use traits::{FrameRenderer, ImageProvider};
pub use viewport::Viewport;
