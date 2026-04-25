//! Integration tests for the full Ken Burns pipeline.
//!
//! These tests verify that the Ken Burns builder, opacity transform, and
//! `ProjectBuilder` composability methods (`add_clips_at`, `with_offset`)
//! compose correctly end-to-end.

#![allow(clippy::float_cmp)]

use std::sync::{Arc, Mutex};

use ss_core::AnimatableProperty;
use ss_effects::{
    KenBurnsBuilder, KenBurnsDirection, KenBurnsParams, KenBurnsSegmentParams, opacity,
};
use ss_project_builder::{ClipBuilder, ClipParams, ProjectBuilder, ProjectParams};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Default test params: 1920×1080, 60 fps, 30s duration.
fn default_params() -> KenBurnsParams {
    KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1920, 1080])
        .build()
}

/// Creates a minimal segment with the given image path and direction.
fn segment(path: &str, direction: KenBurnsDirection) -> KenBurnsSegmentParams {
    KenBurnsSegmentParams::builder()
        .image_path(path)
        .direction(direction)
        .build()
}

/// Creates a `ProjectParams` with enough duration for the test.
fn project_params(duration: f64) -> ProjectParams {
    ProjectParams::builder()
        .resolution([1920, 1080])
        .fps(60)
        .duration(duration)
        .output("out.mp4")
        .build()
}

/// Extracts the opacity animation from a built clip.
fn opacity_track(clip: &ss_core::ClipDef) -> &ss_core::AnimationTrack {
    clip.animations
        .iter()
        .find(|a| a.property == AnimatableProperty::Opacity)
        .expect("clip should have opacity animation")
}

/// Extracts the translate_x animation from a built clip.
fn translate_x_track(clip: &ss_core::ClipDef) -> &ss_core::AnimationTrack {
    clip.animations
        .iter()
        .find(|a| a.property == AnimatableProperty::TranslateX)
        .expect("clip should have translate_x animation")
}

/// Extracts the scale_x animation from a built clip.
fn scale_x_track(clip: &ss_core::ClipDef) -> &ss_core::AnimationTrack {
    clip.animations
        .iter()
        .find(|a| a.property == AnimatableProperty::ScaleX)
        .expect("clip should have scale_x animation")
}

// ---------------------------------------------------------------------------
// Step 2: End-to-end multi-image Ken Burns + static overlay
// ---------------------------------------------------------------------------

#[test]
fn e2e_multi_image_ken_burns_with_overlay_produces_valid_project() {
    // Given a KenBurnsBuilder with 3 segments (different images, different directions).
    let clips = KenBurnsBuilder::new(default_params())
        .add_segment(segment("img1.png", KenBurnsDirection::ZoomInLeft))
        .add_segment(segment("img2.png", KenBurnsDirection::ZoomOutCenter))
        .add_segment(segment("img3.png", KenBurnsDirection::ZoomInRight))
        .build();

    // And a static overlay clip.
    let overlay = ClipBuilder::new(
        ClipParams::builder()
            .id("overlay")
            .path("overlay.png")
            .end_time(30.0)
            .z_index(100)
            .build(),
    );

    // When building via ProjectBuilder::add_clips_at(0.0, ...).add_clip(overlay).
    let project = ProjectBuilder::new(project_params(30.0))
        .add_clips_at(0.0, clips)
        .add_clip(overlay)
        .build()
        .unwrap();

    // Then the project has 4 clips total (3 Ken Burns + 1 overlay).
    assert_eq!(project.clips.len(), 4);

    // And Ken Burns clips are ordered by descending z-index.
    let ken_burns_clips: Vec<_> = project
        .clips
        .iter()
        .filter(|c| c.id.starts_with("ken_burns_"))
        .collect();
    assert_eq!(ken_burns_clips.len(), 3);
    assert!(ken_burns_clips[0].z_index > ken_burns_clips[1].z_index);
    assert!(ken_burns_clips[1].z_index > ken_burns_clips[2].z_index);

    // And the overlay clip has a distinct z-index.
    let overlay_clip = project.clips.iter().find(|c| c.id == "overlay").unwrap();
    assert_eq!(overlay_clip.z_index, 100);

    // And the JSON output is valid (round-trips through serde).
    let json = serde_json::to_string_pretty(&project).unwrap();
    let back: ss_core::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.clips.len(), 4);
}

