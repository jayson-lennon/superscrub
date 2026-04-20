//! Viewport panel: displays the current preview frame as an egui texture.
//!
//! Shows a progress bar while the preview cache is building,
//! or a placeholder if no project is loaded.

use egui::{ColorImage, TextureHandle, TextureOptions};
use image::RgbaImage;
use ss_preview::RenderProgress;

/// Manages the viewport panel display.
#[derive(Default)]
pub struct ViewportPanel {
    /// Cached texture handle (recreated when the frame dimensions change).
    texture: Option<TextureHandle>,
}

impl ViewportPanel {
    /// Create a new viewport panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the viewport panel.
    ///
    /// - If a frame is available, display it scaled to fit the panel.
    /// - If rendering is in progress, show a progress bar.
    /// - If no project is loaded, show a placeholder message.
    pub fn show(&mut self, ui: &mut egui::Ui, frame: Option<&RgbaImage>, progress: RenderProgress) {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Viewport").size(14.0).strong());

            ui.add_space(4.0);

            let available = ui.available_size();

            if let Some(image) = frame {
                self.display_frame(ui, image, available);
            } else if progress.total > 0 && !progress.is_complete() {
                // Show progress bar while rendering.
                ui.add_space(available.y / 3.0);
                let label = format!("Rendering preview... {:.0}%", progress.fraction() * 100.0);
                ui.label(&label);
                ui.add(egui::ProgressBar::new(progress.fraction() as f32).show_percentage());
            } else if progress.total > 0 && progress.is_complete() {
                ui.add_space(available.y / 3.0);
                ui.label("Frame not available at this time");
            } else {
                ui.add_space(available.y / 3.0);
                ui.label("No project loaded");
            }
        });
    }

    /// Display a frame image scaled to fit the available space.
    fn display_frame(&mut self, ui: &mut egui::Ui, image: &RgbaImage, available: egui::Vec2) {
        let img_w = image.width() as f32;
        let img_h = image.height() as f32;

        // Scale to fit, maintaining aspect ratio.
        let scale = (available.x / img_w).min(available.y / img_h).min(1.0);
        let display_w = img_w * scale;
        let display_h = img_h * scale;

        // Upload to texture (or recreate if size changed).
        let color_image = rgba_to_color_image(image);
        let id = "viewport_frame";
        let needs_new = self.texture.as_ref().is_none_or(|t| {
            t.size()[0] != image.width() as usize || t.size()[1] != image.height() as usize
        });

        let texture = if needs_new {
            let tex = ui
                .ctx()
                .load_texture(id, color_image, TextureOptions::LINEAR);
            self.texture = Some(tex);
            self.texture.as_ref().unwrap()
        } else {
            let tex = self.texture.as_mut().unwrap();
            tex.set(color_image, TextureOptions::LINEAR);
            tex
        };

        ui.image(egui::load::SizedTexture::new(
            texture.id(),
            egui::vec2(display_w, display_h),
        ));
    }
}

/// Convert an `RgbaImage` to an egui `ColorImage`.
fn rgba_to_color_image(image: &RgbaImage) -> ColorImage {
    let size = [image.width() as usize, image.height() as usize];
    let pixels: Vec<egui::Color32> = image
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p.0[0], p.0[1], p.0[2], p.0[3]))
        .collect();
    ColorImage { size, pixels }
}
