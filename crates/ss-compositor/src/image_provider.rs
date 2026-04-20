//! Filesystem-backed image provider with caching.
//!
//! Loads images from disk and caches them in memory. Subsequent requests
//! for the same path return the cached image without disk I/O.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use error_stack::Report;
use image::RgbaImage;

use crate::errors::ImageLoadError;
use crate::traits::ImageProvider;

/// Loads images from the filesystem and caches them in memory.
pub struct FilesystemImageProvider {
    cache: Mutex<HashMap<PathBuf, RgbaImage>>,
}

impl FilesystemImageProvider {
    /// Create a new filesystem image provider with an empty cache.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for FilesystemImageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageProvider for FilesystemImageProvider {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn get(&self, path: &Path) -> Result<RgbaImage, Report<ImageLoadError>> {
        // Check cache first.
        if let Some(img) = self.cache.lock().unwrap().get(path) {
            return Ok(img.clone());
        }

        // Load from disk.
        let img = image::open(path)
            .map_err(|_| Report::new(ImageLoadError).attach(format!("path: {}", path.display())))?
            .to_rgba8();

        self.cache
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), img.clone());

        Ok(img)
    }
}
