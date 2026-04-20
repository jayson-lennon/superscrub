//! Project loading trait.
//!
//! Decouples project loading from filesystem operations for testability.

use std::path::Path;

use error_stack::Report;

use crate::project::Project;

/// Failed to load or parse a project file.
#[derive(Debug, wherror::Error)]
#[error("failed to load project")]
pub struct ProjectLoadError;

/// Loads and parses a project file.
pub trait ProjectLoader: Send + Sync {
    /// Returns the name of this loader backend (for debugging).
    fn name(&self) -> &'static str;

    /// Load and parse a project from the given file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    fn load(&self, path: &Path) -> Result<Project, Report<ProjectLoadError>>;
}
