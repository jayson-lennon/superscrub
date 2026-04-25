//! Top-level project assembly, validation, and serialization.
//!
//! Provides [`ProjectParams`] (a `bon`-generated typestate builder for compile-time
//! enforcement of required fields) and [`ProjectBuilder`] (which wraps a fully-built
//! `ProjectParams` and adds clip/audio clip accumulation, cross-cutting validation,
//! and JSON serialization).
//!
//! # Usage
//!
//! ```ignore
//! use ss_project_builder::{ProjectBuilder, ProjectParams, ClipBuilder, ClipParams};
//!
//! let project = ProjectBuilder::new(
//!     ProjectParams::builder()
//!         .resolution([1920, 1080])
//!         .fps(60)
//!         .duration(30.0)
//!         .output("out.mp4")
//!         .build()
//! )
//! .add_clip(
//!     ClipBuilder::new(
//!         ClipParams::builder()
//!             .id("bg")
//!             .path("bg.png")
//!             .end_time(30.0)
//!             .build()
//!     )
//! )
//! .build()
//! .unwrap();
//! ```

use std::path::Path;

use ss_core::{EncodingConfig, Project};

use crate::{AudioClipBuilder, BuilderError, BuilderErrors, ClipBuilder};

/// Parameters managed by `bon`'s typestate builder.
///
/// Required fields (`resolution`, `fps`, `duration`, `output`) are enforced
/// at compile time. All other fields have sensible defaults.
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct ProjectParams {
    /// Output resolution [width, height] in pixels.
    pub resolution: [u32; 2],
    /// Output frames per second.
    pub fps: u32,
    /// Total duration in seconds.
    pub duration: f64,
    /// Output file path.
    pub output: String,
    /// Background color as [R, G, B, A]. Default: `[0x2c, 0x2e, 0x34, 0xff]` (dark gray).
    #[builder(default = [0x2c, 0x2e, 0x34, 0xff])]
    pub background: [u8; 4],
    /// Encoding settings. Default: [`EncodingConfig::default()`].
    #[builder(default = EncodingConfig::default())]
    pub encoding: EncodingConfig,
}

/// Top-level builder for assembling a [`Project`] from clips and audio clips.
///
/// Wraps a fully-built [`ProjectParams`] (enforced by `bon` at compile time)
/// and accumulates [`ClipBuilder`] and [`AudioClipBuilder`] instances.
///
/// Call [`build`](ProjectBuilder::build) to validate and produce a `Project`,
/// or use the convenience methods [`to_json_string`](ProjectBuilder::to_json_string)
/// and [`to_json_file`](ProjectBuilder::to_json_file) for direct serialization.
pub struct ProjectBuilder {
    params: ProjectParams,
    clip_builders: Vec<ClipBuilder>,
    audio_clip_builders: Vec<AudioClipBuilder>,
}

impl ProjectBuilder {
    /// Creates a new `ProjectBuilder` from fully-built [`ProjectParams`].
    pub fn new(params: ProjectParams) -> Self {
        Self {
            params,
            clip_builders: vec![],
            audio_clip_builders: vec![],
        }
    }

    /// Appends a [`ClipBuilder`] to the project's clip list.
    #[must_use]
    pub fn add_clip(mut self, clip: ClipBuilder) -> Self {
        self.clip_builders.push(clip);
        self
    }

    /// Appends multiple [`ClipBuilder`]s without time-shifting them.
    ///
    /// Equivalent to chaining multiple [`add_clip`](ProjectBuilder::add_clip) calls.
    #[must_use]
    pub fn add_clips(mut self, clips: impl IntoIterator<Item = ClipBuilder>) -> Self {
        self.clip_builders.extend(clips);
        self
    }

    /// Applies a time offset to each clip, then appends them to the project.
    ///
    /// This is the primary entry point for composing effects (which produce
    /// clips relative to t=0) into a timeline at a specific position.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let clips = ken_burns.build();
    /// project.add_clips_at(5.0, clips); // Ken Burns starts at t=5s
    /// ```
    #[must_use]
    pub fn add_clips_at(
        mut self,
        offset: f64,
        clips: impl IntoIterator<Item = ClipBuilder>,
    ) -> Self {
        self.clip_builders
            .extend(clips.into_iter().map(|c| c.with_offset(offset)));
        self
    }

