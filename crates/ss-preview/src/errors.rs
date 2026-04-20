//! Error types for preview operations.

use wherror::Error;

/// Failed to perform a preview operation.
#[derive(Debug, Error)]
#[error("preview cache error")]
pub struct PreviewError;
