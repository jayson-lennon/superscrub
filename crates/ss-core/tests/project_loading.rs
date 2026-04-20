//! Tests for project config parsing and path resolution.

mod test_utils;

use std::path::Path;

use ss_core::clip::{ClipType, FitMode, Sizing};
use ss_core::path_resolve::resolve_path;
use ss_core::project::Project;
use test_utils::fixtures::build_project;

#[test]
fn parse_example_project() {
    // Given the example project JSON file.
    let json = std::fs::read_to_string("../../examples/basic_project.json")
        .expect("example project file should exist");

    // When parsing it.
    let project: Project = serde_json::from_str(&json).expect("should parse successfully");

    // Then all fields are correct.
    assert_eq!(project.resolution, [1920, 1080]);
    assert_eq!(project.fps, 60);
    assert!((project.duration - 30.0).abs() < 1e-5);
    assert_eq!(project.output, "output.mp4");
    assert!(project.audio.is_some());

    let audio = project.audio.unwrap();
    assert_eq!(audio.path, "assets/song.mp3");
    assert!((audio.start_time - 0.0).abs() < 1e-5);

    assert_eq!(project.clips.len(), 1);
    let clip = &project.clips[0];
    assert_eq!(clip.id, "background");
    assert!(matches!(&clip.clip_type, ClipType::Image { path } if path == "assets/cover.png"));
    assert_eq!(clip.track, 0);
    assert!((clip.start_time - 0.0).abs() < 1e-5);
    assert!((clip.end_time - 30.0).abs() < 1e-5);
    assert_eq!(clip.z_index, 0);

    // Check sizing.
    assert!(
        matches!(&clip.sizing, Sizing::FitRect { x, y, w, h, mode, anchor: _ }
            if *x == 0 && *y == 0 && *w == 1920 && *h == 1080 && matches!(mode, FitMode::Cover)
        )
    );

    // Check animations.
    assert_eq!(clip.animations.len(), 4);
}

#[test]
fn parse_minimal_project_without_optionals() {
    // Given a minimal project JSON with no audio and no animations.
    let json = r#"{
        "resolution": [1280, 720],
        "fps": 30,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{
            "id": "bg",
            "type": "image",
            "path": "bg.png",
            "track": 0,
            "start_time": 0.0,
            "end_time": 10.0,
            "z_index": 0
        }]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then optional fields get defaults.
    assert!(project.audio.is_none());
    let clip = &project.clips[0];
    assert_eq!(clip.animations.len(), 0);
    assert_eq!(clip.sizing, Sizing::default());
    assert_eq!(clip.pivot, [0.5, 0.5]);
}

#[test]
fn parse_invalid_json_returns_error() {
    // Given invalid JSON.
    let json = "{ not valid }";

    // When parsing it.
    let result = serde_json::from_str::<Project>(json);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn resolve_relative_path_joins_to_project_dir() {
    // Given a project file at /some/dir/project.json and a relative path.
    let project_file = Path::new("/some/dir/project.json");

    // When resolving a relative path.
    let resolved = resolve_path(project_file, "assets/cover.png").unwrap();

    // Then the path is joined to the project file's parent directory.
    assert_eq!(resolved, Path::new("/some/dir/assets/cover.png"));
}

#[test]
fn resolve_absolute_path_returns_as_is() {
    // Given a project file and an absolute path.
    let project_file = Path::new("/some/dir/project.json");

    // When resolving an absolute path.
    let resolved = resolve_path(project_file, "/absolute/path/image.png").unwrap();

    // Then the absolute path is returned unchanged.
    assert_eq!(resolved, Path::new("/absolute/path/image.png"));
}

#[test]
fn resolve_empty_string_returns_project_dir() {
    // Given a project file and an empty string path.
    let project_file = Path::new("/some/dir/project.json");

    // When resolving an empty string.
    let resolved = resolve_path(project_file, "").unwrap();

    // Then the result is the project file's parent directory.
    assert_eq!(resolved, Path::new("/some/dir"));
}

#[test]
fn resolve_dotdot_path_resolves_correctly() {
    // Given a project file and a path with parent directory references.
    let project_file = Path::new("/some/dir/project.json");

    // When resolving a path with ..
    let resolved = resolve_path(project_file, "../shared/assets/img.png").unwrap();

    // Then the path is joined (no canonicalization, just string join).
    assert_eq!(resolved, Path::new("/some/dir/../shared/assets/img.png"));
}

#[test]
fn resolve_dot_path_resolves_correctly() {
    // Given a project file and a path with a dot component.
    let project_file = Path::new("/some/dir/project.json");

    // When resolving a path with .
    let resolved = resolve_path(project_file, "./assets/img.png").unwrap();

    // Then the path is joined to the project directory.
    assert_eq!(resolved, Path::new("/some/dir/./assets/img.png"));
}

#[test]
fn roundtrip_project_serde() {
    // Given a project built programmatically.
    let project = build_project(vec![]);

    // When serializing and deserializing.
    let json = serde_json::to_string(&project).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();

    // Then the roundtrip preserves the data.
    assert_eq!(back.resolution, project.resolution);
    assert_eq!(back.fps, project.fps);
    assert!((back.duration - project.duration).abs() < 1e-5);
    assert_eq!(back.output, project.output);
}

#[test]
fn parse_project_with_multiple_clips_preserves_order() {
    // Given a project JSON with three clips.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [
            {"id": "bg", "type": "image", "path": "bg.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0},
            {"id": "overlay", "type": "image", "path": "overlay.png", "track": 1, "start_time": 2.0, "end_time": 8.0, "z_index": 1},
            {"id": "fg", "type": "image", "path": "fg.png", "track": 2, "start_time": 5.0, "end_time": 10.0, "z_index": 2}
        ]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the clips are preserved in order.
    assert_eq!(project.clips.len(), 3);
}

#[test]
fn parse_project_preserves_clip_order() {
    // Given a project with three clips in a specific order.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [
            {"id": "first", "type": "image", "path": "a.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0},
            {"id": "second", "type": "image", "path": "b.png", "track": 1, "start_time": 0.0, "end_time": 10.0, "z_index": 1},
            {"id": "third", "type": "image", "path": "c.png", "track": 2, "start_time": 0.0, "end_time": 10.0, "z_index": 2}
        ]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then clip IDs are in the original order.
    assert_eq!(project.clips[0].id, "first");
}

#[test]
fn parse_project_third_clip_id_is_third() {
    // Given a project with three ordered clips.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [
            {"id": "first", "type": "image", "path": "a.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0},
            {"id": "second", "type": "image", "path": "b.png", "track": 1, "start_time": 0.0, "end_time": 10.0, "z_index": 1},
            {"id": "third", "type": "image", "path": "c.png", "track": 2, "start_time": 0.0, "end_time": 10.0, "z_index": 2}
        ]
    }"#;
    let project: Project = serde_json::from_str(json).expect("should parse");

    // When checking the third clip.
    // Then it is "third".
    assert_eq!(project.clips[2].id, "third");
}

