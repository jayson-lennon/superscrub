//! Programmatic video effects built on top of [`ss-project-builder`].
//!
//! Provides high-level effect builders that produce `Vec<ClipBuilder>` output,
//! composable via [`ProjectBuilder::add_clips_at`](ss_project_builder::ProjectBuilder).
//!
//! # Effects
//!
//! - **Ken Burns** — Pan and zoom across images with stack-based crossfade transitions.
//!   See the [`ken_burns`] module for details.
//!
//! # Transforms
//!
//! - **Opacity** — Scale opacity keyframe values across a batch of clips.
//!   See the [`opacity`] module for details.

pub mod ken_burns;
pub mod opacity;

pub use ken_burns::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
pub use opacity::opacity;
