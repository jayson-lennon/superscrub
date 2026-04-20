//! Service wrapper for the [`PreviewCache`] trait.

use std::sync::Arc;

use derive_more::Debug;

use crate::traits::PreviewCache;

/// Service wrapper for [`PreviewCache`].
#[derive(Debug, Clone)]
pub struct PreviewCacheService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added in Phase 4
    backend: Arc<dyn PreviewCache>,
}

impl PreviewCacheService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn PreviewCache>) -> Self {
        Self { backend }
    }
}
