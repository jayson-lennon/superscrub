//! Binary entry point for the ss-editor application.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use ss_audio::{AudioEngineService, RodioAudioEngine};
use ss_compositor::{CompositorRenderer, FilesystemImageProvider, FrameRendererService};
use ss_editor::{AppConfig, EditorApp, Services};
use ss_preview::{BackgroundPreviewCache, PreviewCacheService};

/// SuperScrub — programmatic video editor.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Path to the project JSON file.
    project: PathBuf,
}

/// A no-op config watcher for now (actual `notify`-based implementation
/// deferred until needed; Phase 4 uses this stub).
struct StubConfigWatcher;

impl ss_core::traits::ConfigWatcher for StubConfigWatcher {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn watch(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), error_stack::Report<ss_core::errors::ConfigWatchError>> {
        Ok(())
    }

    fn has_changed(&self) -> bool {
        false
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let config = AppConfig::load().unwrap_or_default();

    let project_path = args.project;

    // Create services.
    let audio_engine = RodioAudioEngine::new().expect("failed to create audio engine");
    let audio = AudioEngineService::new(Arc::new(audio_engine));

    let image_provider = Arc::new(FilesystemImageProvider::new());
    let renderer = Arc::new(CompositorRenderer::new(image_provider));
    let preview = PreviewCacheService::new(Arc::new(BackgroundPreviewCache::new(renderer.clone())));
    let renderer_service = FrameRendererService::new(renderer);
    let watcher = Arc::new(StubConfigWatcher);

    let services = Services {
        audio,
        preview,
        renderer: renderer_service,
        watcher,
    };

    let app = EditorApp::new(services, config.clone(), project_path).unwrap_or_else(|e| {
        eprintln!("error: {e:?}");
        std::process::exit(1);
    });

    let window_size = config.window_size;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([window_size[0] as f32, window_size[1] as f32])
            .with_title("SuperScrub"),
        ..Default::default()
    };

    eframe::run_native("SuperScrub", options, Box::new(|_cc| Ok(Box::new(app))))
}
