//! Transport panel: play/pause/stop controls, time display, and volume slider.
//!
//! Horizontal bar at the bottom of the editor window.

use crate::editor_state::TransportState;

/// Manages the transport control panel.
pub struct TransportPanel {
    /// Current volume (0.0–1.0).
    volume: f32,
}

impl Default for TransportPanel {
    fn default() -> Self {
        Self { volume: 0.8 }
    }
}

impl TransportPanel {
    /// Create a new transport panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current volume setting.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Show the transport panel.
    ///
    /// Returns user actions via [`TransportAction`].
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        playback_state: TransportState,
        current_time: f64,
        duration: f64,
    ) -> Vec<TransportAction> {
        let mut actions = Vec::new();

        ui.horizontal(|ui| {
            // Play/Pause toggle button.
            let label = match playback_state {
                TransportState::Playing => "⏸ Pause",
                TransportState::Paused => "▶ Play",
            };
            if ui.button(label).clicked() {
                actions.push(TransportAction::TogglePlayback);
            }

            // Stop button.
            if ui.button("⏹ Stop").clicked() {
                actions.push(TransportAction::Stop);
            }

            ui.separator();

            // Time display.
            let time_str = format!(
                "{:02}:{:05.2} / {:02}:{:05.2}",
                (current_time / 60.0) as u32,
                current_time % 60.0,
                (duration / 60.0) as u32,
                duration % 60.0,
            );
            ui.label(egui::RichText::new(time_str).monospace().size(13.0));

            ui.separator();

            // Volume control.
            ui.label("🔊");
            let old_vol = self.volume;
            ui.add(egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false));
            if (self.volume - old_vol).abs() > 0.001 {
                actions.push(TransportAction::SetVolume(self.volume));
            }
        });

        actions
    }
}

/// Actions the user can trigger from the transport panel.
#[derive(Debug, Clone, PartialEq)]
pub enum TransportAction {
    /// Toggle between play and pause.
    TogglePlayback,
    /// Stop playback and reset to beginning.
    Stop,
    /// Set the audio volume.
    SetVolume(f32),
}
