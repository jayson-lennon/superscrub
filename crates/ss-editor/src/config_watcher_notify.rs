//! Real ConfigWatcher backed by the notify crate.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use derive_more::Debug;
use error_stack::Report;
use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ss_core::{ConfigWatchError, ConfigWatcher};

/// File-system watcher backed by the [`notify`] crate.
///
/// Watches a single config file for modifications or creation events and
/// exposes a polled `has_changed()` flag.
#[derive(Debug)]
pub struct NotifyConfigWatcher {
    /// Shared flag set by the watcher callback and cleared on poll.
    changed: Arc<AtomicBool>,
    /// The file being watched, used to filter events.
    watched_path: Mutex<Option<PathBuf>>,
    /// Held alive to keep watching.
    #[debug("NotifyConfigWatcher")]
    _watcher: Mutex<Option<RecommendedWatcher>>,
}

impl Default for NotifyConfigWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl NotifyConfigWatcher {
    /// Create a new watcher with no file being watched.
    pub fn new() -> Self {
        Self {
            changed: Arc::new(AtomicBool::new(false)),
            watched_path: Mutex::new(None),
            _watcher: Mutex::new(None),
        }
    }
}

impl ConfigWatcher for NotifyConfigWatcher {
    fn name(&self) -> &'static str {
        "notify"
    }

    /// Start watching the given file for changes.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying watcher cannot be created or if
    /// the path cannot be watched.
    fn watch(&self, path: &Path) -> Result<(), Report<ConfigWatchError>> {
        *self.watched_path.lock().unwrap() = Some(path.to_path_buf());

        let changed = self.changed.clone();
        let _watched = path.to_path_buf();

        tracing::info!(path = %path.display(), "starting file watcher");

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        tracing::debug!(
                            kind = ?event.kind,
                            paths = ?event.paths,
                            "notify event"
                        );
                        if matches!(
                            event.kind,
                            EventKind::Modify(_) | EventKind::Create(_)
                        ) {
                            changed.store(true, Ordering::Release);
                            tracing::info!("file change flag set");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("notify watcher error: {e}");
                    }
                }
            },
            NotifyConfig::default(),
        )
        .map_err(|e| Report::new(ConfigWatchError).attach(e))?;

        let watch_dir = path.parent().unwrap_or(path);
        tracing::info!(dir = %watch_dir.display(), "watching directory");

        watcher
            .watch(watch_dir, RecursiveMode::NonRecursive)
            .map_err(|e| Report::new(ConfigWatchError).attach(e))?;

        *self._watcher.lock().unwrap() = Some(watcher);
        Ok(())
    }

    fn has_changed(&self) -> bool {
        let changed = self.changed.swap(false, Ordering::AcqRel);
        if changed {
            tracing::info!("has_changed() returning true, flag cleared");
        }
        changed
    }
}
