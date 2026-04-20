//! Main editor application implementing `eframe::App`.
//!
//! Wires together all panels and the playback controller.
//! This is the only file that directly uses egui for layout.

use std::path::PathBuf;

use eframe::egui;

use crate::app_config::AppConfig;
use crate::playback_controller::PlaybackController;
use crate::project_watcher::ProjectWatcher;
use crate::services::Services;
use crate::settings_panel::SettingsPanel;
use crate::timeline_panel::TimelinePanel;
use crate::transport_panel::{TransportAction, TransportPanel};
use crate::viewport_panel::ViewportPanel;

/// The main editor application.
///
/// Implements `eframe::App` and manages the main loop:
/// detect file changes → advance playback → draw panels.
pub struct EditorApp {
    /// Playback controller (owns editor state, audio, cache).
    controller: PlaybackController,
    /// Services container.
    services: Services,
    /// Project file watcher.
    watcher: ProjectWatcher,
    /// Application config.
    config: AppConfig,
    /// Viewport panel.
    viewport: ViewportPanel,
    /// Timeline panel.
    timeline: TimelinePanel,
    /// Transport panel.
    transport: TransportPanel,
    /// Settings panel.
    settings: SettingsPanel,
}

impl EditorApp {
    /// Create a new editor application.
    ///
    /// Loads the project, starts watching for changes, starts the preview render.
    ///
    /// # Errors
    ///
    /// Returns an error if the project file cannot be read or parsed.
    pub fn new(
        services: Services,
        config: AppConfig,
        project_path: PathBuf,
    ) -> Result<Self, error_stack::Report<crate::app_config::AppConfigError>> {
        let project_json = std::fs::read_to_string(&project_path).map_err(|e| {
            error_stack::Report::new(crate::app_config::AppConfigError).attach(e.to_string())
        })?;
        let project: ss_core::project::Project =
            serde_json::from_str(&project_json).map_err(|e| {
                error_stack::Report::new(crate::app_config::AppConfigError).attach(e.to_string())
            })?;

        let preview_fps = config.preview_fps;
        let preview_res = config.preview_resolution(project.resolution);

        // Build editor state and load project.
        let mut state = crate::editor_state::EditorState::new();
        state.load_project(project, project_path.clone());

        // Build playback controller.
        let mut controller = PlaybackController::new(
            state,
            services.audio.clone(),
            services.preview.clone(),
            preview_fps,
        );

        // Start initial preview render.
        let _ = controller.start_render(preview_res);

        // Load audio if the project has an audio config.
        if let Some(audio_config) = controller.state().project().and_then(|p| p.audio.as_ref())
            && let Ok(audio_path) =
                ss_core::path_resolve::resolve_path(&project_path, &audio_config.path)
        {
            let _ = services.audio.load(&audio_path);
        }

        // Start watching for file changes.
        let mut watcher = ProjectWatcher::new(services.watcher.clone());
        let _ = watcher.start_watching(&project_path);

        Ok(Self {
            controller,
            services,
            watcher,
            config,
            viewport: ViewportPanel::new(),
            timeline: TimelinePanel::new(),
            transport: TransportPanel::new(),
            settings: SettingsPanel::new(&AppConfig::default()),
        })
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Poll for project file changes.
        if self.watcher.poll()
            && let Some(path) = self.controller.state().project_file().cloned()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(project) = serde_json::from_str::<ss_core::project::Project>(&content)
        {
            self.controller.state_mut().reload_project(project);
            let preview_res = self
                .config
                .preview_resolution(self.controller.state().resolution());
            let _ = self.controller.start_render(preview_res);
        }

        // 2. Advance playback if playing.
        if self.controller.state().is_playing() {
            let dt = ctx.input(|i| i.stable_dt) as f64;
            self.controller.advance(dt);
        }

        // Request repaint while playing to keep the viewport updating.
        if self.controller.state().is_playing() {
            ctx.request_repaint();
        }

        // 3. Layout: bottom transport, then timeline, then right settings, then central viewport.
        egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
            let state = self.controller.state().is_playing();
            let playback = if state {
                ss_audio::PlaybackState::Playing
            } else {
                ss_audio::PlaybackState::Paused
            };
            let current_time = self.controller.state().current_time();
            let duration = self.controller.state().duration();

            let actions = self.transport.show(ui, playback, current_time, duration);
            for action in actions {
                match action {
                    TransportAction::TogglePlayback => self.controller.toggle_playback(),
                    TransportAction::Stop => self.controller.stop(),
                    TransportAction::SetVolume(v) => self.services.audio.set_volume(v),
                }
            }
        });

        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .show(ctx, |ui| {
                let project = self.controller.state().project();
                let current_time = self.controller.state().current_time();
                let duration = self.controller.state().duration();

                if let Some(clicked_time) = self.timeline.show(ui, project, current_time, duration)
                {
                    self.controller.seek_to(clicked_time);
                }
            });

        egui::SidePanel::right("settings")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                if let Some(change) = self.settings.show(ui) {
                    self.config.preview_divisor = change.preview_divisor;
                    self.config.preview_fps = change.preview_fps;
                    self.controller.set_preview_fps(change.preview_fps);
                    let preview_res = self
                        .config
                        .preview_resolution(self.controller.state().resolution());
                    let _ = self.controller.start_render(preview_res);
                    let _ = self.config.save();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let frame = self.controller.current_frame();
            let progress = self.controller.render_progress();
            self.viewport.show(ui, frame.as_ref(), progress);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.config.save();
    }
}
