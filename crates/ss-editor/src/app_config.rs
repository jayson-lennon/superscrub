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
    fn preview_resolution_with_divisor_one_returns_full_resolution() {
        let mut config = AppConfig::default();
        config.preview_divisor = 1;
        let res = config.preview_resolution([1920, 1080]);
        assert_eq!(res, (1920, 1080));
    }

    #[test]
    fn preview_resolution_with_divisor_four_returns_quarter_resolution() {
        let mut config = AppConfig::default();
        config.preview_divisor = 4;
        let res = config.preview_resolution([1920, 1080]);
        assert_eq!(res, (480, 270));
    }

    #[test]
    fn config_survives_save_and_load_roundtrip() {
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
        let loaded: AppConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

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
}
