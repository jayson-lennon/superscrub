//! Settings panel: preview resolution, preview fps configuration.
//!
//! A collapsible side panel for adjusting preview quality settings.

use crate::app_config::AppConfig;

/// Manages the settings panel.
pub struct SettingsPanel {
    /// Staging value for preview divisor (before "Apply" is clicked).
    preview_divisor: u32,
    /// Staging value for preview fps (before "Apply" is clicked).
    preview_fps: u32,
}

impl SettingsPanel {
    /// Create a new settings panel initialized from the current config.
    pub fn new(config: &AppConfig) -> Self {
        Self {
            preview_divisor: config.preview_divisor,
            preview_fps: config.preview_fps,
        }
    }

    /// Show the settings panel.
    ///
    /// Returns `Some(AppConfigChange)` if the user clicked "Apply" and settings changed.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<AppConfigChange> {
        let mut result = None;

        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Settings").size(14.0).strong());
            ui.add_space(4.0);

            ui.label("Preview Resolution:");
            ui.horizontal(|ui| {
                ui.label("÷");
                ui.add(egui::DragValue::new(&mut self.preview_divisor).range(1..=8));
                ui.label("(1 = full, 2 = half, 4 = quarter)");
            });

            ui.add_space(4.0);

            ui.label("Preview FPS:");
            ui.add(egui::DragValue::new(&mut self.preview_fps).range(1..=120));

            ui.add_space(8.0);

            if ui.button("Apply").clicked() {
                result = Some(AppConfigChange {
                    preview_divisor: self.preview_divisor,
                    preview_fps: self.preview_fps,
                });
            }
        });

        result
    }
}

/// A change to app config from the settings panel.
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfigChange {
    /// New preview divisor.
    pub preview_divisor: u32,
    /// New preview fps.
    pub preview_fps: u32,
}