// ---------------------------------------------------------------------------
// Step 3: Landscape vs portrait resolution scaling
// ---------------------------------------------------------------------------

#[test]
fn translations_scale_proportionally_between_landscape_and_portrait() {
    // Given a KenBurnsBuilder with a ZoomInLeft segment, built twice.
    let landscape_params = KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1920, 1080])
        .build();
    let portrait_params = KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1080, 1920])
        .build();

    let landscape_clips = KenBurnsBuilder::new(landscape_params)
        .add_segment(segment("img.png", KenBurnsDirection::ZoomInLeft))
        .build();
    let portrait_clips = KenBurnsBuilder::new(portrait_params)
        .add_segment(segment("img.png", KenBurnsDirection::ZoomInLeft))
        .build();

    // When extracting translate_x keyframe values.
    let land_clip = landscape_clips.into_iter().next().unwrap().build().unwrap();
    let port_clip = portrait_clips.into_iter().next().unwrap().build().unwrap();

    let land_tx = translate_x_track(&land_clip);
    let port_tx = translate_x_track(&port_clip);

    // Then landscape translate_x_end = 0.1 × 1920 = 192.0.
    assert_eq!(land_tx.keyframes[1].value, 192.0);

    // And portrait translate_x_end = 0.1 × 1080 = 108.0.
    assert_eq!(port_tx.keyframes[1].value, 108.0);

    // And the ratio matches the resolution ratio.
    let ratio = land_tx.keyframes[1].value / port_tx.keyframes[1].value;
    assert!((ratio - (1920.0_f32 / 1080.0_f32)).abs() < 0.01);
}

// ---------------------------------------------------------------------------
// Step 4: Stack pop timing — sequential fades, no overlap
// ---------------------------------------------------------------------------

