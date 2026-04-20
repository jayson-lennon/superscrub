//! Audio playback engine.
//!
//! This crate provides audio playback with seeking capability
//! for synchronizing with the video preview. The [`AudioEngine`] trait
//! abstracts over audio backends, with [`RodioAudioEngine`] as the
//! real implementation and [`FakeAudioEngine`] for testing.
//!
//! # Architecture
//!
//! - [`engine`] — Audio engine trait, playback state, and error types
//! - [`engine::rodio`] — Rodio-based real implementation
//! - [`engine::fake`] — Test fake with call tracking
//! - [`engine::service`] — `AudioEngineService` wrapper

pub mod engine;

pub use engine::fake::FakeAudioEngine;
pub use engine::rodio::RodioAudioEngine;
pub use engine::service::AudioEngineService;
pub use engine::{AudioEngine, AudioError, AudioPlaybackState};
