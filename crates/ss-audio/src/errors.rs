//! Error types for audio operations.

use wherror::Error;

/// Failed to perform an audio operation.
#[derive(Debug, Error)]
#[error("audio engine error")]
pub struct AudioError;