#[test]
fn stack_pop_timing_no_overlap_no_gap() {
    // Given a KenBurnsBuilder with 3 segments, each hold=5.0, fade=2.0.
    let params = KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1920, 1080])
        .default_hold_duration(5.0)
        .default_fade_duration(2.0)
        .build();

    let clips = KenBurnsBuilder::new(params)
        .add_segment(segment("img1.png", KenBurnsDirection::ZoomInCenter))
        .add_segment(segment("img2.png", KenBurnsDirection::ZoomInCenter))
        .add_segment(segment("img3.png", KenBurnsDirection::ZoomInCenter))
        .build();

    let built: Vec<_> = clips.into_iter().map(|c| c.build().unwrap()).collect();

    // When extracting opacity keyframe times from each clip.
    // Clip 0 (top, z=2): fades 5.0→7.0.
    let op0 = opacity_track(&built[0]);
    assert_eq!(op0.keyframes[0].time, 5.0);
    assert_eq!(op0.keyframes[0].value, 1.0);
    assert_eq!(op0.keyframes[1].time, 7.0);
    assert_eq!(op0.keyframes[1].value, 0.0);

    // Clip 1 (mid, z=1): fades 12.0→14.0.
    let op1 = opacity_track(&built[1]);
    assert_eq!(op1.keyframes[0].time, 12.0);
    assert_eq!(op1.keyframes[0].value, 1.0);
    assert_eq!(op1.keyframes[1].time, 14.0);
    assert_eq!(op1.keyframes[1].value, 0.0);

    // Clip 2 (bottom, z=0): fades 19.0→21.0.
    let op2 = opacity_track(&built[2]);
    assert_eq!(op2.keyframes[0].time, 19.0);
    assert_eq!(op2.keyframes[0].value, 1.0);
    assert_eq!(op2.keyframes[1].time, 21.0);
    assert_eq!(op2.keyframes[1].value, 0.0);

    // Then no two clips have overlapping fade windows.
    let fade_ranges: Vec<_> = built
        .iter()
        .map(|c| {
            let op = opacity_track(c);
            (op.keyframes[0].time, op.keyframes[1].time)
        })
        .collect();

    for i in 0..fade_ranges.len() {
        for j in (i + 1)..fade_ranges.len() {
            let (a_start, a_end) = fade_ranges[i];
            let (b_start, b_end) = fade_ranges[j];
            assert!(
                a_end <= b_start || b_end <= a_start,
                "fade windows overlap: [{a_start}, {a_end}) and [{b_start}, {b_end})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Step 5: FocalPoint direction — custom translate values
// ---------------------------------------------------------------------------

#[test]
fn focal_point_direction_produces_correct_translate_keyframes() {
    // Given a KenBurnsDirection::FocalPoint with start=[0.25, 0.25], end=[0.75, 0.75].
    // Using default zoom_start=1.0, zoom_end=1.3, resolution=[1920, 1080].
    let params = KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1920, 1080])
        .build();

    let clips = KenBurnsBuilder::new(params)
        .add_segment(
            KenBurnsSegmentParams::builder()
                .image_path("img.png")
                .direction(KenBurnsDirection::FocalPoint {
                    start: [0.25, 0.25],
                    end: [0.75, 0.75],
                })
                .build(),
        )
        .build();

    let clip = clips.into_iter().next().unwrap().build().unwrap();

    // Then translate_x: start=(0.5-0.25)*1920*1.0=480.0, end=(0.5-0.75)*1920*1.3=-624.0.
    let tx = clip
        .animations
        .iter()
        .find(|a| a.property == AnimatableProperty::TranslateX)
        .unwrap();
    assert_eq!(tx.keyframes[0].value, 480.0);
    assert_eq!(tx.keyframes[1].value, -624.0);

    // And translate_y: start=(0.5-0.25)*1080*1.0=270.0, end=(0.5-0.75)*1080*1.3=-351.0.
    let ty = clip
        .animations
        .iter()
        .find(|a| a.property == AnimatableProperty::TranslateY)
        .unwrap();
    assert_eq!(ty.keyframes[0].value, 270.0);
    assert_eq!(ty.keyframes[1].value, -351.0);
}

// ---------------------------------------------------------------------------
// Step 6: Per-segment overrides
// ---------------------------------------------------------------------------

#[test]
fn per_segment_overrides_take_precedence_over_defaults() {
    // Given a builder with default zoom_start=1.0, zoom_end=1.3, hold=5.0, fade=2.0.
    let params = KenBurnsParams::builder()
        .duration(30.0)
        .resolution([1920, 1080])
        .default_hold_duration(5.0)
        .default_fade_duration(2.0)
        .build();

    // And two segments where the second overrides zoom_start, zoom_end, hold, fade.
    let clips = KenBurnsBuilder::new(params)
        .add_segment(segment("img1.png", KenBurnsDirection::ZoomInCenter))
        .add_segment(
            KenBurnsSegmentParams::builder()
                .image_path("img2.png")
                .direction(KenBurnsDirection::ZoomInCenter)
                .zoom_start(0.5)
                .zoom_end(2.0)
                .hold_duration(3.0)
                .fade_duration(1.0)
                .build(),
        )
        .build();

    let built: Vec<_> = clips.into_iter().map(|c| c.build().unwrap()).collect();

    // Then clip 0 uses defaults: scale 1.0→1.3, opacity timing hold=5.0, fade=2.0.
    let scale0 = scale_x_track(&built[0]);
    assert_eq!(scale0.keyframes[0].value, 1.0);
    assert_eq!(scale0.keyframes[1].value, 1.3);

    let op0 = opacity_track(&built[0]);
    assert_eq!(op0.keyframes[0].time, 5.0);
    assert_eq!(op0.keyframes[1].time, 7.0);

    // And clip 1 uses overrides: scale 0.5→2.0, opacity timing hold=3.0, fade=1.0.
    let scale1 = scale_x_track(&built[1]);
    assert_eq!(scale1.keyframes[0].value, 0.5);
    assert_eq!(scale1.keyframes[1].value, 2.0);

    let op1 = opacity_track(&built[1]);
    assert_eq!(op1.keyframes[0].time, 10.0); // after clip 0 finishes (7.0) + hold(3.0)
    assert_eq!(op1.keyframes[1].time, 11.0); // 10.0 + fade(1.0)
}

// ---------------------------------------------------------------------------
// Step 7: with_offset and add_clips_at composability
// ---------------------------------------------------------------------------

#[test]
fn add_clips_at_offsets_clips_and_keyframes_into_project() {
    // Given a Ken Burns effect producing 2 clips (each at t=0..30).
    let clips = KenBurnsBuilder::new(default_params())
        .add_segment(segment("img1.png", KenBurnsDirection::ZoomInCenter))
        .add_segment(segment("img2.png", KenBurnsDirection::ZoomInCenter))
        .build();

    // When using project.add_clips_at(10.0, ...).
    let project = ProjectBuilder::new(project_params(40.0))
        .add_clips_at(10.0, clips)
        .build()
        .unwrap();

    // Then each clip has start_time=10.0, end_time=40.0.
    assert_eq!(project.clips.len(), 2);
    for clip in &project.clips {
        assert_eq!(clip.start_time, 10.0);
        assert_eq!(clip.end_time, 40.0);
    }

    // And all keyframe times across all animations are shifted by 10.0.
    for clip in &project.clips {
        for anim in &clip.animations {
            for kf in &anim.keyframes {
                assert!(
                    kf.time >= 10.0,
                    "keyframe time {} should be >= 10.0 (offset applied)",
                    kf.time
                );
            }
        }
    }
}

#[test]
fn composability_pipeline_opacity_then_add_clips_at() {
    // Given a Ken Burns effect with 2 segments.
    let ken_burns = KenBurnsBuilder::new(
        KenBurnsParams::builder()
            .duration(30.0)
            .resolution([1920, 1080])
            .default_hold_duration(5.0)
            .default_fade_duration(2.0)
            .build(),
    )
    .add_segment(segment("img1.png", KenBurnsDirection::ZoomInCenter))
    .add_segment(segment("img2.png", KenBurnsDirection::ZoomInCenter));

    // When composing: project.add_clips_at(5.0, opacity(ken_burns.build(), 0.5)).
    let clips = opacity(ken_burns.build(), 0.5);
    let project = ProjectBuilder::new(project_params(40.0))
        .add_clips_at(5.0, clips)
        .build()
        .unwrap();

    // Then the project has 2 clips.
    assert_eq!(project.clips.len(), 2);

    // Each clip is offset by 5.0 (start=5.0, end=35.0).
    for clip in &project.clips {
        assert_eq!(clip.start_time, 5.0);
        assert_eq!(clip.end_time, 35.0);
    }

    // And opacity keyframes are scaled by 0.5 (max 0.5 instead of 1.0).
    for clip in &project.clips {
        let op = opacity_track(clip);
        // The first keyframe (fade_start, value=1.0) should now be 0.5.
        assert!(
            (op.keyframes[0].value - 0.5).abs() < 0.01,
            "opacity value should be 0.5, got {}",
            op.keyframes[0].value
        );
        // The second keyframe (fade_end, value=0.0) should still be 0.0.
        assert!(
            (op.keyframes[1].value - 0.0).abs() < 0.01,
            "opacity value should be 0.0, got {}",
            op.keyframes[1].value
        );
    }
}

// ---------------------------------------------------------------------------
// Step 8: Warning emission when segments exceed duration
// ---------------------------------------------------------------------------

#[test]
fn build_emits_warning_when_segments_exceed_duration() {
    // Given a builder with duration=10.0 but segments needing 20s total
    // (2 segments × hold=8 + fade=2 = 20s).
    let params = KenBurnsParams::builder()
        .duration(10.0)
        .resolution([1920, 1080])
        .default_hold_duration(8.0)
        .default_fade_duration(2.0)
        .build();

    let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let writer = buf.clone();

    // When building under a tracing subscriber that captures output.
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || WriterClone(writer.clone()))
        .with_target(false)
        .with_file(false)
        .finish();

    let clips = tracing::subscriber::with_default(subscriber, || {
        KenBurnsBuilder::new(params)
            .add_segment(segment("img1.png", KenBurnsDirection::ZoomInCenter))
            .add_segment(segment("img2.png", KenBurnsDirection::ZoomInCenter))
            .build()
    });

    // Then the build still succeeds (returns clips).
    assert_eq!(clips.len(), 2);

    // And a tracing::warn! was emitted containing "Ken Burns segments require".
    let binding = buf.lock().unwrap();
    let captured = String::from_utf8_lossy(&binding);
    assert!(
        captured.contains("Ken Burns segments require"),
        "expected warning about segments exceeding duration, got: {captured}"
    );
}

