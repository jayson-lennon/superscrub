//! Service wrapper for [`ImageProvider`].

use std::sync::Arc;

use derive_more::Debug;

use crate::image::ImageProvider;

/// Service wrapper for [`ImageProvider`].
#[derive(Debug, Clone)]
pub struct ImageProviderService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added when needed
    backend: Arc<dyn ImageProvider>,
}

impl ImageProviderService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn ImageProvider>) -> Self {
        Self { backend }
    }
}
