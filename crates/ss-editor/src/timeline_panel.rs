//! Timeline panel: custom-painted tracks with clip blocks and a playhead.
//!
//! The timeline shows one row per track, with clip blocks as colored rectangles
//! positioned according to their start/end times. A vertical red playhead line
//! shows the current position. Click-to-seek and click+drag scrub are supported.
//!
//! Video tracks occupy the top section. A separator with a "♫ Audio" label
//! divides video from audio tracks below. Audio clip blocks use a distinct
//! color palette and display volume percentage when below 100%.

use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};
use ss_core::item::ItemDef;
use ss_core::project::{AudioClipDef, Project};

/// Computed layout for the timeline (testable without egui).
///
/// All positions are relative to the timeline widget's origin.
/// The `show()` method uses this to drive drawing; tests verify the math.
#[derive(Debug, PartialEq)]
struct TimelineLayout {
    /// Total height of the timeline widget.
    total_height: f32,
    /// Y position where the cache status bar starts (relative to origin).
    cache_bar_y: f32,
    /// Y position where video tracks start (relative to origin).
    video_origin_y: f32,
    /// Number of video track rows.
    num_video_tracks: usize,
    /// Y position where the separator is drawn (relative to origin), if any.
    separator_y: Option<f32>,
    /// Y position where audio tracks start (relative to origin), if any.
    audio_origin_y: Option<f32>,
    /// Number of audio track rows.
    num_audio_tracks: usize,
}

/// Layout constants used by the timeline.
const TRACK_HEIGHT: f32 = 24.0;
const GAP: f32 = 2.0;
const TIME_BAR_HEIGHT: f32 = 16.0;
/// Height of the cache status bar (half of `TIME_BAR_HEIGHT`).
const CACHE_BAR_HEIGHT: f32 = 4.0;
const DEFAULT_SEPARATOR_HEIGHT: f32 = 4.0;

impl TimelineLayout {
    /// Compute the timeline layout from project data and dimensions.
    ///
    /// Returns a `TimelineLayout` with all positions needed for rendering.
    fn compute(
        num_video_tracks: usize,
        num_audio_tracks: usize,
        time_bar_height: f32,
        cache_bar_height: f32,
        track_height: f32,
        gap: f32,
        separator_height: f32,
    ) -> Self {
        let cache_bar_y = 0.0;
        let video_origin_y = cache_bar_height + time_bar_height;
        let video_section_height = num_video_tracks as f32 * (track_height + gap);
        let has_audio = num_audio_tracks > 0;

        let (separator_y, audio_origin_y, separator_space) = if has_audio {
            let sep_y = video_origin_y + video_section_height;
            let sep_space = separator_height + gap;
            let audio_y = sep_y + sep_space;
            (Some(sep_y), Some(audio_y), sep_space)
        } else {
            (None, None, 0.0)
        };

        let audio_section_height = num_audio_tracks as f32 * (track_height + gap);
        let total_height = cache_bar_height
            + time_bar_height
            + video_section_height
            + separator_space
            + audio_section_height;

        Self {
            total_height,
            cache_bar_y,
            video_origin_y,
            num_video_tracks,
            separator_y,
            audio_origin_y,
            num_audio_tracks,
        }
    }
}

/// Action returned by the timeline panel each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimelineAction {
    /// User clicked (press + release without significant movement). Seek to time.
    Click(f64),
    /// User is holding the pointer down (scrub in progress). Seek to time every frame.
    Scrub(f64),
}

/// Manages the timeline panel display.
pub struct TimelinePanel {
    /// Video track colors (cycled for visual distinction).
    video_colors: Vec<Color32>,
    /// Audio track colors (distinct from video colors).
    audio_colors: Vec<Color32>,
    /// Separator height (thin line between video and audio sections).
    separator_height: f32,
    /// Whether a scrub (click+drag) operation is in progress.
    is_scrubbing: bool,
}

impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            video_colors: vec![
                Color32::from_rgb(70, 130, 180),  // Steel blue
                Color32::from_rgb(180, 100, 70),  // Warm brown
                Color32::from_rgb(100, 170, 100), // Muted green
                Color32::from_rgb(170, 130, 190), // Soft purple
                Color32::from_rgb(190, 170, 100), // Gold
                Color32::from_rgb(130, 180, 170), // Teal
            ],
            audio_colors: vec![
                Color32::from_rgb(0, 140, 140),   // Dark teal
                Color32::from_rgb(200, 120, 50),  // Warm orange
                Color32::from_rgb(130, 100, 170), // Muted violet
                Color32::from_rgb(140, 150, 60),  // Olive
                Color32::from_rgb(200, 100, 100), // Coral
                Color32::from_rgb(110, 130, 150), // Slate
            ],
            separator_height: DEFAULT_SEPARATOR_HEIGHT,
            is_scrubbing: false,
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
    /// Returns a [`TimelineAction`] if the user interacted with the timeline:
    /// [`TimelineAction::Scrub`] while the pointer is held down, or
    /// [`TimelineAction::Click`] on release without significant movement.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        project: Option<&Project>,
        current_time: f64,
        duration: f64,
        cached_frames: &[bool],
        preview_fps: u32,
    ) -> Option<TimelineAction> {
        let mut action = None;

        // preview_fps is available for future use (e.g. mapping frames to time).
        let _ = preview_fps;

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

            // Compute track counts.
            let max_video_track = project
                .items
                .iter()
                .map(|item| item.track)
                .max()
                .unwrap_or(0);
            let num_video_tracks = (max_video_track + 1) as usize;

            let max_audio_track = project
                .audio_clips
                .iter()
                .map(|c| c.track)
                .max()
                .unwrap_or(0);
            let num_audio_tracks = if project.audio_clips.is_empty() {
                0
            } else {
                (max_audio_track + 1) as usize
            };

            // Compute layout.
            let layout = TimelineLayout::compute(
                num_video_tracks,
                num_audio_tracks,
                TIME_BAR_HEIGHT,
                CACHE_BAR_HEIGHT,
                TRACK_HEIGHT,
                GAP,
                self.separator_height,
            );

            let (response, painter) = ui.allocate_painter(
                Vec2::new(available_width, layout.total_height),
                Sense::click(),
            );

            let origin = response.rect.left_top();

            // Draw cache status bar.
            self.draw_cache_bar(
                &painter,
                Pos2::new(origin.x, origin.y + layout.cache_bar_y),
                available_width,
                cached_frames,
                CACHE_BAR_HEIGHT,
            );

            // Draw time ruler (shifted down by cache bar height).
            self.draw_time_ruler(
                &painter,
                Pos2::new(origin.x, origin.y + CACHE_BAR_HEIGHT),
                available_width,
                duration,
                TIME_BAR_HEIGHT,
            );

            // Draw video track lanes with item blocks.
            for item in &project.items {
                let track_y =
                    origin.y + layout.video_origin_y + item.track as f32 * (TRACK_HEIGHT + GAP);
                self.draw_item_block(
                    &painter,
                    item,
                    origin.x,
                    track_y,
                    available_width,
                    TRACK_HEIGHT,
                    duration,
                );
            }

            // Draw separator and audio section.
            if let (Some(sep_y), Some(audio_y)) = (layout.separator_y, layout.audio_origin_y) {
                let separator_rect = Rect::from_min_size(
                    Pos2::new(origin.x, origin.y + sep_y),
                    Vec2::new(available_width, self.separator_height),
                );
                painter.rect_filled(separator_rect, 0.0, Color32::from_gray(60));

                // Label on the separator.
                painter.text(
                    separator_rect.left_top() + Vec2::new(4.0, -1.0),
                    egui::Align2::LEFT_CENTER,
                    "♫ Audio",
                    egui::FontId::proportional(10.0),
                    Color32::from_gray(180),
                );

                // Draw audio track lanes.
                for clip in &project.audio_clips {
                    let track_y = origin.y + audio_y + clip.track as f32 * (TRACK_HEIGHT + GAP);
                    self.draw_audio_clip_block(
                        &painter,
                        clip,
                        origin.x,
                        track_y,
                        available_width,
                        TRACK_HEIGHT,
                        duration,
                    );
                }
            }

            // Draw playhead.
            if duration > 0.0 {
                let fraction = (current_time / duration) as f32;
                let playhead_x = origin.x + fraction * available_width;
                let playhead_top = origin.y;
                let playhead_bottom = origin.y + layout.total_height;
                painter.line_segment(
                    [
                        Pos2::new(playhead_x, playhead_top),
                        Pos2::new(playhead_x, playhead_bottom),
                    ],
                    egui::Stroke::new(2.0, Color32::RED),
                );
            }

            // Handle scrub (pointer held down) and click-to-seek.
            let primary_down = response.ctx.input(|i| i.pointer.primary_down());
            if self.is_scrubbing {
                if primary_down {
                    // Continue scrubbing — use pointer position directly.
                    if let Some(pos) = response.ctx.input(|i| i.pointer.interact_pos()) {
                        let fraction = ((pos.x - origin.x) / available_width).clamp(0.0, 1.0);
                        let time = fraction as f64 * duration;
                        action = Some(TimelineAction::Scrub(time));
                    }
                } else {
                    // Button released — scrub ended. Check if it was a click
                    // (press + release without significant movement).
                    self.is_scrubbing = false;
                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let fraction = ((pos.x - origin.x) / available_width).clamp(0.0, 1.0);
                            let time = fraction as f64 * duration;
                            action = Some(TimelineAction::Click(time));
                        }
                    }
                }
            } else if response.is_pointer_button_down_on() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let fraction = ((pos.x - origin.x) / available_width).clamp(0.0, 1.0);
                    let time = fraction as f64 * duration;
                    self.is_scrubbing = true;
                    action = Some(TimelineAction::Scrub(time));
                }
            } else if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let fraction = ((pos.x - origin.x) / available_width).clamp(0.0, 1.0);
                    let time = fraction as f64 * duration;
                    action = Some(TimelineAction::Click(time));
                }
            }

            // Set cursor icon.
            if self.is_scrubbing {
                response.ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if response.hovered() {
                response.ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }
        });

        action
    }

    /// Draw the cache status bar showing which frames are cached.
    ///
    /// Each frame maps to a column of at least 1px wide. Cached frames are
    /// painted with an extra pixel of width to prevent sub-pixel gaps between
    /// adjacent columns. Uncached frames are painted dark gray.
    fn draw_cache_bar(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        width: f32,
        cached_frames: &[bool],
        height: f32,
    ) {
        // Fill the entire bar as uncached.
        let uncached_color = Color32::from_gray(40);
        let bar_rect = Rect::from_min_size(origin, Vec2::new(width, height));
        painter.rect_filled(bar_rect, 0.0, uncached_color);

        if cached_frames.is_empty() {
            return;
        }

        let total_frames = cached_frames.len();
        let column_width = (width / total_frames as f32).max(1.0);
        let cached_color = Color32::GREEN;

        // Paint cached frames on top.
        for (index, &is_cached) in cached_frames.iter().enumerate() {
            if is_cached {
                let x = origin.x + index as f32 * column_width;
                let rect = Rect::from_min_size(
                    Pos2::new(x, origin.y),
                    Vec2::new(column_width + 1.0, height),
                );
                painter.rect_filled(rect, 0.0, cached_color);
            }
        }
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

    /// Draw a single video clip block as a colored rectangle.
    #[allow(clippy::too_many_arguments)]
    fn draw_item_block(
        &self,
        painter: &egui::Painter,
        item: &ItemDef,
        origin_x: f32,
        track_y: f32,
        total_width: f32,
        track_height: f32,
        duration: f64,
    ) {
        let start_frac = (item.start_time / duration) as f32;
        let end_frac = (item.end_time / duration) as f32;
        let x = origin_x + start_frac * total_width;
        let w = (end_frac - start_frac) * total_width;

        let color = self.video_colors[item.track as usize % self.video_colors.len()];
        let rect = Rect::from_min_size(Pos2::new(x, track_y), Vec2::new(w.max(2.0), track_height));
        painter.rect_filled(rect, 3.0, color);

        // Clip ID label (truncate if too wide).
        if w > 20.0 {
            painter.text(
                rect.left_top() + Vec2::new(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &item.id,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }

    /// Draw a single audio clip block as a colored rectangle with volume indicator.
    ///
    /// Audio blocks use the audio color palette and apply volume-based opacity
    /// via `Color32::linear_multiply`. The label shows volume percentage when
    /// below 100%.
    #[allow(clippy::too_many_arguments)]
    fn draw_audio_clip_block(
        &self,
        painter: &egui::Painter,
        clip: &AudioClipDef,
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

        let base_color = self.audio_colors[clip.track as usize % self.audio_colors.len()];
        // Apply volume as opacity (volume=1.0 → fully opaque, volume=0.0 → very faint).
        // Minimum 30% opacity so the block is always visible.
        let alpha = 0.3 + 0.7 * clip.volume;
        let color = base_color.linear_multiply(alpha);

        let rect = Rect::from_min_size(Pos2::new(x, track_y), Vec2::new(w.max(2.0), track_height));
        painter.rect_filled(rect, 3.0, color);

        // Clip ID label with volume indicator.
        if w > 30.0 {
            let label = if (clip.volume - 1.0).abs() < f32::EPSILON {
                clip.id.clone()
            } else {
                format!("{} ({:.0}%)", clip.id, clip.volume * 100.0)
            };
            painter.text(
                rect.left_top() + Vec2::new(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        } else if w > 20.0 {
            // Narrow: just show the ID.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a project with the given video and audio clips.
    fn build_project(
        items: Vec<ItemDef>,
        audio_clips: Vec<AudioClipDef>,
    ) -> ss_core::project::Project {
        ss_core::project::Project {
            resolution: [1920, 1080],
            fps: 30,
            duration: std::time::Duration::from_secs_f64(10.0),
            output: "out.mp4".to_string(),
            background: [0x2c, 0x2e, 0x34, 0xff],
            audio_clips,
            encoding: ss_core::project::EncodingConfig::default(),
            items,
        }
    }

    /// Helper to create a minimal video clip on the given track.
    fn video_item(id: &str, track: u32) -> ItemDef {
        ItemDef {
            id: id.to_string(),
            content: ss_core::item::ItemContent::Image {
                path: format!("{id}.png"),
            },
            track,
            start_time: 0.0,
            end_time: 10.0,
            z_index: 0,
            sizing: ss_core::item::Sizing::default(),
            pivot: [0.5, 0.5],
            animations: vec![],
        }
    }

    /// Helper to create a minimal audio clip on the given track.
    fn audio_clip(id: &str, track: u32, volume: f32) -> AudioClipDef {
        AudioClipDef {
            id: id.to_string(),
            path: format!("{id}.mp3"),
            track,
            start_time: 0.0,
            end_time: 10.0,
            volume,
            source_offset: 0.0,
            trim_end: 0.0,
            animations: vec![],
        }
    }

    // ============================================================
    // TimelineLayout unit tests
    // ============================================================

    #[test]
    fn layout_with_no_audio_clips_has_no_separator() {
        // Given a project with video clips but no audio clips.
        let layout = TimelineLayout::compute(
            2,
            0,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then there is no separator or audio section.
        assert_eq!(layout.separator_y, None);
        assert_eq!(layout.audio_origin_y, None);
        assert_eq!(layout.num_audio_tracks, 0);
        assert_eq!(layout.num_video_tracks, 2);
    }

    #[test]
    fn layout_with_audio_clips_includes_audio_rows() {
        // Given a project with 2 audio tracks.
        let layout = TimelineLayout::compute(
            1,
            2,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then the layout includes audio rows.
        assert_eq!(layout.num_audio_tracks, 2);
        assert!(layout.separator_y.is_some());
        assert!(layout.audio_origin_y.is_some());
    }

    #[test]
    fn layout_audio_tracks_are_below_video_tracks() {
        // Given 1 video track and 2 audio tracks.
        let layout = TimelineLayout::compute(
            1,
            2,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // When checking positions.
        let video_bottom = layout.video_origin_y + 1.0_f32 * (TRACK_HEIGHT + GAP);
        let sep_y = layout.separator_y.unwrap();
        let audio_y = layout.audio_origin_y.unwrap();

        // Then separator is below video section.
        assert!(sep_y >= video_bottom - GAP); // separator starts at video section end
        // And audio section is below separator.
        assert!(audio_y > sep_y + DEFAULT_SEPARATOR_HEIGHT);
    }

    #[test]
    fn layout_with_no_clips_at_all() {
        // Given a project with no clips at all (max_track defaults to 0 → 1 track).
        let layout = TimelineLayout::compute(
            1,
            0,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then there is 1 video track row and no audio.
        assert_eq!(layout.num_video_tracks, 1);
        assert_eq!(layout.num_audio_tracks, 0);
        assert!(layout.separator_y.is_none());
    }

    #[test]
    fn layout_single_audio_clip_on_track_0() {
        // Given a single audio clip on track 0 (1 audio track).
        let layout = TimelineLayout::compute(
            1,
            1,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then positions are correct.
        assert_eq!(layout.num_audio_tracks, 1);
        let expected_audio_y = CACHE_BAR_HEIGHT
            + TIME_BAR_HEIGHT
            + 1.0_f32 * (TRACK_HEIGHT + GAP)
            + DEFAULT_SEPARATOR_HEIGHT
            + GAP;
        assert!((layout.audio_origin_y.unwrap() - expected_audio_y).abs() < f32::EPSILON);
    }

    // ============================================================
    // Integration tests (show() does not panic)
    // ============================================================

    #[rstest::rstest]
    #[case::with_audio_clips(
        vec![video_item("bg", 0)],
        vec![audio_clip("music", 0, 1.0), audio_clip("sfx", 1, 0.5)],
        2.5,
    )]
    #[case::only_audio_clips(
        vec![],
        vec![audio_clip("bg-music", 0, 0.8)],
        0.0,
    )]
    #[case::empty_audio_clips(
        vec![video_item("bg", 0)],
        vec![],
        5.0,
    )]
    fn show_does_not_panic(
        #[case] items: Vec<ItemDef>,
        #[case] audio_clips: Vec<AudioClipDef>,
        #[case] current_time: f64,
    ) {
        // Given a project with the specified items.
        let project = build_project(items, audio_clips);
        let mut panel = TimelinePanel::new();

        // When showing the timeline.
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                panel.show(ui, Some(&project), current_time, 10.0, &[], 30);
            });
        });

        // Then it does not panic.
    }

    // ============================================================
    // Cache bar layout tests
    // ============================================================

    #[test]
    fn layout_cache_bar_position_is_at_top() {
        // Given a layout with cache bar.
        let layout = TimelineLayout::compute(
            1,
            0,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then cache_bar_y is 0 (top of the widget).
        assert!((layout.cache_bar_y - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn layout_video_origin_accounts_for_cache_bar() {
        // Given a layout with cache bar.
        let layout = TimelineLayout::compute(
            1,
            0,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then video_origin_y includes cache bar + time bar.
        let expected = CACHE_BAR_HEIGHT + TIME_BAR_HEIGHT;
        assert!((layout.video_origin_y - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn layout_total_height_includes_cache_bar() {
        // Given a layout with cache bar.
        let layout_with = TimelineLayout::compute(
            1,
            0,
            TIME_BAR_HEIGHT,
            CACHE_BAR_HEIGHT,
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );
        let layout_without = TimelineLayout::compute(
            1,
            0,
            TIME_BAR_HEIGHT,
            0.0, // no cache bar
            TRACK_HEIGHT,
            GAP,
            DEFAULT_SEPARATOR_HEIGHT,
        );

        // Then the difference is exactly CACHE_BAR_HEIGHT.
        let diff = layout_with.total_height - layout_without.total_height;
        assert!((diff - CACHE_BAR_HEIGHT).abs() < f32::EPSILON);
    }

    #[test]
    fn show_with_cached_frames_does_not_panic() {
        // Given a project and some cached frame data.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let cached = vec![true, false, true, false, true];

        // When showing the timeline with sparse cached frames.
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                panel.show(ui, Some(&project), 2.5, 10.0, &cached, 30);
            });
        });

        // Then it does not panic.
    }

    #[test]
    fn show_with_empty_cached_frames_does_not_panic() {
        // Given a project with no cached frames.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();

        // When showing the timeline with empty cached frames.
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                panel.show(ui, Some(&project), 2.5, 10.0, &[], 30);
            });
        });

        // Then it does not panic.
    }

    #[test]
    fn show_with_all_cached_frames_does_not_panic() {
        // Given a project where all 300 frames are cached.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let cached = vec![true; 300];

        // When showing the timeline.
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                panel.show(ui, Some(&project), 2.5, 10.0, &cached, 30);
            });
        });

        // Then it does not panic.
    }

    // ============================================================
    // TimelineAction interaction tests
    // ============================================================

    /// Run one frame of the timeline panel and return the action.
    fn run_timeline_frame(
        ctx: &egui::Context,
        panel: &mut TimelinePanel,
        project: &Project,
        raw_input: egui::RawInput,
    ) -> Option<TimelineAction> {
        let mut action = None;
        let _ = ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                action = panel.show(ui, Some(project), 5.0, 10.0, &[], 30);
            });
        });
        action
    }

    /// Run a setup frame so egui learns the widget layout before we send
    /// pointer events. Without this, egui cannot match pointer events to
    /// widgets on the very first frame.
    fn setup_frame(ctx: &egui::Context, panel: &mut TimelinePanel, project: &Project) {
        let _ = run_timeline_frame(ctx, panel, project, egui::RawInput::default());
    }

    fn pointer_press_at(x: f32, y: f32) -> egui::RawInput {
        egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::Pos2::new(x, y)),
                egui::Event::PointerButton {
                    pos: egui::Pos2::new(x, y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            ..Default::default()
        }
    }

    fn pointer_release_at(x: f32, y: f32) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: egui::Pos2::new(x, y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
            ..Default::default()
        }
    }

    fn pointer_move_to(x: f32, y: f32) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::Pos2::new(x, y))],
            ..Default::default()
        }
    }

    #[test]
    fn show_returns_none_when_no_pointer_interaction() {
        // Given a project and timeline panel with a setup frame.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let ctx = egui::Context::default();
        setup_frame(&ctx, &mut panel, &project);

        // When showing the timeline with no pointer events.
        let action = run_timeline_frame(&ctx, &mut panel, &project, egui::RawInput::default());

        // Then no action is returned.
        assert_eq!(action, None);
    }

    #[test]
    fn show_returns_scrub_while_pointer_is_held_down() {
        // Given a project and timeline panel with a setup frame.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let ctx = egui::Context::default();
        setup_frame(&ctx, &mut panel, &project);

        // When pressing the pointer at x=250 in a ~10000px-wide panel (2.5% → time ~0.25).
        let raw_input = pointer_press_at(250.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_input);

        // Then a Scrub action is returned.
        assert!(matches!(action, Some(TimelineAction::Scrub(_))));
        if let Some(TimelineAction::Scrub(time)) = action {
            assert!(
                (0.0..=10.0).contains(&time),
                "time should be in [0, 10], got {time}"
            );
        }
    }

    #[test]
    fn show_returns_none_after_scrub_releases() {
        // Given a panel with a setup frame.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let ctx = egui::Context::default();
        setup_frame(&ctx, &mut panel, &project);

        // Frame 1: press to start scrub.
        let raw_press = pointer_press_at(250.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_press);
        assert!(matches!(action, Some(TimelineAction::Scrub(_))));

        // Frame 2: drag to a new position (still holding).
        let raw_drag = pointer_move_to(400.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_drag);
        assert!(matches!(action, Some(TimelineAction::Scrub(_))));

        // Frame 3: release after drag.
        let raw_release = pointer_release_at(400.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_release);

        // Then no action is returned (drag was a scrub, not a click).
        assert_eq!(action, None);
    }

    #[test]
    fn show_returns_click_on_press_and_release_without_move() {
        // Given a project and timeline panel with a setup frame.
        let project = build_project(vec![video_item("bg", 0)], vec![]);
        let mut panel = TimelinePanel::new();
        let ctx = egui::Context::default();
        setup_frame(&ctx, &mut panel, &project);

        // Frame 1: press.
        let raw_press = pointer_press_at(250.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_press);
        // During press, Scrub is returned.
        assert!(matches!(action, Some(TimelineAction::Scrub(_))));

        // Frame 2: release at same position (no drag → counts as click).
        let raw_release = pointer_release_at(250.0, 50.0);
        let action = run_timeline_frame(&ctx, &mut panel, &project, raw_release);

        // Then a Click action is returned.
        assert!(matches!(action, Some(TimelineAction::Click(_))));
        if let Some(TimelineAction::Click(time)) = action {
            assert!(
                (0.0..=10.0).contains(&time),
                "time should be in [0, 10], got {time}"
            );
        }
    }
}
