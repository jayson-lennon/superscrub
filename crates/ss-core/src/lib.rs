//! SuperScrub core data model and interpolation engine.
//!
//! This crate defines the project configuration format, clip types,
//! animation system, and interpolation logic used across all SuperScrub
//! components.
//!
//! # Architecture
//!
//! - [`project`] — Top-level project configuration structs
//! - [`clip`] — Clip definitions, sizing modes, clip types
//! - [`animation`] — Animation tracks, keyframes, easing curves
//! - [`interpolation`] — Easing functions, keyframe interpolation, clip resolution
//! - [`transform`] — Resolved clip state after interpolation
//! - [`path_resolve`] — Path resolution relative to project file
//! - [`traits`] — Trait definitions for project loading and file watching

pub mod animation;
pub mod clip;
pub mod errors;
pub mod interpolation;
pub mod path_resolve;
pub mod project;
pub mod traits;
pub mod transform;

pub use animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
pub use clip::{ClipDef, ClipType, FitAnchor, FitMode, Sizing};
pub use errors::{ConfigWatchError, InterpolationError, PathResolveError, ProjectLoadError};
pub use interpolation::{apply_easing, interpolate_keyframes, resolve_clip};
pub use path_resolve::resolve_path;
pub use project::{AudioConfig, Project};
pub use traits::{ConfigWatcher, ProjectLoader};
pub use transform::ResolvedClip;