    /// Appends an [`AudioClipBuilder`] to the project's audio clip list.
    #[must_use]
    pub fn add_audio_clip(mut self, clip: AudioClipBuilder) -> Self {
        self.audio_clip_builders.push(clip);
        self
    }

    /// Consumes the builder, validates, and returns a [`Project`].
    ///
    /// # Validation
    ///
    /// - `duration` must be greater than 0
    /// - `fps` must be greater than 0
    /// - `resolution` width and height must be greater than 0
    /// - All nested clip builders must pass their own validation
    /// - All nested audio clip builders must pass their own validation
    /// - All clip and audio clip time ranges must fit within the project duration
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if any validation rules are violated.
    pub fn build(self) -> Result<Project, BuilderErrors> {
        let mut errors = BuilderErrors::new();

        // Validate project-level fields.
        if self.params.duration <= 0.0 {
            errors.push(BuilderError::new("duration must be greater than 0"));
        }
        if self.params.fps == 0 {
            errors.push(BuilderError::new("fps must be greater than 0"));
        }
        if self.params.resolution[0] == 0 || self.params.resolution[1] == 0 {
            errors.push(BuilderError::new(
                "resolution width and height must be greater than 0",
            ));
        }

        // Build all clips, collecting errors.
        let mut clips = Vec::with_capacity(self.clip_builders.len());
        for clip_builder in self.clip_builders {
            match clip_builder.build() {
                Ok(clip) => clips.push(clip),
                Err(e) => errors.merge(e),
            }
        }

        // Build all audio clips, collecting errors.
        let mut audio_clips = Vec::with_capacity(self.audio_clip_builders.len());
        for audio_builder in self.audio_clip_builders {
            match audio_builder.build() {
                Ok(clip) => audio_clips.push(clip),
                Err(e) => errors.merge(e),
            }
        }

        // Validate time ranges against project duration (only if duration is valid).
        if self.params.duration > 0.0 {
            for clip in &clips {
                if clip.end_time > self.params.duration {
                    errors.push(
                        BuilderError::new(format!(
                            "clip end_time ({}) exceeds project duration ({})",
                            clip.end_time, self.params.duration
                        ))
                        .in_context(format!("clip \"{}\"", clip.id)),
                    );
                }
            }
            for clip in &audio_clips {
                if clip.end_time > self.params.duration {
                    errors.push(
                        BuilderError::new(format!(
                            "audio clip end_time ({}) exceeds project duration ({})",
                            clip.end_time, self.params.duration
                        ))
                        .in_context(format!("audio clip \"{}\"", clip.id)),
                    );
                }
            }
        }

        errors.into_result()?;

        Ok(Project {
            resolution: self.params.resolution,
            fps: self.params.fps,
            duration: std::time::Duration::from_secs_f64(self.params.duration),
            output: self.params.output,
            background: self.params.background,
            audio_clips,
            encoding: self.params.encoding,
            clips,
        })
    }

    /// Builds the project and serializes it to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if build validation fails.
    /// Returns `Err(BuilderErrors)` if serialization fails (wraps the serde error).
    pub fn to_json_string(self) -> Result<String, BuilderErrors> {
        let project = self.build()?;
        serde_json::to_string_pretty(&project).map_err(|e| {
            let mut errors = BuilderErrors::new();
            errors.push(BuilderError::new(format!("serialization failed: {e}")));
            errors
        })
    }

