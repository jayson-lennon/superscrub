//! Timeline panel: custom-painted tracks with clip blocks and a playhead.
//!
//! The timeline shows one row per track, with clip blocks as colored rectangles
//! positioned according to their start/end times. A vertical red playhead line
//! shows the current position. Click-to-seek is supported.

use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};
use ss_core::clip::ClipDef;
use ss_core::project::Project;

/// Manages the timeline panel display.
pub struct TimelinePanel {
    /// Track colors (cycled for visual distinction).
    track_colors: Vec<Color32>,
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            track_colors: vec![
                Color32::from_rgb(70, 130, 180),  // Steel blue
                Color32::from_rgb(180, 100, 70),  // Warm brown
                Color32::from_rgb(100, 170, 100), // Muted green
                Color32::from_rgb(170, 130, 190), // Soft purple
                Color32::from_rgb(190, 170, 100), // Gold
                Color32::from_rgb(130, 180, 170), // Teal
            ],
        }
    }
}

impl TimelinePanel {
    /// Create a new timeline panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the timeline panel.
    ///
    /// Returns the clicked time position if the user clicked on the timeline area.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        project: Option<&Project>,
        current_time: f64,
        duration: f64,
    ) -> Option<f64> {
        let mut clicked_time = None;

        ui.vertical(|ui| {
            ui.label(RichText::new("Timeline").size(14.0).strong());
            ui.add_space(2.0);

            let Some(project) = project else {
                ui.label("No project loaded");
                return;
            };

            if duration <= 0.0 {
                ui.label("Empty project");
                return;
            }

            let available_width = ui.available_width();
            let track_height = 24.0_f32;
            let gap = 2.0;
            let time_bar_height = 16.0;

            // Compute max track index.
            let max_track = project.clips.iter().map(|c| c.track).max().unwrap_or(0);
            let num_tracks = (max_track + 1) as usize;
            let total_height = time_bar_height + num_tracks as f32 * (track_height + gap);

            let (response, painter) =
                ui.allocate_painter(Vec2::new(available_width, total_height), Sense::click());

            let origin = response.rect.left_top();

            // Draw time ruler.
            self.draw_time_ruler(&painter, origin, available_width, duration, time_bar_height);

            // Draw track lanes with clip blocks.
            for clip in &project.clips {
                let track_y = origin.y + time_bar_height + clip.track as f32 * (track_height + gap);
                self.draw_clip_block(
                    &painter,
                    clip,
                    origin.x,
                    track_y,
                    available_width,
                    track_height,
                    duration,
                );
            }

            // Draw playhead.
            if duration > 0.0 {
                let fraction = (current_time / duration) as f32;
                let playhead_x = origin.x + fraction * available_width;
                let playhead_top = origin.y;
                let playhead_bottom = origin.y + total_height;
                painter.line_segment(
                    [
                        Pos2::new(playhead_x, playhead_top),
                        Pos2::new(playhead_x, playhead_bottom),
                    ],
                    egui::Stroke::new(2.0, Color32::RED),
                );
            }

            // Handle click-to-seek.
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                let fraction = ((pos.x - origin.x) / available_width).clamp(0.0, 1.0);
                clicked_time = Some(fraction as f64 * duration);
            }
        });

        clicked_time
    }

    /// Draw tick marks on the time ruler.
    fn draw_time_ruler(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        width: f32,
        duration: f64,
        height: f32,
    ) {
        let rect = Rect::from_min_size(origin, Vec2::new(width, height));
        painter.rect_filled(rect, 0.0, Color32::from_gray(40));

        // Draw ticks at reasonable intervals.
        let tick_interval = self.tick_interval(duration);
        let num_ticks = (duration / tick_interval).ceil() as usize;

        for i in 0..=num_ticks {
            let time = i as f64 * tick_interval;
            if time > duration {
                break;
            }
            let x = origin.x + (time / duration) as f32 * width;
            painter.line_segment(
                [Pos2::new(x, origin.y), Pos2::new(x, origin.y + height)],
                egui::Stroke::new(1.0, Color32::from_gray(80)),
            );

            let label = format!("{time:.0}s");
            painter.text(
                Pos2::new(x + 2.0, origin.y + 1.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_gray(160),
            );
        }
    }

    /// Choose a tick interval based on the duration.
    fn tick_interval(&self, duration: f64) -> f64 {
        if duration <= 10.0 {
            1.0
        } else if duration <= 60.0 {
            5.0
        } else if duration <= 300.0 {
            10.0
        } else {
            30.0
        }
    }

    /// Draw a single clip block as a colored rectangle.
    #[allow(clippy::too_many_arguments)]
    fn draw_clip_block(
        &self,
        painter: &egui::Painter,
        clip: &ClipDef,
        origin_x: f32,
        track_y: f32,
        total_width: f32,
        track_height: f32,
        duration: f64,
    ) {
        let start_frac = (clip.start_time / duration) as f32;
        let end_frac = (clip.end_time / duration) as f32;
        let x = origin_x + start_frac * total_width;
        let w = (end_frac - start_frac) * total_width;

        let color = self.track_colors[clip.track as usize % self.track_colors.len()];
        let rect = Rect::from_min_size(Pos2::new(x, track_y), Vec2::new(w.max(2.0), track_height));
        painter.rect_filled(rect, 3.0, color);

        // Clip ID label (truncate if too wide).
        if w > 20.0 {
            painter.text(
                rect.left_top() + Vec2::new(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &clip.id,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}
