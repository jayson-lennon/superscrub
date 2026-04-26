//! Image loading trait and error types.
//!
//! The [`ImageProvider`] trait abstracts over image loading backends,
//! enabling dependency injection for testing.

use std::path::Path;
use std::sync::Arc;

pub mod fake;
pub mod filesystem;
pub mod service;

use error_stack::Report;
use image::RgbaImage;

/// Failed to load an image.
#[derive(Debug, wherror::Error)]
#[error("failed to load image")]
pub struct ImageLoadError;

/// Loads and caches images by path.
pub trait ImageProvider: Send + Sync {
    /// Returns the name of this provider backend (for debugging).
    fn name(&self) -> &'static str;

    /// Load or retrieve a cached image by path.
    ///
    /// Returns an [`Arc<RgbaImage>`] to avoid cloning large pixel buffers
    /// on cache hits. Callers can dereference the Arc to access the image data.
    ///
    /// # Errors
    ///
    /// Returns an error if the image cannot be loaded.
    fn get(&self, path: &Path) -> Result<Arc<RgbaImage>, Report<ImageLoadError>>;
}
