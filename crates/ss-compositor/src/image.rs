//! Image loading trait and error types.
//!
//! The [`ImageProvider`] trait abstracts over image loading backends,
//! enabling dependency injection for testing.

use std::path::Path;

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
    /// # Errors
    ///
    /// Returns an error if the image cannot be loaded.
    fn get(&self, path: &Path) -> Result<RgbaImage, Report<ImageLoadError>>;
}
