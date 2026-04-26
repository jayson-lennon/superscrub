//! Ken Burns effect builder — pan and zoom across images with crossfade transitions.
//!
//! Produces a stack of clips with pan/zoom animations and opacity fades that
//! follow a stack-based pop model: the topmost clip holds, fades out, then
//! the next clip does the same.
//!
//! # Usage
//!
//! ```ignore
//! use ss_effects::{KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams};
//!
//! let clips = KenBurnsBuilder::new(
//!     KenBurnsParams::builder()
//!         .duration(30.0)
//!         .resolution([1920, 1080])
//!         .build()
//! )
//! .add_segment(
//!     KenBurnsSegmentParams::builder()
//!         .image_path("img1.png")
//!         .direction(KenBurnsDirection::ZoomInLeft)
//!         .build()
//! )
//! .add_segment(
//!     KenBurnsSegmentParams::builder()
//!         .image_path("img2.png")
//!         .direction(KenBurnsDirection::ZoomOutCenter)
//!         .build()
//! )
//! .build();
//! ```

mod mapping;
mod timing;

use ss_core::Easing;
use ss_project_builder::{AnimBuilder, ClipBuilder, ClipParams, clip_builder::sizing};

/// Pan/zoom direction for a Ken Burns segment.
///
/// `FocalPoint` uses fractional [0..1] coordinates where `[0, 0]` is the
/// top-left corner and `[1, 1]` is the bottom-right corner.
#[derive(Debug, Clone, PartialEq)]
pub enum KenBurnsDirection {
    /// Zoom in toward the center.
    ZoomInCenter,
    /// Zoom in, panning to the left (camera moves left, image shifts right).
    ZoomInLeft,
    /// Zoom in, panning to the right (camera moves right, image shifts left).
    ZoomInRight,
    /// Zoom in, panning upward (camera moves up, image shifts down).
    ZoomInUp,
    /// Zoom in, panning downward (camera moves down, image shifts up).
    ZoomInDown,
    /// Zoom out from the center.
    ZoomOutCenter,
    /// Zoom out, panning from the left.
    ZoomOutLeft,
    /// Zoom out, panning from the right.
    ZoomOutRight,
    /// Zoom out, panning from the top.
    ZoomOutUp,
    /// Zoom out, panning from the bottom.
    ZoomOutDown,
    /// Custom focal point with start and end positions as fractional coordinates.
    FocalPoint {
        /// Starting focal position [x, y] in [0..1].
        start: [f32; 2],
        /// Ending focal position [x, y] in [0..1].
        end: [f32; 2],
    },
}

/// Per-segment parameters for a Ken Burns effect.
///
/// Required fields are enforced at compile time by `bon`. Optional fields
/// default to `None` and are resolved from the builder-level defaults during
/// [`KenBurnsBuilder::build`].
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct KenBurnsSegmentParams {
    /// Path to the image for this segment.
    pub image_path: String,
    /// Pan/zoom direction for this segment.
    pub direction: KenBurnsDirection,
    /// Hold duration before fading. Falls back to builder default.
    pub hold_duration: Option<f64>,
    /// Fade duration. Falls back to builder default.
    pub fade_duration: Option<f64>,
    /// Starting zoom scale. Falls back to builder default.
    pub zoom_start: Option<f32>,
    /// Ending zoom scale. Falls back to builder default.
    pub zoom_end: Option<f32>,
    /// Pan intensity as a fraction of resolution. Falls back to builder default.
    pub pan_fraction: Option<f32>,
    /// Easing curve for animations. Falls back to builder default.
    pub easing: Option<Easing>,
}

/// Resolved segment with all fields filled in (no `Option`s).
struct KenBurnsSegment {
    image_path: String,
    direction: KenBurnsDirection,
    hold_duration: f64,
    fade_duration: f64,
    zoom_start: f32,
    zoom_end: f32,
    pan_fraction: f32,
    easing: Easing,
}

