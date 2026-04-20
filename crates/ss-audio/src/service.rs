//! Service wrapper for the [`AudioEngine`] trait.

use std::sync::Arc;

use derive_more::Debug;

use crate::traits::AudioEngine;

/// Service wrapper for [`AudioEngine`].
#[derive(Debug, Clone)]
pub struct AudioEngineService {
    #[debug("backend<{}>", self.backend.name())]
    #[allow(dead_code)] // Read by Debug impl; delegation methods added in Phase 4
    backend: Arc<dyn AudioEngine>,
}

impl AudioEngineService {
    /// Create a new service wrapping the given backend.
    pub fn new(backend: Arc<dyn AudioEngine>) -> Self {
        Self { backend }
    }
}
