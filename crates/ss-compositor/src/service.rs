//! Service wrappers for compositor traits.
//!
//! Service wrappers hold `Arc<dyn Trait>` for shared ownership and
//! provide the standard dependency injection pattern used across
//! SuperScrub.

use std::sync::Arc;

use derive_more::Debug;

use crate::traits::{FrameRenderer, ImageProvider};

/// Service wrapper for [`FrameRenderer`].
#[derive(Debug, Clone)]
pub struct FrameRendererService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added in Phase 4
    backend: Arc<dyn FrameRenderer>,
}

impl FrameRendererService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn FrameRenderer>) -> Self {
        Self { backend }
    }
}

/// Service wrapper for [`ImageProvider`].
#[derive(Debug, Clone)]
pub struct ImageProviderService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added in Phase 4
    backend: Arc<dyn ImageProvider>,
}

impl ImageProviderService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn ImageProvider>) -> Self {
        Self { backend }
    }
}
