use ss_editor::AppConfig;

#[test]
fn default_config_has_half_res_divisor() {
    let config = AppConfig::default();
    assert_eq!(config.preview_divisor, 2);
}

#[test]
fn default_config_has_30fps() {
    let config = AppConfig::default();
    assert_eq!(config.preview_fps, 30);
}

#[test]
fn preview_resolution_halves_project_res() {
    let config = AppConfig::default();
    let res = config.preview_resolution([1920, 1080]);
    assert_eq!(res, (960, 540));
}

#[test]
fn preview_resolution_full_res() {
    let mut config = AppConfig::default();
    config.preview_divisor = 1;
    let res = config.preview_resolution([1920, 1080]);
    assert_eq!(res, (1920, 1080));
}

#[test]
fn preview_resolution_quarter_res() {
    let mut config = AppConfig::default();
    config.preview_divisor = 4;
    let res = config.preview_resolution([1920, 1080]);
    assert_eq!(res, (480, 270));
}

#[test]
fn save_and_load_roundtrip() {
    // Given a temp dir for the config file.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.json");

    let mut config = AppConfig::default();
    config.preview_divisor = 4;
    config.preview_fps = 60;
    config.window_size = [1920, 1080];
    config.last_project = Some("/foo/bar.json".into());

    // When saving and reloading.
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&path, &json).unwrap();
    let loaded: AppConfig = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    // Then the values match.
    assert_eq!(loaded.preview_divisor, 4);
    assert_eq!(loaded.preview_fps, 60);
    assert_eq!(loaded.window_size, [1920, 1080]);
    assert_eq!(loaded.last_project, Some("/foo/bar.json".into()));
}

#[test]
fn default_window_size() {
    let config = AppConfig::default();
    assert_eq!(config.window_size, [1280, 720]);
}

#[test]
fn default_last_project_is_none() {
    let config = AppConfig::default();
    assert!(config.last_project.is_none());
}
