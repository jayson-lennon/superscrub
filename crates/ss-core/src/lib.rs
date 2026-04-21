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
//! - [`watcher`] — Config file watching trait

pub mod animation;
pub mod clip;
pub mod interpolation;
pub mod path_resolve;
pub mod project;
pub mod transform;
pub mod watcher;

#[doc(hidden)]
pub mod test_utils;

pub use animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
pub use clip::{ClipDef, ClipType, FitAnchor, FitMode, Sizing};
pub use interpolation::{InterpolationError, apply_easing, interpolate_keyframes, resolve_clip};
pub use path_resolve::{PathResolveError, resolve_path};
pub use project::loader::{ProjectLoadError, ProjectLoader};
pub use project::{AudioClipDef, EncodingConfig, Project};
pub use transform::ResolvedClip;
pub use watcher::{ConfigWatchError, ConfigWatcher};
