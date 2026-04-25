//! Validation error types for the project builder.
//!
//! [`BuilderError`] represents a single validation failure with optional
//! path context describing where in the project hierarchy the error occurred.
//! [`BuilderErrors`] collects multiple errors for batch reporting.

use std::fmt;

/// A single validation error with optional path context.
///
/// Path segments describe where in the project hierarchy the error occurred,
/// e.g., `["clip \"background\"", "animation \"opacity\""]`.
///
/// Use [`BuilderError::in_context`] to prepend path segments when propagating
/// errors up the builder hierarchy.
#[derive(Debug, Clone)]
pub struct BuilderError {
    message: String,
    path: Vec<String>,
}

impl BuilderError {
    /// Creates a new error with the given message and no path context.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: vec![],
        }
    }

    /// Prepends a path segment to provide hierarchical context.
    ///
    /// Call this when propagating an error from a nested builder to its parent:
    ///
    /// ```ignore
    /// let err = BuilderError::new("empty keyframes")
    ///     .in_context("animation \"opacity\"")
    ///     .in_context("clip \"background\"");
    /// // Displays as: clip "background" → animation "opacity": empty keyframes
    /// ```
    #[must_use]
    pub fn in_context(mut self, segment: impl Into<String>) -> Self {
        self.path.insert(0, segment.into());
        self
    }
}

impl fmt::Display for BuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path.join(" → "), self.message)
        }
    }
}

impl std::error::Error for BuilderError {}

/// Multiple validation errors collected during a build.
///
/// Accumulates errors from all builders rather than failing on the first one,
/// so consumers see every issue in a single pass.
#[derive(Debug, Clone)]
pub struct BuilderErrors {
    errors: Vec<BuilderError>,
}

impl BuilderErrors {
    /// Creates an empty error collection.
    pub fn new() -> Self {
        Self { errors: vec![] }
    }

    /// Appends a single error to the collection.
    pub fn push(&mut self, error: BuilderError) {
        self.errors.push(error);
    }

    /// Returns `true` if no errors have been collected.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Converts into `Ok(())` if empty, or `Err(self)` if errors were collected.
    pub fn into_result(self) -> Result<(), Self> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }

    /// Merges another [`BuilderErrors`] into this one, appending all its errors.
    pub fn merge(&mut self, other: BuilderErrors) {
        self.errors.extend(other.errors);
    }

    /// Returns an iterator over the collected errors.
    pub fn iter(&self) -> impl Iterator<Item = &BuilderError> {
        self.errors.iter()
    }
}

impl fmt::Display for BuilderErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, err) in self.errors.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "  {}. {err}", i + 1)?;
        }
        Ok(())
    }
}

impl std::error::Error for BuilderErrors {}

impl Default for BuilderErrors {
    fn default() -> Self {
        Self::new()
    }
}
