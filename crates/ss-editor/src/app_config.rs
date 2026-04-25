//! Persistent application configuration stored via the `dirs` crate.
//!
//! Settings are serialized as JSON in the user's config directory.
//! On first launch, defaults are used and the config file is created.

use std::path::PathBuf;

use error_stack::ResultExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Persistent application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Window size in pixels [width, height].
    #[serde(default = "default_window_size")]
    pub window_size: [u32; 2],
    /// Path to the last opened project file.
    #[serde(default)]
    pub last_project: Option<String>,
    /// Preview resolution divisor (e.g., 2 = half resolution).
    #[serde(default = "default_preview_divisor")]
    pub preview_divisor: u32,
    /// Preview frames per second.
    #[serde(default = "default_preview_fps")]
    pub preview_fps: u32,
}

fn default_window_size() -> [u32; 2] {
    [1280, 720]
}

fn default_preview_divisor() -> u32 {
    2
}

fn default_preview_fps() -> u32 {
    30
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_size: default_window_size(),
            last_project: None,
            preview_divisor: default_preview_divisor(),
            preview_fps: default_preview_fps(),
        }
    }
}

/// Failed to load or save application configuration.
#[derive(Debug, wherror::Error)]
#[error("app config error")]
pub struct AppConfigError;

impl AppConfig {
    /// Returns the path to the config file in the user's config directory.
    ///
    /// The path is `$XDG_CONFIG_HOME/superscrub/config.json` on Linux,
    /// or the equivalent on macOS/Windows.
    pub fn config_path() -> Result<PathBuf, std::io::Error> {
        let config_dir = dirs::config_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no config directory")
        })?;
        Ok(config_dir.join("superscrub").join("config.json"))
    }

    /// Load the config from disk, or return defaults if no file exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be parsed.
    pub fn load() -> Result<Self, error_stack::Report<AppConfigError>> {
        let path = Self::config_path()
            .change_context(AppConfigError)
            .attach("failed to determine config directory")?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .change_context(AppConfigError)
            .attach("failed to read config file")?;

        let config: Self = serde_json::from_str(&content)
            .change_context(AppConfigError)
            .attach("failed to parse config JSON")?;

        info!("app config loaded");
        Ok(config)
    }

    /// Save the config to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or the file
    /// cannot be written.
    pub fn save(&self) -> Result<(), error_stack::Report<AppConfigError>> {
        let path = Self::config_path()
            .change_context(AppConfigError)
            .attach("failed to determine config directory")?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .change_context(AppConfigError)
                .attach("failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(self)
            .change_context(AppConfigError)
            .attach("failed to serialize config")?;

        std::fs::write(&path, content)
            .change_context(AppConfigError)
            .attach("failed to write config file")?;

        debug!("app config saved");
        Ok(())
    }

    /// Compute the preview resolution for a given project resolution.
    ///
    /// Divides each dimension by `preview_divisor`, ensuring a minimum of 1.
    pub fn preview_resolution(&self, project_resolution: [u32; 2]) -> (u32, u32) {
        let w = (project_resolution[0] / self.preview_divisor).max(1);
        let h = (project_resolution[1] / self.preview_divisor).max(1);
        (w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[rstest::rstest]
    #[case::half_res(2, (960, 540))]
    #[case::full_res(1, (1920, 1080))]
    #[case::quarter_res(4, (480, 270))]
    fn preview_resolution_divides_project_dimensions(
        #[case] divisor: u32,
        #[case] expected: (u32, u32),
    ) {
        // Given a config with the given preview divisor and a 1920×1080 project.
        let config = AppConfig {
            preview_divisor: divisor,
            ..Default::default()
        };
        let project_res = [1920, 1080];

        // When computing the preview resolution.
        let res = config.preview_resolution(project_res);

        // Then each dimension is divided correctly.
        assert_eq!(res, expected);
    }

    #[test]
    fn config_survives_save_and_load_roundtrip() {
        // Given a temp dir for the config file.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");

        let config = AppConfig {
            preview_divisor: 4,
            preview_fps: 60,
            window_size: [1920, 1080],
            last_project: Some("/foo/bar.json".into()),
        };

        // When saving and reloading.
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &json).unwrap();
        let loaded: AppConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        // Then the values match.
        assert_eq!(loaded.preview_divisor, 4);
        assert_eq!(loaded.preview_fps, 60);
        assert_eq!(loaded.window_size, [1920, 1080]);
        assert_eq!(loaded.last_project, Some("/foo/bar.json".into()));
    }
}
