//! Filesystem-backed image provider with caching.
//!
//! Loads images from disk and caches them in memory. Subsequent requests
//! for the same path return the cached image without disk I/O.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use error_stack::{Report, ResultExt};
use image::RgbaImage;
use tracing::debug;

use crate::image::ImageLoadError;
use crate::image::ImageProvider;

/// Loads images from the filesystem and caches them in memory.
pub struct FilesystemImageProvider {
    cache: Mutex<HashMap<PathBuf, Arc<RgbaImage>>>,
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

    fn get(&self, path: &Path) -> Result<Arc<RgbaImage>, Report<ImageLoadError>> {
        // Check cache first — Arc::clone is just a refcount bump (~8 bytes),
        // not a full pixel buffer copy (~8 MB at 1080p).
        if let Some(img) = self.cache.lock().unwrap().get(path) {
            return Ok(img.clone());
        }

        // Load from disk.
        let img = image::open(path)
            .change_context(ImageLoadError)
            .attach(format!("image path: {}", path.display()))?
            .to_rgba8();

        debug!("image loaded: {}", path.display());

        let arc = Arc::new(img);
        self.cache
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), arc.clone());

        Ok(arc)
    }
}