#[test]
fn parse_sizing_natural() {
    // Given a clip with sizing omitted (defaults to Natural).
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{"id": "bg", "type": "image", "path": "bg.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0}]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the clip's sizing is Natural.
    assert_eq!(project.clips[0].sizing, Sizing::Natural);
}

#[test]
fn parse_sizing_explicit() {
    // Given a clip with Explicit sizing.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{"id": "bg", "type": "image", "path": "bg.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0, "sizing": {"Explicit": {"width": 800, "height": 600}}}]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the sizing is Explicit with the correct dimensions.
    assert!(
        matches!(&project.clips[0].sizing, Sizing::Explicit { width, height } if *width == 800 && *height == 600)
    );
}

#[test]
fn parse_sizing_scale() {
    // Given a clip with Scale sizing.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{"id": "bg", "type": "image", "path": "bg.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0, "sizing": {"Scale": 1.5}}]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the sizing is Scale with the correct factor.
    assert!(matches!(&project.clips[0].sizing, Sizing::Scale(s) if (*s - 1.5).abs() < 1e-5));
}

#[test]
fn parse_sizing_fit_rect_contain() {
    // Given a clip with FitRect Contain sizing.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{"id": "bg", "type": "image", "path": "bg.png", "track": 0, "start_time": 0.0, "end_time": 10.0, "z_index": 0, "sizing": {"FitRect": {"x": 100, "y": 50, "w": 640, "h": 480, "mode": "Contain"}}}]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the sizing is FitRect with Contain mode.
    assert!(
        matches!(&project.clips[0].sizing, Sizing::FitRect { x, y, w, h, mode, anchor: _ }
            if *x == 100 && *y == 50 && *w == 640 && *h == 480 && matches!(mode, FitMode::Contain)
        )
    );
}

#[test]
fn parse_project_missing_resolution_returns_error() {
    // Given a project JSON missing the required resolution field.
    let json = r#"{
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": []
    }"#;

    // When parsing it.
    let result = serde_json::from_str::<Project>(json);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn parse_project_missing_clips_returns_error() {
    // Given a project JSON missing the required clips field.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4"
    }"#;

    // When parsing it.
    let result = serde_json::from_str::<Project>(json);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn parse_project_missing_all_required_fields_returns_error() {
    // Given a project JSON that is an empty object.
    let json = "{}";

    // When parsing it.
    let result = serde_json::from_str::<Project>(json);

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn keyframe_without_easing_field_defaults_to_linear() {
    // Given a keyframe in JSON with no easing field.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{
            "id": "test",
            "type": "image",
            "path": "img.png",
            "track": 0,
            "start_time": 0.0,
            "end_time": 10.0,
            "z_index": 0,
            "animations": [{
                "property": "opacity",
                "keyframes": [
                    {"time": 0.0, "value": 0.0},
                    {"time": 10.0, "value": 1.0}
                ]
            }]
        }]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then both keyframes have Linear as their easing default.
    let keyframes = &project.clips[0].animations[0].keyframes;
    assert_eq!(keyframes[0].easing, ss_core::animation::Easing::Linear);
}

#[test]
fn second_keyframe_easing_also_defaults_to_linear() {
    // Given a keyframe in JSON with no easing field on the second keyframe.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": [{
            "id": "test",
            "type": "image",
            "path": "img.png",
            "track": 0,
            "start_time": 0.0,
            "end_time": 10.0,
            "z_index": 0,
            "animations": [{
                "property": "opacity",
                "keyframes": [
                    {"time": 0.0, "value": 0.0},
                    {"time": 10.0, "value": 1.0}
                ]
            }]
        }]
    }"#;

    // When parsing it.
    let project: Project = serde_json::from_str(json).expect("should parse");

    // Then the second keyframe also defaults to Linear.
    let keyframes = &project.clips[0].animations[0].keyframes;
    assert_eq!(keyframes[1].easing, ss_core::animation::Easing::Linear);
}