/// Global parameters for the Ken Burns effect.
///
/// Required fields (`duration`, `resolution`) are enforced at compile time.
/// Optional fields provide builder-level defaults for any segment that
/// doesn't specify its own value.
#[derive(bon::Builder)]
pub struct KenBurnsParams {
    /// Total effect duration in seconds.
    pub duration: f64,
    /// Output resolution [width, height].
    pub resolution: [u32; 2],
    /// Default hold duration in seconds (per segment).
    #[builder(default = 10.0)]
    pub default_hold_duration: f64,
    /// Default fade duration in seconds (per segment).
    #[builder(default = 2.0)]
    pub default_fade_duration: f64,
    /// Default starting zoom scale.
    #[builder(default = 1.0)]
    pub default_zoom_start: f32,
    /// Default ending zoom scale.
    #[builder(default = 1.3)]
    pub default_zoom_end: f32,
    /// Default pan fraction of resolution.
    #[builder(default = 0.1)]
    pub default_pan_fraction: f32,
    /// Default easing curve for animations.
    #[builder(default = Easing::SineInOut)]
    pub default_easing: Easing,
    /// Base z-index for the first clip. Higher z-index = rendered on top.
    #[builder(default = 0)]
    pub base_z_index: i32,
}

/// Builds a Ken Burns effect that produces `Vec<ClipBuilder>`.
///
/// Accumulate segments via [`add_segment`](KenBurnsBuilder::add_segment), then
/// call [`build`](KenBurnsBuilder::build) to produce the clip list.
pub struct KenBurnsBuilder {
    params: KenBurnsParams,
    segments: Vec<KenBurnsSegmentParams>,
}

impl KenBurnsBuilder {
    /// Creates a new builder with the given global parameters.
    pub fn new(params: KenBurnsParams) -> Self {
        Self {
            params,
            segments: vec![],
        }
    }

    /// Adds a segment to the effect.
    ///
    /// Segments are ordered top-to-bottom in the z-stack: the first segment
    /// added is the topmost (fades first).
    #[must_use]
    pub fn add_segment(mut self, segment: KenBurnsSegmentParams) -> Self {
        self.segments.push(segment);
        self
    }

