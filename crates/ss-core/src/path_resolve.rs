//! Path resolution relative to the project file.
//!
//! Paths in a project file are resolved relative to the project file's
//! parent directory. Absolute paths are returned as-is.

use std::path::{Path, PathBuf};

use error_stack::Report;

use crate::errors::PathResolveError;

/// Resolve a path relative to the project file's parent directory.
///
/// If `path` is absolute, returns it as-is.
/// If relative, joins it to the project file's parent directory.
///
/// # Errors
///
/// Returns an error if the project file has no parent directory.
pub fn resolve_path(project_file: &Path, path: &str) -> Result<PathBuf, Report<PathResolveError>> {
    let path = Path::new(path);

    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let parent = project_file.parent().ok_or_else(|| {
        Report::new(PathResolveError).attach("project file has no parent directory")
    })?;

    Ok(parent.join(path))
}
