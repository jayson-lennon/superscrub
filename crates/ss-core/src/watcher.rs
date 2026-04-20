//! Config file watching trait.
//!
//! Decouples file watching from the data model for testability.

use std::path::Path;

use error_stack::Report;

/// Failed to watch a config file for changes.
#[derive(Debug, wherror::Error)]
#[error("failed to watch config file")]
pub struct ConfigWatchError;

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
