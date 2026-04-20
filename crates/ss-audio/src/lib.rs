//! Audio playback engine.
//!
//! This crate provides audio playback with seeking capability
//! for synchronizing with the video preview. The [`AudioEngine`] trait
//! abstracts over audio backends, with [`RodioAudioEngine`] as the
//! real implementation and [`FakeAudioEngine`] for testing.
//!
//! # Architecture
//!
//! - [`traits`] — `AudioEngine` trait + `PlaybackState` enum
//! - [`rodio_engine`] — Rodio-based real implementation
//! - [`fake_engine`] — Test fake with call tracking
//! - [`service`] — `AudioEngineService` wrapper
//! - [`errors`] — `AudioError`

pub mod errors;
pub mod fake_engine;
pub mod rodio_engine;
pub mod service;
pub mod traits;

pub use errors::AudioError;
pub use fake_engine::FakeAudioEngine;
pub use rodio_engine::RodioAudioEngine;
pub use service::AudioEngineService;
pub use traits::{AudioEngine, PlaybackState};
