//! Audio playback engine.
//!
//! This crate provides audio playback with seeking capability
//! for synchronizing with the video preview. The [`AudioEngine`] trait
//! abstracts over audio backends, with [`CpalAudioEngine`] as the
//! real implementation and [`FakeAudioEngine`] for testing.
//!
//! # Architecture
//!
//! - [`decoder`] — Audio file decoder (symphonia + rubato resampling)
//! - [`engine`] — Audio engine trait, playback state, and error types
//! - [`engine::cpal`] — cpal-based real implementation (direct audio callback)
//! - [`engine::fake`] — Test fake with call tracking
//! - [`engine::service`] — `AudioEngineService` wrapper

pub mod decoder;
pub mod engine;

pub use decoder::{DecodeError, DecodedAudio, InterleavedSamples, PlanarSamples};
pub use engine::cpal::{CpalAudioEngine, CpalInitError};
pub use engine::fake::FakeAudioEngine;
pub use engine::service::AudioEngineService;
pub use engine::{AudioClipInfo, AudioEngine, AudioError, AudioPlaybackState, LoadedAudioClip};