/// A `MakeWriter` clone-able wrapper for capturing tracing output.
#[derive(Clone)]
struct WriterClone(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for WriterClone {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

// ---------------------------------------------------------------------------
// Step 9: Single segment edge case
// ---------------------------------------------------------------------------

#[test]
fn single_segment_produces_pan_zoom_and_opacity_fade() {
    // Given a builder with 1 segment, duration=15.0, ZoomInLeft direction.
    let params = KenBurnsParams::builder()
        .duration(15.0)
        .resolution([1920, 1080])
        .default_hold_duration(5.0)
        .default_fade_duration(2.0)
        .build();

    let clips = KenBurnsBuilder::new(params)
        .add_segment(segment("img.png", KenBurnsDirection::ZoomInLeft))
        .build();

    // Then 1 clip is produced with z_index=0.
    assert_eq!(clips.len(), 1);
    let clip = clips.into_iter().next().unwrap().build().unwrap();
    assert_eq!(clip.z_index, 0);

    // The clip has scale_x, scale_y, translate_x, and opacity animations.
    let properties: Vec<_> = clip.animations.iter().map(|a| a.property).collect();
    assert!(properties.contains(&AnimatableProperty::ScaleX));
    assert!(properties.contains(&AnimatableProperty::ScaleY));
    assert!(properties.contains(&AnimatableProperty::TranslateX));
    assert!(properties.contains(&AnimatableProperty::Opacity));

    // Opacity fades from 1.0 at t=5.0 to 0.0 at t=7.0.
    let op = opacity_track(&clip);
    assert_eq!(op.keyframes[0].time, 5.0);
    assert_eq!(op.keyframes[0].value, 1.0);
    assert_eq!(op.keyframes[1].time, 7.0);
    assert_eq!(op.keyframes[1].value, 0.0);

    // The clip can be used in a project and serialized to JSON.
    //
    // Re-build the Ken Burns clips and add them directly via add_clips_at
    // to verify the full pipeline without reconstructing from ClipDef.
    let params2 = KenBurnsParams::builder()
        .duration(15.0)
        .resolution([1920, 1080])
        .default_hold_duration(5.0)
        .default_fade_duration(2.0)
        .build();
    let clips2 = KenBurnsBuilder::new(params2)
        .add_segment(segment("img.png", KenBurnsDirection::ZoomInLeft))
        .build();
    let project = ProjectBuilder::new(project_params(15.0))
        .add_clips_at(0.0, clips2)
        .build()
        .unwrap();
    let json = serde_json::to_string_pretty(&project).unwrap();
    assert!(serde_json::from_str::<ss_core::Project>(&json).is_ok());
}
