//! Audio clip construction.
//!
//! Provides [`AudioClipParams`] (a `bon`-generated typestate builder for compile-time
//! enforcement of required fields) and [`AudioClipBuilder`] (which wraps a fully-built
//! `AudioClipParams` and adds runtime validation).
//!
//! # Usage
//!
//! ```ignore
//! use ss_project_builder::{AudioClipBuilder, AudioClipParams};
//!
//! let clip = AudioClipBuilder::new(
//!     AudioClipParams::builder()
//!         .id("bg-music")
//!         .path("song.mp3")
//!         .end_time(30.0)
//!         .build()
//! )
//! .build()
//! .unwrap();
//! ```

use ss_core::AudioClipDef;

use crate::{BuilderError, BuilderErrors};

/// Parameters managed by `bon`'s typestate builder.
///
/// Required fields (`id`, `path`, `end_time`) are enforced at compile time —
/// you cannot call `.build()` without setting them. All other fields have
/// sensible defaults matching [`AudioClipDef`]'s serde defaults.
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct AudioClipParams {
    /// Unique identifier for reference.
    pub id: String,
    /// Path to the audio file, relative to the project.
    pub path: String,
    /// Which audio track this clip occupies.
    #[builder(default = 0)]
    pub track: u32,
    /// When this clip starts playing (seconds).
    #[builder(default = 0.0)]
    pub start_time: f64,
    /// When this clip stops playing (seconds).
    pub end_time: f64,
    /// Volume level. Default: 1.0.
    #[builder(default = 1.0)]
    pub volume: f32,
    /// Offset into the source audio file where playback begins (seconds).
    #[builder(default = 0.0)]
    pub source_offset: f64,
    /// Offset into the source audio file where playback ends (seconds).
    /// A value of 0.0 means play to the end of the source file.
    #[builder(default = 0.0)]
    pub trim_end: f64,
}

/// Builder for constructing an [`AudioClipDef`] with runtime validation.
///
/// Wraps a fully-built [`AudioClipParams`] (enforced by `bon` at compile time).
///
/// Call [`build`](AudioClipBuilder::build) to validate and produce an `AudioClipDef`.
pub struct AudioClipBuilder {
    params: AudioClipParams,
}

impl AudioClipBuilder {
    /// Creates a new `AudioClipBuilder` from fully-built [`AudioClipParams`].
    ///
    /// Use `AudioClipParams::builder().id(...).path(...).end_time(...).build()` to
    /// construct the params — `bon` enforces required fields at compile time.
    pub fn new(params: AudioClipParams) -> Self {
        Self { params }
    }

    /// Consumes the builder, validates, and returns an [`AudioClipDef`].
    ///
    /// # Validation
    ///
    /// - `end_time` must be greater than `start_time`
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if validation fails.
    pub fn build(self) -> Result<AudioClipDef, BuilderErrors> {
        let mut errors = BuilderErrors::new();
        let clip_id = &self.params.id;
        let clip_ctx = format!("audio clip \"{clip_id}\"");

        if self.params.end_time <= self.params.start_time {
            errors.push(
                BuilderError::new("end_time must be greater than start_time").in_context(&clip_ctx),
            );
        }

        errors.into_result()?;

        Ok(AudioClipDef {
            id: self.params.id,
            path: self.params.path,
            track: self.params.track,
            start_time: self.params.start_time,
            end_time: self.params.end_time,
            volume: self.params.volume,
            source_offset: self.params.source_offset,
            trim_end: self.params.trim_end,
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    /// Creates a minimal valid `AudioClipParams` for test reuse.
    fn minimal_params(id: &str, path: &str, end_time: f64) -> AudioClipParams {
        AudioClipParams::builder()
            .id(id)
            .path(path)
            .end_time(end_time)
            .build()
    }

    #[test]
    fn build_minimal_audio_clip_with_required_fields() {
        // Given an AudioClipBuilder with only required fields.
        let params = minimal_params("bg-music", "song.mp3", 30.0);

        // When building.
        let clip = AudioClipBuilder::new(params).build().unwrap();

        // Then defaults are applied correctly.
        assert_eq!(clip.id, "bg-music");
        assert_eq!(clip.path, "song.mp3");
        assert_eq!(clip.track, 0);
        assert_eq!(clip.start_time, 0.0);
        assert_eq!(clip.end_time, 30.0);
        assert_eq!(clip.volume, 1.0);
    }

    #[test]
    fn build_audio_clip_with_all_optional_fields() {
        // Given an AudioClipBuilder with all optional fields set.
        let params = AudioClipParams::builder()
            .id("sfx")
            .path("beep.wav")
            .track(2)
            .start_time(5.0)
            .end_time(10.0)
            .volume(0.5)
            .build();

        // When building.
        let clip = AudioClipBuilder::new(params).build().unwrap();

        // Then all values are reflected in the output.
        assert_eq!(clip.id, "sfx");
        assert_eq!(clip.path, "beep.wav");
        assert_eq!(clip.track, 2);
        assert_eq!(clip.start_time, 5.0);
        assert_eq!(clip.end_time, 10.0);
        assert_eq!(clip.volume, 0.5);
    }

    #[test]
    fn build_audio_clip_with_invalid_time_range() {
        // Given an AudioClipBuilder where end_time < start_time.
        let params = AudioClipParams::builder()
            .id("test")
            .path("song.mp3")
            .start_time(10.0)
            .end_time(5.0)
            .build();

        // When building.
        let result = AudioClipBuilder::new(params).build();

        // Then the error has correct path context.
        let errors = result.expect_err("should fail with invalid time range");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(
            msg.contains("audio clip \"test\""),
            "error should mention clip: {msg}"
        );
        assert!(
            msg.contains("end_time must be greater than start_time"),
            "error should describe time issue: {msg}"
        );
    }

    #[test]
    fn build_audio_clip_with_zero_duration() {
        // Given an AudioClipBuilder where end_time == start_time.
        let params = AudioClipParams::builder()
            .id("test")
            .path("song.mp3")
            .start_time(5.0)
            .end_time(5.0)
            .build();

        // When building.
        let result = AudioClipBuilder::new(params).build();

        // Then the error is returned (equal is not strictly greater).
        let errors = result.expect_err("should fail with zero duration");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(
            msg.contains("end_time must be greater than start_time"),
            "error should describe time issue: {msg}"
        );
    }

    #[test]
    fn bon_builder_accepts_string_into_for_id_and_path() {
        // Given a builder using &str for id and path (tests #[builder(on(String, into))]).
        let params = AudioClipParams::builder()
            .id("test") // &str, not String
            .path("song.mp3")
            .end_time(5.0)
            .build();

        // When building the audio clip.
        let clip = AudioClipBuilder::new(params).build().unwrap();

        // Then the strings are properly owned.
        assert_eq!(clip.id, "test");
        assert_eq!(clip.path, "song.mp3");
    }
}
