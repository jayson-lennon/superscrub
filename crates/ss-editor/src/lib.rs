//! SuperScrub egui-based video editor.
//!
//! Provides a viewport, timeline, transport controls, and settings panel
//! for previewing and scrubbing through video projects.
//!
//! # Architecture
//!
//! All non-UI logic lives in library modules behind traits for testability:
//!
//! - [`app`] — `EditorApp` (eframe::App impl), thin UI shell
//! - [`app_config`] — Persistent settings via `dirs` crate
//! - [`editor_state`] — Core state: current time, project, transport state
//! - [`playback_controller`] — Coordinates time, audio, and preview cache
//! - [`project_watcher`] — Detects project file changes
//! - [`services`] — Service container for all injectable deps
//! - [`viewport_panel`] — Viewport display (frame → egui texture)
//! - [`timeline_panel`] — Timeline with tracks, clip blocks, playhead
//! - [`transport_panel`] — Play/pause/stop, time display, volume
//! - [`settings_panel`] — Preview resolution and FPS settings

pub mod app;
pub mod app_config;
pub mod config_watcher_notify;
pub mod editor_state;
pub mod playback_controller;
pub mod project_watcher;
pub mod services;
pub mod settings_panel;
pub mod timeline_panel;
pub mod transport_panel;
pub mod viewport_panel;

pub use app::EditorApp;
pub use app_config::AppConfig;
pub use editor_state::EditorState;
pub use editor_state::TransportState;
pub use playback_controller::PlaybackController;
pub use project_watcher::ProjectWatcher;
pub use services::Services;
pub use timeline_panel::TimelineAction;
