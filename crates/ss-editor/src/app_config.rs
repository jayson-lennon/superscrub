//! Persistent application configuration stored via the `dirs` crate.
//!
//! Settings are serialized as JSON in the user's config directory.
//! On first launch, defaults are used and the config file is created.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))?;

        serde_json::from_str(&content)
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))
    }

    /// Save the config to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or the file
    /// cannot be written.
    pub fn save(&self) -> Result<(), error_stack::Report<AppConfigError>> {
        let path = Self::config_path()
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))?;

        std::fs::write(&path, content)
            .map_err(|e| error_stack::Report::new(AppConfigError).attach(e.to_string()))
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