    /// Builds all segments into a list of `ClipBuilder`s.
    ///
    /// Each segment becomes one clip with pan/zoom and opacity animations.
    /// All clips span the full effect duration. Opacity follows the stack-based
    /// pop model where each clip holds then fades to reveal the one below.
    pub fn build(self) -> Vec<ClipBuilder> {
        if self.segments.is_empty() {
            return vec![];
        }

        let params = &self.params;
        let n = self.segments.len();

        // Resolve defaults for each segment.
        let resolved: Vec<KenBurnsSegment> = self
            .segments
            .into_iter()
            .map(|seg| KenBurnsSegment {
                image_path: seg.image_path,
                direction: seg.direction,
                hold_duration: seg.hold_duration.unwrap_or(params.default_hold_duration),
                fade_duration: seg.fade_duration.unwrap_or(params.default_fade_duration),
                zoom_start: seg.zoom_start.unwrap_or(params.default_zoom_start),
                zoom_end: seg.zoom_end.unwrap_or(params.default_zoom_end),
                pan_fraction: seg.pan_fraction.unwrap_or(params.default_pan_fraction),
                easing: seg.easing.unwrap_or(params.default_easing),
            })
            .collect();

        // Compute timing arrays.
        let holds: Vec<f64> = resolved.iter().map(|s| s.hold_duration).collect();
        let fades: Vec<f64> = resolved.iter().map(|s| s.fade_duration).collect();
        let timings = timing::compute_segment_timings(&holds, &fades);

        // Warn if segments overflow the effect duration.
        let total_time = timing::total_segment_time(&holds, &fades);
        if total_time > params.duration {
            tracing::warn!(
                "Ken Burns segments require {total_time:.1}s but effect duration is {:.1}s. \
                 Some clips may not fade completely.",
                params.duration
            );
        }

        // Build clips.
        let mut clips = Vec::with_capacity(n);
        for (i, seg) in resolved.iter().enumerate() {
            // z-index: segment 0 (top of stack) gets highest z.
            #[allow(clippy::cast_possible_wrap)]
            let z_index = params.base_z_index + (n as i32 - 1 - i as i32);

            let clip_params = ClipParams::builder()
                .id(format!("ken_burns_{i}"))
                .path(&seg.image_path)
                .start_time(0.0)
                .end_time(params.duration)
                .z_index(z_index)
                .sizing(sizing::fit_rect_cover(
                    0,
                    0,
                    params.resolution[0],
                    params.resolution[1],
                ))
                .build();

            let mut clip_builder = ClipBuilder::new(clip_params);

            // Pan/zoom animations.
            let keyframes = mapping::direction_to_keyframes(
                &seg.direction,
                seg.pan_fraction,
                params.resolution,
                seg.zoom_start,
                seg.zoom_end,
            );

            // Segment visibility window: the clip is the topmost visible layer from
            // when the clip above finishes fading, until this clip finishes fading.
            let seg_start = if i > 0 { timings[i - 1].fade_end } else { 0.0 };
            let seg_end = timings[i].fade_end;

            // Scale animation scoped to segment visibility.
            let scale_anim = AnimBuilder::scale_x()
                .keyframe_with_easing(seg_start, keyframes.scale_start, seg.easing)
                .keyframe_with_easing(seg_end, keyframes.scale_end, seg.easing);
            clip_builder = clip_builder.add_animation(scale_anim);

            let scale_y_anim = AnimBuilder::scale_y()
                .keyframe_with_easing(seg_start, keyframes.scale_start, seg.easing)
                .keyframe_with_easing(seg_end, keyframes.scale_end, seg.easing);
            clip_builder = clip_builder.add_animation(scale_y_anim);

            // Translate animations scoped to segment visibility.
            if let Some((tx_start, tx_end)) = keyframes.translate_x {
                let tx_anim = AnimBuilder::translate_x()
                    .keyframe_with_easing(seg_start, tx_start, seg.easing)
                    .keyframe_with_easing(seg_end, tx_end, seg.easing);
                clip_builder = clip_builder.add_animation(tx_anim);
            }
            if let Some((ty_start, ty_end)) = keyframes.translate_y {
                let ty_anim = AnimBuilder::translate_y()
                    .keyframe_with_easing(seg_start, ty_start, seg.easing)
                    .keyframe_with_easing(seg_end, ty_end, seg.easing);
                clip_builder = clip_builder.add_animation(ty_anim);
            }

            // Opacity animation: hold at 1.0 then fade to 0.0.
            // Two keyframes: fade_start (1.0) and fade_end (0.0).
            // The interpolation engine holds the first keyframe value before its time,
            // so the clip is implicitly at 1.0 from t=0 to fade_start.
            let opacity_anim = AnimBuilder::opacity()
                .keyframe_with_easing(timings[i].fade_start, 1.0, seg.easing)
                .keyframe_with_easing(timings[i].fade_end, 0.0, seg.easing);
            clip_builder = clip_builder.add_animation(opacity_anim);

            clips.push(clip_builder);
        }

        clips
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn default_params() -> KenBurnsParams {
        KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .build()
    }

    fn minimal_segment(path: &str, direction: KenBurnsDirection) -> KenBurnsSegmentParams {
        KenBurnsSegmentParams::builder()
            .image_path(path)
            .direction(direction)
            .build()
    }

    #[test]
    fn single_segment_produces_one_clip() {
        // Given a builder with one segment.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then one clip is produced with correct z-index and sizing.
        assert_eq!(clips.len(), 1);
        // Build the clip to inspect.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        assert_eq!(clip.id, "ken_burns_0");
        assert_eq!(clip.z_index, 0);
        assert_eq!(clip.start_time, 0.0);
        assert_eq!(clip.end_time, 30.0);
    }

    #[test]
    fn three_segments_descending_z_index() {
        // Given a builder with three segments.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(minimal_segment("img2.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(minimal_segment("img3.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then three clips have descending z-index (first = top of stack).
        assert_eq!(clips.len(), 3);
        let built: Vec<_> = clips.into_iter().map(|cb| cb.build().unwrap()).collect();

        assert_eq!(built[0].z_index, 2); // top of stack
        assert_eq!(built[1].z_index, 1);
        assert_eq!(built[2].z_index, 0); // bottom of stack
    }

    #[test]
    fn each_clip_has_scale_and_opacity_animations() {
        // Given a builder with one segment.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then the clip has scale_x, scale_y, and opacity animations.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        let props: Vec<_> = clip.animations.iter().map(|a| a.property).collect();
        use ss_core::AnimatableProperty;
        assert!(props.contains(&AnimatableProperty::ScaleX));
        assert!(props.contains(&AnimatableProperty::ScaleY));
        assert!(props.contains(&AnimatableProperty::Opacity));
    }

    #[test]
    fn center_direction_no_translate() {
        // Given a ZoomInCenter direction.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then the clip has no translate animations.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        use ss_core::AnimatableProperty;
        assert!(
            clip.animations
                .iter()
                .all(|a| a.property != AnimatableProperty::TranslateX
                    && a.property != AnimatableProperty::TranslateY)
        );
    }

    #[test]
    fn non_center_direction_has_translate() {
        // Given a ZoomInLeft direction.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInLeft))
            .build();

        // Then the clip has a translate_x animation.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        use ss_core::AnimatableProperty;
        assert!(
            clip.animations
                .iter()
                .any(|a| a.property == AnimatableProperty::TranslateX)
        );
    }

    #[test]
    fn opacity_keyframes_hold_then_fade() {
        // Given a builder with hold=5.0, fade=2.0.
        let params = KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .default_hold_duration(5.0)
            .default_fade_duration(2.0)
            .build();

        // When building a single segment.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then opacity has 2 keyframes: fade_start=5.0 (1.0) and fade_end=7.0 (0.0).
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        let opacity = clip
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity.keyframes.len(), 2);
        assert_eq!(opacity.keyframes[0].time, 5.0);
        assert_eq!(opacity.keyframes[0].value, 1.0);
        assert_eq!(opacity.keyframes[1].time, 7.0);
        assert_eq!(opacity.keyframes[1].value, 0.0);
    }

