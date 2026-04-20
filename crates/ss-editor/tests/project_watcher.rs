use std::sync::Arc;

use error_stack::Report;
use ss_core::ConfigWatchError;
use ss_core::ConfigWatcher;
use ss_editor::ProjectWatcher;

/// A fake config watcher that tracks whether `has_changed` returns true.
struct FakeWatcher {
    changed: std::sync::Mutex<bool>,
}

impl FakeWatcher {
    fn new() -> Self {
        Self {
            changed: std::sync::Mutex::new(false),
        }
    }

    fn set_changed(&self, value: bool) {
        *self.changed.lock().unwrap() = value;
    }
}

impl ConfigWatcher for FakeWatcher {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn watch(&self, _path: &std::path::Path) -> Result<(), Report<ConfigWatchError>> {
        Ok(())
    }

    fn has_changed(&self) -> bool {
        *self.changed.lock().unwrap()
    }
}

#[test]
fn new_watcher_reports_no_change() {
    // Given a watcher that hasn't started watching.
    let watcher = ProjectWatcher::new(Arc::new(FakeWatcher::new()));

    // Then poll returns false (not watching yet).
    assert!(!watcher.poll());
}

#[test]
fn poll_returns_true_when_changed() {
    // Given a watcher with a fake that reports changes.
    let fake = Arc::new(FakeWatcher::new());
    let mut watcher = ProjectWatcher::new(fake.clone());

    // When watching is started and a change is signaled.
    let _ = watcher.start_watching(std::path::Path::new("/test/project.json"));
    fake.set_changed(true);

    // Then poll returns true.
    assert!(watcher.poll());
}

#[test]
fn poll_returns_false_before_start_watching() {
    // A watcher that hasn't started watching always returns false.
    let fake = Arc::new(FakeWatcher::new());
    fake.set_changed(true);

    let watcher = ProjectWatcher::new(fake);
    assert!(!watcher.poll());
}