    /// Builds the project, serializes it to pretty-printed JSON, and writes to a file.
    ///
    /// # Errors
    ///
    /// Returns `Err(BuilderErrors)` if build validation fails.
    /// Returns `Err(BuilderErrors)` if serialization or file I/O fails.
    pub fn to_json_file(self, path: impl AsRef<Path>) -> Result<(), BuilderErrors> {
        let json = self.to_json_string()?;
        std::fs::write(path, json).map_err(|e| {
            let mut errors = BuilderErrors::new();
            errors.push(BuilderError::new(format!("failed to write file: {e}")));
            errors
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::{AnimBuilder, AudioClipParams, ClipParams};
    use ss_core::EncodingConfig;

    /// Creates a minimal valid `ProjectParams` for test reuse.
    fn minimal_params() -> ProjectParams {
        ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(30.0)
            .output("out.mp4")
            .build()
    }

    // ============================================================
    // Happy path
    // ============================================================

    #[test]
    fn build_minimal_project_with_required_fields() {
        // Given a ProjectBuilder with only required fields.
        let params = minimal_params();

        // When building.
        let project = ProjectBuilder::new(params).build().unwrap();

        // Then defaults are applied correctly.
        assert_eq!(project.resolution, [1920, 1080]);
        assert_eq!(project.fps, 60);
        assert!((project.duration.as_secs_f64() - 30.0).abs() < 1e-5);
        assert_eq!(project.output, "out.mp4");
        assert_eq!(project.background, [0x2c, 0x2e, 0x34, 0xff]);
        assert_eq!(project.encoding.crf, EncodingConfig::default().crf);
        assert_eq!(project.encoding.preset, EncodingConfig::default().preset);
        assert_eq!(
            project.encoding.pixel_format,
            EncodingConfig::default().pixel_format
        );
        assert!(project.clips.is_empty());
        assert!(project.audio_clips.is_empty());
    }

    #[test]
    fn build_project_with_all_optional_fields() {
        // Given a ProjectBuilder with background and encoding set.
        let params = ProjectParams::builder()
            .resolution([1280, 720])
            .fps(30)
            .duration(10.0)
            .output("out.mp4")
            .background([0xff, 0x00, 0x00, 0xff])
            .encoding(EncodingConfig {
                crf: 23,
                preset: "fast".into(),
                pixel_format: "yuv422p".into(),
            })
            .build();

        // When building.
        let project = ProjectBuilder::new(params).build().unwrap();

        // Then all values are reflected in the output.
        assert_eq!(project.resolution, [1280, 720]);
        assert_eq!(project.fps, 30);
        assert_eq!(project.background, [0xff, 0x00, 0x00, 0xff]);
        assert_eq!(project.encoding.crf, 23);
        assert_eq!(project.encoding.preset, "fast");
        assert_eq!(project.encoding.pixel_format, "yuv422p");
    }

    #[test]
    fn build_project_with_clips_and_audio_clips() {
        // Given a ProjectBuilder with one clip and one audio clip.
        let params = minimal_params();
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bg")
                .path("bg.png")
                .end_time(30.0)
                .build(),
        );
        let audio = AudioClipBuilder::new(
            AudioClipParams::builder()
                .id("music")
                .path("song.mp3")
                .end_time(30.0)
                .build(),
        );

        // When building.
        let project = ProjectBuilder::new(params)
            .add_clip(clip)
            .add_audio_clip(audio)
            .build()
            .unwrap();

        // Then both are present in the output.
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.clips[0].id, "bg");
        assert_eq!(project.audio_clips.len(), 1);
        assert_eq!(project.audio_clips[0].id, "music");
    }

    #[test]
    fn bon_builder_accepts_string_into_for_output() {
        // Given a builder using &str for output (tests #[builder(on(String, into))]).
        let params = ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(10.0)
            .output("out.mp4") // &str, not String
            .build();

        // When building the project.
        let project = ProjectBuilder::new(params).build().unwrap();

        // Then the string is properly owned.
        assert_eq!(project.output, "out.mp4");
    }

    // ============================================================
    // Validation failures
    // ============================================================

    #[test]
    fn build_project_with_zero_duration() {
        // Given a ProjectBuilder with duration = 0.
        let params = ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(0.0)
            .output("out.mp4")
            .build();

        // When building.
        let result = ProjectBuilder::new(params).build();

        // Then the error mentions duration.
        let errors = result.expect_err("should fail with zero duration");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("duration must be greater than 0"),
            "error should describe duration: {msg}"
        );
    }

