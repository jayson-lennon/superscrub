//! Project builder API for constructing SuperScrub projects programmatically.
//!
//! Provides type-safe builders using [`bon`] for compile-time enforcement
//! of required fields, with deferred runtime validation that collects all
//! errors into a single report.
//!
//! # Architecture
//!
//! - [`project_builder`] — Top-level project assembly, validation, and serialization
//! - [`clip_builder`] — Clip construction with sizing and animations
//! - [`anim_builder`] — Animation track and keyframe builders
//! - [`audio_clip_builder`] — Audio clip construction
//! - [`validation`] — Error types for builder validation

pub mod anim_builder;
pub mod audio_clip_builder;
pub mod clip_builder;
pub mod group_builder;
pub mod project_builder;
pub mod validation;

pub use anim_builder::AnimBuilder;
pub use audio_clip_builder::{AudioClipBuilder, AudioClipParams};
pub use clip_builder::{ClipBuilder, ClipParams};
pub use group_builder::{GroupBuilder, GroupParams};
pub use project_builder::{ProjectBuilder, ProjectParams};
pub use ss_core;
pub use validation::{BuilderError, BuilderErrors};
