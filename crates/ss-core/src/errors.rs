//! Error types for project loading and configuration.

use wherror::Error;

/// Failed to load or parse a project file.
#[derive(Debug, Error)]
#[error("failed to load project")]
pub struct ProjectLoadError;

/// Failed to resolve a file path.
#[derive(Debug, Error)]
#[error("failed to resolve path")]
pub struct PathResolveError;

/// Failed during interpolation.
#[derive(Debug, Error)]
#[error("interpolation error")]
pub struct InterpolationError;

/// Failed to watch a config file for changes.
#[derive(Debug, Error)]
#[error("failed to watch config file")]
pub struct ConfigWatchError;
