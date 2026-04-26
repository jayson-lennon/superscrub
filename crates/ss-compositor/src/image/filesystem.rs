//! Filesystem-backed image provider with caching.
//!
//! Loads images from disk and caches them in memory. Subsequent requests
//! for the same path return the cached image without disk I/O.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use error_stack::{Report, ResultExt};
use image::RgbaImage;
use tracing::debug;

use crate::image::ImageLoadError;
use crate::image::ImageProvider;

/// Loads images from the filesystem and caches them in memory.
pub struct FilesystemImageProvider {
    cache: RwLock<HashMap<PathBuf, Arc<RgbaImage>>>,
}

impl FilesystemImageProvider {
    /// Create a new filesystem image provider with an empty cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
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
        // Check cache first — read lock allows concurrent cache lookups across threads.
        // Arc::clone is just a refcount bump (~8 bytes), not a full pixel buffer copy (~8 MB).
        {
            let cache = self.cache.read().unwrap();
            if let Some(img) = cache.get(path) {
                return Ok(img.clone());
            }
        }

        // Load from disk — no lock held during I/O.
        let img = image::open(path)
            .change_context(ImageLoadError)
            .attach(format!("image path: {}", path.display()))?
            .to_rgba8();

        debug!("image loaded: {}", path.display());

        let arc = Arc::new(img);
        self.cache
            .write()
            .unwrap()
            .insert(path.to_path_buf(), arc.clone());

        Ok(arc)
    }
}