    #[test]
    fn per_segment_zoom_override() {
        // Given a segment with custom zoom_start/zoom_end.
        let params = default_params();
        let seg = KenBurnsSegmentParams::builder()
            .image_path("img1.png")
            .direction(KenBurnsDirection::ZoomInCenter)
            .zoom_start(0.5)
            .zoom_end(2.0)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params).add_segment(seg).build();

        // Then the scale animation uses the overridden values.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        let scale_x = clip
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::ScaleX)
            .unwrap();
        assert_eq!(scale_x.keyframes[0].value, 0.5);
        assert_eq!(scale_x.keyframes[1].value, 2.0);
    }

    #[test]
    fn multi_segment_scale_keyframes_scoped_to_visibility_window() {
        // Given three segments with hold=5.0, fade=2.0.
        let params = KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .default_hold_duration(5.0)
            .default_fade_duration(2.0)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(minimal_segment("img2.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(minimal_segment("img3.png", KenBurnsDirection::ZoomInCenter))
            .build();

        let built: Vec<_> = clips.into_iter().map(|cb| cb.build().unwrap()).collect();

        // Segment 0 (top of stack): visible from 0.0 to fade_end=7.0.
        let scale_0 = built[0]
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::ScaleX)
            .unwrap();
        assert_eq!(scale_0.keyframes[0].time, 0.0);
        assert_eq!(scale_0.keyframes[1].time, 7.0);

        // Segment 1 (middle): visible from fade_end[0]=7.0 to fade_end[1]=14.0.
        let scale_1 = built[1]
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::ScaleX)
            .unwrap();
        assert_eq!(scale_1.keyframes[0].time, 7.0);
        assert_eq!(scale_1.keyframes[1].time, 14.0);

        // Segment 2 (bottom): visible from fade_end[1]=14.0 to fade_end[2]=21.0.
        let scale_2 = built[2]
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::ScaleX)
            .unwrap();
        assert_eq!(scale_2.keyframes[0].time, 14.0);
        assert_eq!(scale_2.keyframes[1].time, 21.0);
    }

    #[test]
    fn multi_segment_translate_scoped_to_visibility_window() {
        // Given two segments with ZoomInLeft (has translate_x).
        let params = KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .default_hold_duration(5.0)
            .default_fade_duration(2.0)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInLeft))
            .add_segment(minimal_segment("img2.png", KenBurnsDirection::ZoomInLeft))
            .build();

        let built: Vec<_> = clips.into_iter().map(|cb| cb.build().unwrap()).collect();

        // Segment 0: translate_x scoped from 0.0 to 7.0.
        let tx_0 = built[0]
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::TranslateX)
            .unwrap();
        assert_eq!(tx_0.keyframes[0].time, 0.0);
        assert_eq!(tx_0.keyframes[1].time, 7.0);

        // Segment 1: translate_x scoped from 7.0 to 14.0.
        let tx_1 = built[1]
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::TranslateX)
            .unwrap();
        assert_eq!(tx_1.keyframes[0].time, 7.0);
        assert_eq!(tx_1.keyframes[1].time, 14.0);
    }

    #[test]
    fn per_segment_hold_fade_override() {
        // Given a segment with custom hold/fade durations.
        let params = default_params();
        let seg = KenBurnsSegmentParams::builder()
            .image_path("img1.png")
            .direction(KenBurnsDirection::ZoomInCenter)
            .hold_duration(3.0)
            .fade_duration(1.0)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params).add_segment(seg).build();

        // Then opacity timing uses the overridden durations.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        let opacity = clip
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity.keyframes[0].time, 3.0); // hold=3.0
        assert_eq!(opacity.keyframes[1].time, 4.0); // hold+fade=4.0
    }

    #[test]
    fn defaults_fallback_to_builder_level() {
        // Given a segment without overrides.
        let params = KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .default_hold_duration(7.0)
            .default_fade_duration(3.0)
            .default_zoom_start(0.8)
            .default_zoom_end(1.5)
            .default_pan_fraction(0.2)
            .default_easing(Easing::Linear)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then the clip uses builder defaults.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        let opacity = clip
            .animations
            .iter()
            .find(|a| a.property == ss_core::AnimatableProperty::Opacity)
            .unwrap();
        assert_eq!(opacity.keyframes[0].time, 7.0);
        assert_eq!(opacity.keyframes[1].time, 10.0);
        assert_eq!(opacity.keyframes[0].easing, Easing::Linear);
    }

    #[test]
    fn empty_builder_returns_empty_vec() {
        // Given a builder with no segments.
        let params = default_params();

        // When building.
        let clips = KenBurnsBuilder::new(params).build();

        // Then result is empty.
        assert!(clips.is_empty());
    }

    #[test]
    fn focal_point_direction_produces_both_translates() {
        // Given a FocalPoint direction.
        let params = default_params();
        let seg = KenBurnsSegmentParams::builder()
            .image_path("img1.png")
            .direction(KenBurnsDirection::FocalPoint {
                start: [0.0, 0.0],
                end: [1.0, 1.0],
            })
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params).add_segment(seg).build();

        // Then the clip has both translate_x and translate_y.
        let clip = clips.into_iter().next().unwrap().build().unwrap();
        use ss_core::AnimatableProperty;
        assert!(
            clip.animations
                .iter()
                .any(|a| a.property == AnimatableProperty::TranslateX)
        );
        assert!(
            clip.animations
                .iter()
                .any(|a| a.property == AnimatableProperty::TranslateY)
        );
    }

    #[test]
    fn base_z_index_offsets_all_clips() {
        // Given a builder with base_z_index=10 and two segments.
        let params = KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .base_z_index(10)
            .build();

        // When building.
        let clips = KenBurnsBuilder::new(params)
            .add_segment(minimal_segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(minimal_segment("img2.png", KenBurnsDirection::ZoomInCenter))
            .build();

        // Then z-indices are offset from the base.
        let built: Vec<_> = clips.into_iter().map(|cb| cb.build().unwrap()).collect();
        assert_eq!(built[0].z_index, 11); // top: base + (2 - 1 - 0)
        assert_eq!(built[1].z_index, 10); // bottom: base + (2 - 1 - 1)
    }
}
