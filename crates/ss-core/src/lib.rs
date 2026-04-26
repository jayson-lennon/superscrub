//! SuperScrub core data model and interpolation engine.
//!
//! This crate defines the project configuration format, item types,
//! animation system, and interpolation logic used across all SuperScrub
//! components.
//!
//! # Architecture
//!
//! - [`project`] — Top-level project configuration structs
//! - [`item`] — Item definitions, sizing modes, item content types
//! - [`animation`] — Animation tracks, keyframes, easing curves
//! - [`interpolation`] — Easing functions, keyframe interpolation, item resolution
//! - [`transform`] — Resolved item state after interpolation
//! - [`path_resolve`] — Path resolution relative to project file
//! - [`watcher`] — Config file watching trait

pub mod animation;
pub mod interpolation;
pub mod item;
pub mod path_resolve;
pub mod project;
pub mod transform;
pub mod watcher;

#[doc(hidden)]
pub mod test_utils;

pub use animation::{AnimatableProperty, AnimationTrack, Easing, Keyframe};
pub use interpolation::{InterpolationError, apply_easing, interpolate_keyframes, resolve_items};
pub use item::{FitAnchor, FitMode, ItemContent, ItemDef, Sizing};
pub use path_resolve::{PathResolveError, resolve_path};
pub use project::loader::{ProjectLoadError, ProjectLoader};
pub use project::{AudioClipDef, EncodingConfig, Project};
pub use transform::ResolvedItem;
pub use watcher::{ConfigWatchError, ConfigWatcher};
