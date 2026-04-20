//! Watches the project file for changes and signals when a reload is needed.
//!
//! Uses `ss_core::ConfigWatcher` trait for testability. The editor polls
//! `has_changed()` each frame in the update loop.

use std::path::Path;
use std::sync::Arc;

use error_stack::Report;
use ss_core::ConfigWatcher;

/// Failed to watch the project file.
#[derive(Debug, wherror::Error)]
#[error("project watcher error")]
pub struct ProjectWatcherError;

/// Watches a project file for changes on disk.
///
/// Wraps `ConfigWatcher` and tracks whether a reload is needed.
pub struct ProjectWatcher {
    watcher: Arc<dyn ConfigWatcher>,
    watching: bool,
}

impl ProjectWatcher {
    /// Create a new watcher wrapping the given backend.
    pub fn new(watcher: Arc<dyn ConfigWatcher>) -> Self {
        Self {
            watcher,
            watching: false,
        }
    }

    /// Start watching the given project file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be watched.
    pub fn start_watching(&mut self, path: &Path) -> Result<(), Report<ProjectWatcherError>> {
        self.watcher
            .watch(path)
            .map_err(|e| e.change_context(ProjectWatcherError))?;
        self.watching = true;
        Ok(())
    }

    /// Poll for file changes. Returns `true` if the project file was modified
    /// since the last call.
    ///
    /// Returns `false` if not watching or no change detected.
    pub fn poll(&self) -> bool {
        if !self.watching {
            return false;
        }
        self.watcher.has_changed()
    }
}