    #[test]
    fn build_project_with_negative_duration() {
        // Given a ProjectBuilder with duration = -1.
        let params = ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(-1.0)
            .output("out.mp4")
            .build();

        // When building.
        let result = ProjectBuilder::new(params).build();

        // Then the error mentions duration.
        let errors = result.expect_err("should fail with negative duration");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("duration must be greater than 0"),
            "error should describe duration: {msg}"
        );
    }

    #[test]
    fn build_project_with_zero_fps() {
        // Given a ProjectBuilder with fps = 0.
        let params = ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(0)
            .duration(30.0)
            .output("out.mp4")
            .build();

        // When building.
        let result = ProjectBuilder::new(params).build();

        // Then the error mentions fps.
        let errors = result.expect_err("should fail with zero fps");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("fps must be greater than 0"),
            "error should describe fps: {msg}"
        );
    }

    #[test]
    fn build_project_with_zero_resolution_width() {
        // Given a ProjectBuilder with resolution = [0, 1080].
        let params = ProjectParams::builder()
            .resolution([0, 1080])
            .fps(60)
            .duration(30.0)
            .output("out.mp4")
            .build();

        // When building.
        let result = ProjectBuilder::new(params).build();

        // Then the error mentions resolution.
        let errors = result.expect_err("should fail with zero width");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("resolution width and height must be greater than 0"),
            "error should describe resolution: {msg}"
        );
    }

    #[test]
    fn build_project_with_zero_resolution_height() {
        // Given a ProjectBuilder with resolution = [1920, 0].
        let params = ProjectParams::builder()
            .resolution([1920, 0])
            .fps(60)
            .duration(30.0)
            .output("out.mp4")
            .build();

        // When building.
        let result = ProjectBuilder::new(params).build();

        // Then the error mentions resolution.
        let errors = result.expect_err("should fail with zero height");
        let msg = errors.iter().next().unwrap().to_string();
        assert!(
            msg.contains("resolution width and height must be greater than 0"),
            "error should describe resolution: {msg}"
        );
    }

    #[test]
    fn build_project_clip_exceeds_duration() {
        // Given a ProjectBuilder with a clip whose end_time > duration.
        let params = minimal_params();
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bg")
                .path("bg.png")
                .end_time(60.0) // exceeds 30.0 duration
                .build(),
        );

        // When building.
        let result = ProjectBuilder::new(params).add_clip(clip).build();

        // Then the error has correct path context.
        let errors = result.expect_err("should fail with clip exceeding duration");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(
            msg.contains("clip \"bg\""),
            "error should mention clip: {msg}"
        );
        assert!(
            msg.contains("exceeds project duration"),
            "error should describe duration issue: {msg}"
        );
    }

    #[test]
    fn build_project_audio_clip_exceeds_duration() {
        // Given a ProjectBuilder with an audio clip whose end_time > duration.
        let params = minimal_params();
        let audio = AudioClipBuilder::new(
            AudioClipParams::builder()
                .id("music")
                .path("song.mp3")
                .end_time(60.0) // exceeds 30.0 duration
                .build(),
        );

        // When building.
        let result = ProjectBuilder::new(params).add_audio_clip(audio).build();

        // Then the error has correct path context.
        let errors = result.expect_err("should fail with audio clip exceeding duration");
        let error = errors.iter().next().unwrap();
        let msg = error.to_string();
        assert!(
            msg.contains("audio clip \"music\""),
            "error should mention audio clip: {msg}"
        );
        assert!(
            msg.contains("exceeds project duration"),
            "error should describe duration issue: {msg}"
        );
    }

    #[test]
    fn build_project_collects_errors_from_nested_builders() {
        // Given a ProjectBuilder with multiple independent errors.
        let params = ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(0.0) // invalid duration
            .output("out.mp4")
            .build();
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bg")
                .path("bg.png")
                .start_time(10.0)
                .end_time(5.0) // invalid time range
                .build(),
        );
        let audio = AudioClipBuilder::new(
            AudioClipParams::builder()
                .id("music")
                .path("song.mp3")
                .start_time(10.0)
                .end_time(5.0) // invalid time range
                .build(),
        );

        // When building.
        let result = ProjectBuilder::new(params)
            .add_clip(clip)
            .add_audio_clip(audio)
            .build();

        // Then ALL errors are collected.
        let errors = result.expect_err("should fail with multiple errors");
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        assert!(
            messages.len() >= 3,
            "should have at least 3 errors (duration, clip time, audio time): {messages:?}"
        );
        let combined = messages.join("; ");
        assert!(
            combined.contains("duration must be greater than 0"),
            "should mention duration: {combined}"
        );
        assert!(
            combined.contains("clip \"bg\""),
            "should mention clip: {combined}"
        );
        assert!(
            combined.contains("audio clip \"music\""),
            "should mention audio clip: {combined}"
        );
    }

    // ============================================================
    // Serialization
    // ============================================================

    #[test]
    fn to_json_string_produces_valid_json() {
        // Given a minimal project.
        let params = minimal_params();

        // When serializing to JSON.
        let json = ProjectBuilder::new(params).to_json_string().unwrap();

        // Then the JSON round-trips through serde_json.
        let back: ss_core::Project = serde_json::from_str(&json).expect("should parse back");
        assert_eq!(back.resolution, [1920, 1080]);
        assert_eq!(back.fps, 60);
        assert!((back.duration.as_secs_f64() - 30.0).abs() < 1e-5);
        assert_eq!(back.output, "out.mp4");
    }

    #[test]
    fn to_json_file_writes_to_disk() {
        // Given a minimal project.
        let params = minimal_params();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");

        // When writing to file.
        ProjectBuilder::new(params).to_json_file(&path).unwrap();

        // Then the file exists, is valid JSON, and round-trips.
        let json = std::fs::read_to_string(&path).unwrap();
        let back: ss_core::Project = serde_json::from_str(&json).expect("should parse back");
        assert_eq!(back.resolution, [1920, 1080]);
        assert_eq!(back.fps, 60);
    }

    #[test]
    fn to_json_string_with_clips_round_trips() {
        // Given a project with clips and animations.
        let params = minimal_params();
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("bg")
                .path("bg.png")
                .end_time(30.0)
                .build(),
        )
        .add_animation(AnimBuilder::opacity().keyframe(0.0, 0.0).keyframe(3.0, 1.0));

        // When serializing to JSON and parsing back.
        let json = ProjectBuilder::new(params)
            .add_clip(clip)
            .to_json_string()
            .unwrap();
        let back: ss_core::Project = serde_json::from_str(&json).expect("should parse back");

        // Then the clip and animation survive the round trip.
        assert_eq!(back.clips.len(), 1);
        assert_eq!(back.clips[0].id, "bg");
        assert_eq!(back.clips[0].animations.len(), 1);
        assert_eq!(back.clips[0].animations[0].keyframes.len(), 2);
    }

    // ============================================================
    // Batch clip methods
    // ============================================================

    #[test]
    fn add_clips_adds_multiple_clips_at_once() {
        // Given a project and three clips.
        let params = minimal_params();
        let clip1 = ClipBuilder::new(
            ClipParams::builder()
                .id("a")
                .path("a.png")
                .end_time(5.0)
                .build(),
        );
        let clip2 = ClipBuilder::new(
            ClipParams::builder()
                .id("b")
                .path("b.png")
                .end_time(5.0)
                .build(),
        );
        let clip3 = ClipBuilder::new(
            ClipParams::builder()
                .id("c")
                .path("c.png")
                .end_time(5.0)
                .build(),
        );

        // When adding all three via add_clips.
        let project = ProjectBuilder::new(params)
            .add_clips([clip1, clip2, clip3])
            .build()
            .unwrap();

        // Then all three clips are present in order.
        assert_eq!(project.clips.len(), 3);
        assert_eq!(project.clips[0].id, "a");
        assert_eq!(project.clips[1].id, "b");
        assert_eq!(project.clips[2].id, "c");
    }

    #[test]
    fn add_clips_with_empty_iterator_is_no_op() {
        // Given a project with no clips.
        let params = minimal_params();

        // When adding an empty iterator.
        let project = ProjectBuilder::new(params)
            .add_clips(Vec::<ClipBuilder>::new())
            .build()
            .unwrap();

        // Then the project has no clips.
        assert!(project.clips.is_empty());
    }

    #[test]
    fn add_clips_followed_by_add_clip_preserves_order() {
        // Given a project.
        let params = minimal_params();
        let batch_clip = ClipBuilder::new(
            ClipParams::builder()
                .id("batch")
                .path("batch.png")
                .end_time(5.0)
                .build(),
        );
        let single_clip = ClipBuilder::new(
            ClipParams::builder()
                .id("single")
                .path("single.png")
                .end_time(5.0)
                .build(),
        );

        // When adding via add_clips then add_clip.
        let project = ProjectBuilder::new(params)
            .add_clips([batch_clip])
            .add_clip(single_clip)
            .build()
            .unwrap();

        // Then order is preserved: batch first, then single.
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[0].id, "batch");
        assert_eq!(project.clips[1].id, "single");
    }

    #[test]
    fn add_clips_at_shifts_all_clips_by_offset() {
        // Given two clips at t=0..10.
        let params = minimal_params();
        let clip1 = ClipBuilder::new(
            ClipParams::builder()
                .id("a")
                .path("a.png")
                .start_time(0.0)
                .end_time(10.0)
                .build(),
        );
        let clip2 = ClipBuilder::new(
            ClipParams::builder()
                .id("b")
                .path("b.png")
                .start_time(0.0)
                .end_time(10.0)
                .build(),
        );

        // When adding at offset 5.0.
        let project = ProjectBuilder::new(params)
            .add_clips_at(5.0, [clip1, clip2])
            .build()
            .unwrap();

        // Then both clips are shifted to t=5..15.
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[0].start_time, 5.0);
        assert_eq!(project.clips[0].end_time, 15.0);
        assert_eq!(project.clips[1].start_time, 5.0);
        assert_eq!(project.clips[1].end_time, 15.0);
    }

    #[test]
    fn add_clips_at_with_zero_offset_is_equivalent_to_add_clips() {
        // Given two identical sets of clips.
        let params1 = minimal_params();
        let params2 = minimal_params();

        let clips1 = vec![ClipBuilder::new(
            ClipParams::builder()
                .id("a")
                .path("a.png")
                .start_time(2.0)
                .end_time(8.0)
                .build(),
        )];
        let clips2 = vec![ClipBuilder::new(
            ClipParams::builder()
                .id("a")
                .path("a.png")
                .start_time(2.0)
                .end_time(8.0)
                .build(),
        )];

        // When using add_clips vs add_clips_at(0.0).
        let p1 = ProjectBuilder::new(params1)
            .add_clips(clips1)
            .build()
            .unwrap();
        let p2 = ProjectBuilder::new(params2)
            .add_clips_at(0.0, clips2)
            .build()
            .unwrap();

        // Then start/end times are identical.
        assert_eq!(p1.clips[0].start_time, p2.clips[0].start_time);
        assert_eq!(p1.clips[0].end_time, p2.clips[0].end_time);
    }

    #[test]
    fn add_clips_at_shifts_keyframe_times() {
        // Given a clip with an opacity keyframe at t=3.
        let params = minimal_params();
        let clip = ClipBuilder::new(
            ClipParams::builder()
                .id("anim")
                .path("img.png")
                .end_time(10.0)
                .build(),
        )
        .add_animation(AnimBuilder::opacity().keyframe(3.0, 1.0));

        // When adding at offset 7.0.
        let project = ProjectBuilder::new(params)
            .add_clips_at(7.0, [clip])
            .build()
            .unwrap();

        // Then the keyframe time is shifted to 10.0.
        assert_eq!(project.clips[0].start_time, 7.0);
        assert_eq!(project.clips[0].end_time, 17.0);
        assert_eq!(project.clips[0].animations[0].keyframes[0].time, 10.0);
    }

    #[test]
    fn add_clips_at_with_empty_iterator_is_no_op() {
        // Given a project.
        let params = minimal_params();

        // When adding empty clips at offset 5.0.
        let project = ProjectBuilder::new(params)
            .add_clips_at(5.0, Vec::<ClipBuilder>::new())
            .build()
            .unwrap();

        // Then the project has no clips.
        assert!(project.clips.is_empty());
    }
}
