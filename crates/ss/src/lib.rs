//! SuperScrub library facade.
//!
//! Re-exports all project builder types at the top level and exposes effects
//! under the [`effects`] module.
//!
//! # Usage
//!
//! ```ignore
//! use ss::{ProjectBuilder, ClipBuilder};
//! use ss::effects::KenBurnsBuilder;
//! ```

// Re-export project builder types at the top level for convenience.
pub use ss_project_builder::{
    AnimBuilder, AudioClipBuilder, AudioClipParams, BuilderError, BuilderErrors, ClipBuilder,
    ClipParams, GroupBuilder, GroupParams, ProjectBuilder, ProjectParams,
};

/// Re-export of the [`ss_project_builder`] crate for accessing submodules
/// like [`clip_builder::sizing`].
///
/// [`clip_builder::sizing`]: ss_project_builder::clip_builder::sizing
pub use ss_project_builder;

// Re-export core types so consumers don't need a separate ss-core dep.
pub use ss_core;

// Expose effects as a module: `use ss::effects::KenBurnsBuilder`.
pub mod effects {
    pub use ss_effects::*;
}
