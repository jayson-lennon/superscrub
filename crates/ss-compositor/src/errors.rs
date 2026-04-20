//! Error types for compositing operations.

use wherror::Error;

/// Failed to render a frame.
#[derive(Debug, Error)]
#[error("failed to render frame")]
pub struct CompositorError;

/// Failed to load an image.
#[derive(Debug, Error)]
#[error("failed to load image")]
pub struct ImageLoadError;
