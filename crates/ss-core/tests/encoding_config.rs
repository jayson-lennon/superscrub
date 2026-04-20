use ss_core::project::{EncodingConfig, Project};

#[test]
fn default_crf_is_18() {
    let config = EncodingConfig::default();
    assert_eq!(config.crf, 18);
}

#[test]
fn default_preset_is_medium() {
    let config = EncodingConfig::default();
    assert_eq!(config.preset, "medium");
}

#[test]
fn default_pixel_format_is_yuv420p() {
    let config = EncodingConfig::default();
    assert_eq!(config.pixel_format, "yuv420p");
}

#[test]
fn project_without_encoding_gets_defaults() {
    // Given a project JSON without the encoding field.
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "clips": []
    }"#;

    let project: Project = serde_json::from_str(json).unwrap();

    // Then encoding defaults are applied.
    assert_eq!(project.encoding.crf, 18);
    assert_eq!(project.encoding.preset, "medium");
    assert_eq!(project.encoding.pixel_format, "yuv420p");
}

#[test]
fn project_with_explicit_encoding() {
    let json = r#"{
        "resolution": [1920, 1080],
        "fps": 60,
        "duration": 10.0,
        "output": "out.mp4",
        "encoding": { "crf": 23, "preset": "fast", "pixel_format": "yuv422p" },
        "clips": []
    }"#;

    let project: Project = serde_json::from_str(json).unwrap();

    assert_eq!(project.encoding.crf, 23);
    assert_eq!(project.encoding.preset, "fast");
    assert_eq!(project.encoding.pixel_format, "yuv422p");
}

#[test]
fn encoding_config_serde_roundtrip() {
    let config = EncodingConfig {
        crf: 28,
        preset: "slow".into(),
        pixel_format: "yuv444p".into(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let loaded: EncodingConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.crf, 28);
    assert_eq!(loaded.preset, "slow");
    assert_eq!(loaded.pixel_format, "yuv444p");
}
