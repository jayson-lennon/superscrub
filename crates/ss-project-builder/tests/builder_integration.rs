//! Integration tests for the full builder pipeline.

use ss_core::Easing;
use ss_project_builder::*;
use ss_project_builder::clip_builder::sizing;

// ============================================================
// Round-trip test
// ============================================================

#[test]
fn builder_reproduces_example_project_json() {
    // Given the example project JSON.
    let example_json = std::fs::read_to_string("../../examples/basic_project.json")
        .expect("example project file should exist");

    // And the same project built via builders.
    let clip = ClipBuilder::new(
        ClipParams::builder()
            .id("background")
            .path("assets/cover.png")
            .end_time(30.0)
            .sizing(sizing::fit_rect_cover(0, 0, 1920, 1080))
            .build(),
    )
    .add_animation(
        AnimBuilder::scale_x()
            .keyframe(0.0, 1.0)
            .keyframe(30.0, 1.3),
    )
    .add_animation(
        AnimBuilder::scale_y()
            .keyframe(0.0, 1.0)
            .keyframe(30.0, 1.3),
    )
    .add_animation(
        AnimBuilder::translate_x()
            .keyframe_with_easing(0.0, 0.0, Easing::SineInOut)
            .keyframe_with_easing(30.0, -200.0, Easing::SineInOut),
    )
    .add_animation(
        AnimBuilder::opacity()
            .keyframe_with_easing(0.0, 0.0, Easing::SineInOut)
            .keyframe_with_easing(3.0, 1.0, Easing::SineInOut)
            .keyframe_with_easing(27.0, 1.0, Easing::SineInOut)
            .keyframe_with_easing(30.0, 0.0, Easing::SineInOut),
    );

    let audio_clip = AudioClipBuilder::new(
        AudioClipParams::builder()
            .id("background-music")
            .path("assets/song.mp3")
            .end_time(30.0)
            .build(),
    );

    let builder_json = ProjectBuilder::new(
        ProjectParams::builder()
            .resolution([1920, 1080])
            .fps(60)
            .duration(30.0)
            .output("output.mp4")
            .build(),
    )
    .add_clip(clip)
    .add_audio_clip(audio_clip)
    .to_json_string()
    .unwrap();

    // When parsing the example into a Project and re-serializing (normalizes defaults).
    let example_project: ss_core::Project =
        serde_json::from_str(&example_json).expect("example should parse as Project");
    let example_normalized = serde_json::to_string_pretty(&example_project).unwrap();
    let builder_normalized = builder_json;

    // Then parsing both as Value produces identical structures.
    let example_value: serde_json::Value =
        serde_json::from_str(&example_normalized).expect("example should be valid JSON");
    let builder_value: serde_json::Value =
        serde_json::from_str(&builder_normalized).expect("builder output should be valid JSON");

    assert_eq!(builder_value, example_value);
}

// ============================================================
// Error collection test
// ============================================================

#[test]
fn all_errors_collected_across_nested_builders() {
    // Given a project with multiple independent errors.
    let params = ProjectParams::builder()
        .resolution([1920, 1080])
        .fps(60)
        .duration(0.0) // error: duration must be > 0
        .output("out.mp4")
        .build();

    let bad_clip = ClipBuilder::new(
        ClipParams::builder()
            .id("bad-clip")
            .path("img.png")
            .start_time(10.0)
            .end_time(5.0) // error: end_time <= start_time
            .build(),
    )
    .add_animation(AnimBuilder::opacity()); // error: no keyframes

    let bad_audio = AudioClipBuilder::new(
        AudioClipParams::builder()
            .id("bad-audio")
            .path("song.mp3")
            .start_time(10.0)
            .end_time(5.0) // error: end_time <= start_time
            .build(),
    );

    // When building the project.
    let result = ProjectBuilder::new(params)
        .add_clip(bad_clip)
        .add_audio_clip(bad_audio)
        .build();

    // Then ALL errors are collected.
    let errors = result.expect_err("should fail with multiple errors");
    let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();

    // We expect 4 errors: 1 project-level (duration), 2 clip-level (time range + empty keyframes), 1 audio-level (time range)
    assert_eq!(messages.len(), 4, "should have exactly 4 errors: {messages:?}");

    let combined = messages.join("\n");
    assert!(
        combined.contains("duration must be greater than 0"),
        "missing duration error: {combined}"
    );
    assert!(
        combined.contains("clip \"bad-clip\""),
        "missing clip context: {combined}"
    );
    assert!(
        combined.contains("end_time must be greater than start_time"),
        "missing time range error: {combined}"
    );
    assert!(
        combined.contains("no keyframes"),
        "missing keyframe error: {combined}"
    );
    assert!(
        combined.contains("audio clip \"bad-audio\""),
        "missing audio clip context: {combined}"
    );
}

// ============================================================
// Serialization round-trip test
// ============================================================

#[test]
fn built_project_round_trips_through_serde() {
    // Given a project built via builders with clips and audio.
    let clip = ClipBuilder::new(
        ClipParams::builder()
            .id("test")
            .path("img.png")
            .end_time(10.0)
            .build(),
    )
    .add_animation(
        AnimBuilder::opacity()
            .keyframe(0.0, 0.0)
            .keyframe(5.0, 1.0),
    );

    let audio = AudioClipBuilder::new(
        AudioClipParams::builder()
            .id("music")
            .path("song.mp3")
            .end_time(10.0)
            .build(),
    );

    let json = ProjectBuilder::new(
        ProjectParams::builder()
            .resolution([1280, 720])
            .fps(30)
            .duration(10.0)
            .output("out.mp4")
            .build(),
    )
    .add_clip(clip)
    .add_audio_clip(audio)
    .to_json_string()
    .unwrap();

    // When parsing back via serde.
    let project: ss_core::Project = serde_json::from_str(&json).unwrap();

    // Then all fields survive the round trip.
    assert_eq!(project.resolution, [1280, 720]);
    assert_eq!(project.fps, 30);
    assert!((project.duration - 10.0).abs() < 1e-5);
    assert_eq!(project.output, "out.mp4");
    assert_eq!(project.clips.len(), 1);
    assert_eq!(project.clips[0].id, "test");
    assert_eq!(project.clips[0].animations.len(), 1);
    assert_eq!(project.clips[0].animations[0].keyframes.len(), 2);
    assert_eq!(project.audio_clips.len(), 1);
    assert_eq!(project.audio_clips[0].id, "music");
}
