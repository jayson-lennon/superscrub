//! Trait definitions for project loading and file watching.
//!
//! These traits decouple the data model from filesystem operations,
//! enabling dependency injection and testing with fake implementations.

use std::path::Path;

use error_stack::Report;

use crate::errors::{ConfigWatchError, ProjectLoadError};
use crate::project::Project;

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

/// Watches a project file for changes on disk.
pub trait ConfigWatcher: Send + Sync {
    /// Returns the name of this watcher backend (for debugging).
    fn name(&self) -> &'static str;

    /// Start watching the given file for changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be watched.
    fn watch(&self, path: &Path) -> Result<(), Report<ConfigWatchError>>;

    /// Returns `true` if the watched file has changed since the last call.
    fn has_changed(&self) -> bool;
}
