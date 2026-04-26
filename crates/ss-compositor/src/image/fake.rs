//! Fake image provider for testing.
//!
//! Allows inserting pre-built images by path and tracking how many
//! times images are loaded (for cache verification).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use error_stack::Report;
use image::{Rgba, RgbaImage};

use crate::image::ImageLoadError;
use crate::image::ImageProvider;

/// A fake image provider that returns pre-inserted images.
///
/// Use [`FakeImageProvider::insert_solid`] to add test images, and check [`FakeImageProvider::load_count`]
/// to verify caching behavior.
pub struct FakeImageProvider {
    images: HashMap<PathBuf, Arc<RgbaImage>>,
    /// Number of times `get` has been called (for cache verification).
    pub load_count: AtomicUsize,
}

impl FakeImageProvider {
    /// Create a new fake image provider with no images.
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            load_count: AtomicUsize::new(0),
        }
    }

    /// Insert a solid-color image of the given dimensions.
    pub fn insert_solid(&mut self, path: &str, width: u32, height: u32, color: [u8; 4]) {
        let img = RgbaImage::from_pixel(width, height, Rgba(color));
        self.images.insert(PathBuf::from(path), Arc::new(img));
    }
}

impl Default for FakeImageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageProvider for FakeImageProvider {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn get(&self, path: &Path) -> Result<Arc<RgbaImage>, Report<ImageLoadError>> {
        self.load_count.fetch_add(1, Ordering::SeqCst);
        self.images
            .get(path)
            .cloned()
            .ok_or_else(|| Report::new(ImageLoadError).attach(format!("path: {}", path.display())))
    }
}
