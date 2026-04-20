//! Frame compositing and rendering.
//!
//! This crate handles rendering individual frames by compositing clips
//! with transforms, alpha blending, and z-ordering.
//!
//! # Architecture
//!
//! - [`rendering`] — Frame rendering trait and compositor implementation
//! - [`image`] — Image loading trait and providers
//! - [`sizing`] — Sizing mode → placement rectangle computation
//! - [`viewport`] — Viewport description (canvas rect, output size, camera)

pub mod image;
pub mod rendering;
pub mod sizing;
pub mod viewport;

pub use image::fake::FakeImageProvider;
pub use image::filesystem::FilesystemImageProvider;
pub use image::service::ImageProviderService;
pub use image::{ImageLoadError, ImageProvider};
pub use rendering::compositor::CompositorRenderer;
pub use rendering::service::FrameRendererService;
pub use rendering::{CompositorError, FrameRenderer};
pub use sizing::PlacedRect;
pub use viewport::Viewport;
