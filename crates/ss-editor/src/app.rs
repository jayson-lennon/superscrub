//! Main editor application implementing `eframe::App`.
//!
//! Wires together all panels and the playback controller.
//! This is the only file that directly uses egui for layout.

use std::path::PathBuf;

use eframe::egui;
use error_stack::ResultExt;

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
    /// Cloned egui context for triggering repaints from background tasks.
    egui_ctx: egui::Context,
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
        egui_ctx: egui::Context,
        services: Services,
        config: AppConfig,
        project_path: PathBuf,
    ) -> Result<Self, error_stack::Report<crate::app_config::AppConfigError>> {
        let project_json = std::fs::read_to_string(&project_path)
            .change_context(crate::app_config::AppConfigError)
            .attach("failed to read project file")?;
        let project: ss_core::project::Project = serde_json::from_str(&project_json)
            .change_context(crate::app_config::AppConfigError)
            .attach("failed to parse project JSON")?;

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
        let _ = controller.load_project_audio();

        // Start watching for file changes.
        let mut watcher = ProjectWatcher::new(services.watcher.clone());
        let _ = watcher.start_watching(&project_path);

        let settings = SettingsPanel::new(&config);

        Ok(Self {
            controller,
            services,
            watcher,
            config,
            viewport: ViewportPanel::new(),
            timeline: TimelinePanel::new(),
            transport: TransportPanel::new(),
            settings,
            egui_ctx,
        })
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            let _ = self.controller.load_project_audio();
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Layout: bottom transport, then timeline, then right settings, then central viewport.
        egui::Panel::bottom("transport").show_inside(ui, |ui| {
            let transport = self.controller.state().transport_state();
            let current_time = self.controller.state().current_time();
            let duration = self.controller.state().duration();

            let actions = self.transport.show(ui, transport, current_time, duration);
            for action in actions {
                match action {
                    TransportAction::TogglePlayback => self.controller.toggle_playback(),
                    TransportAction::Stop => self.controller.stop(),
                    TransportAction::SetVolume(v) => self.services.audio.set_volume(v),
                }
            }
        });

        egui::Panel::bottom("timeline")
            .resizable(true)
            .show_inside(ui, |ui| {
                let project = self.controller.state().project();
                let current_time = self.controller.state().current_time();
                let duration = self.controller.state().duration();

                if let Some(action) = self.timeline.show(ui, project, current_time, duration) {
                    let time = match action {
                        crate::timeline_panel::TimelineAction::Click(t)
                        | crate::timeline_panel::TimelineAction::Scrub(t) => t,
                    };
                    if matches!(action, crate::timeline_panel::TimelineAction::Scrub(_)) {
                        self.controller.pause();
                    }
                    self.controller.seek_to(time);
                }
            });

        egui::Panel::right("settings")
            .resizable(true)
            .default_size(200.0)
            .show_inside(ui, |ui| {
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

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let frame = self.controller.current_frame();
            let progress = self.controller.render_progress();
            self.viewport.show(ui, frame.as_ref(), progress);
        });
    }

    fn on_exit(&mut self) {
        let _ = self.config.save();
    }
}
