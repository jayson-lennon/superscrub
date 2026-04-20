//! Tests for sizing mode computation.

mod test_utils;

use ss_compositor::sizing::compute_placement;
use ss_core::clip::{FitAnchor, FitMode, Sizing};

#[test]
fn natural_sizing_returns_image_dimensions() {
    // Given a Natural sizing mode.
    let sizing = Sizing::Natural;

    // When computing placement for a 200x100 image.
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));

    // Then the placement is at (0, 0) with the image's natural dimensions.
    assert!((result.w - 200.0).abs() < 1e-5);
}

#[test]
fn natural_sizing_position_is_origin() {
    let sizing = Sizing::Natural;
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));
    assert!((result.x - 0.0).abs() < 1e-5 && (result.y - 0.0).abs() < 1e-5);
}

#[test]
fn explicit_sizing_returns_given_dimensions() {
    // Given an Explicit sizing of 800x600.
    let sizing = Sizing::Explicit {
        width: 800,
        height: 600,
    };

    // When computing placement.
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));

    // Then the placement has the explicit dimensions.
    assert!((result.w - 800.0).abs() < 1e-5 && (result.h - 600.0).abs() < 1e-5);
}

#[test]
fn explicit_sizing_position_is_origin() {
    let sizing = Sizing::Explicit {
        width: 800,
        height: 600,
    };
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));
    assert!((result.x - 0.0).abs() < 1e-5 && (result.y - 0.0).abs() < 1e-5);
}

#[test]
fn scale_sizing_multiplies_dimensions() {
    // Given a Scale sizing of 2.0.
    let sizing = Sizing::Scale(2.0);

    // When computing placement for a 100x50 image.
    let result = compute_placement(&sizing, (100, 50), (1920, 1080));

    // Then the dimensions are doubled.
    assert!((result.w - 200.0).abs() < 1e-5 && (result.h - 100.0).abs() < 1e-5);
}

#[test]
fn scale_sizing_position_is_origin() {
    let sizing = Sizing::Scale(1.5);
    let result = compute_placement(&sizing, (100, 50), (1920, 1080));
    assert!((result.x - 0.0).abs() < 1e-5 && (result.y - 0.0).abs() < 1e-5);
}

#[test]
fn fit_rect_cover_same_aspect_fills_rect() {
    // Given a FitRect Cover with a 200x100 rect and same-aspect image.
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 200,
        h: 100,
        mode: FitMode::Cover,
        anchor: FitAnchor::Center,
    };

    // When computing placement for a 400x200 image (same aspect).
    let result = compute_placement(&sizing, (400, 200), (1920, 1080));

    // Then the result fills the rect exactly.
    assert!((result.w - 200.0).abs() < 1e-5 && (result.h - 100.0).abs() < 1e-5);
}

#[test]
fn fit_rect_cover_wider_image_scales_to_height() {
    // Given a FitRect Cover with a 100x100 rect and a wider image (200x100).
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        mode: FitMode::Cover,
        anchor: FitAnchor::Center,
    };

    // When computing placement for a 200x100 image.
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));

    // Then the image scales to cover the rect height (scaled to 200x100),
    // wider than the rect, centered horizontally.
    assert!((result.w - 200.0).abs() < 1e-5);
}

#[test]
fn fit_rect_cover_taller_image_scales_to_width() {
    // Given a FitRect Cover with a 100x100 rect and a taller image (100x200).
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        mode: FitMode::Cover,
        anchor: FitAnchor::Center,
    };

    // When computing placement for a 100x200 image.
    let result = compute_placement(&sizing, (100, 200), (1920, 1080));

    // Then the image scales to cover the rect width (scaled to 100x200),
    // taller than the rect, centered vertically.
    assert!((result.h - 200.0).abs() < 1e-5);
}

#[test]
fn fit_rect_contain_same_aspect_fills_rect() {
    // Given a FitRect Contain with same-aspect image.
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 200,
        h: 100,
        mode: FitMode::Contain,
        anchor: FitAnchor::Center,
    };

    let result = compute_placement(&sizing, (400, 200), (1920, 1080));

    // Same aspect → fills exactly.
    assert!((result.w - 200.0).abs() < 1e-5 && (result.h - 100.0).abs() < 1e-5);
}

#[test]
fn fit_rect_contain_wider_image_scales_to_width() {
    // Given a FitRect Contain with a 100x100 rect and a wider image (200x100).
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        mode: FitMode::Contain,
        anchor: FitAnchor::Center,
    };

    let result = compute_placement(&sizing, (200, 100), (1920, 1080));

    // Contain scales to fit width → 100x50, centered vertically.
    assert!((result.w - 100.0).abs() < 1e-5 && (result.h - 50.0).abs() < 1e-5);
}

#[test]
fn fit_rect_contain_wider_image_is_centered_vertically() {
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        mode: FitMode::Contain,
        anchor: FitAnchor::Center,
    };
    let result = compute_placement(&sizing, (200, 100), (1920, 1080));

    // Centered: y = (100 - 50) / 2 = 25.
    assert!((result.y - 25.0).abs() < 1e-5);
}

#[test]
fn fit_rect_contain_taller_image_is_centered_horizontally() {
    let sizing = Sizing::FitRect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        mode: FitMode::Contain,
        anchor: FitAnchor::Center,
    };
    let result = compute_placement(&sizing, (100, 200), (1920, 1080));

    // Scales to fit height → 50x100, centered: x = (100 - 50) / 2 = 25.
    assert!((result.x - 25.0).abs() < 1e-5);
}

#[test]
fn fit_rect_with_offset_positions_correctly() {
    let sizing = Sizing::FitRect {
        x: 50,
        y: 30,
        w: 100,
        h: 100,
        mode: FitMode::Contain,
        anchor: FitAnchor::Center,
    };
    let result = compute_placement(&sizing, (100, 100), (1920, 1080));

    // Same aspect, fills rect. Position at (50, 30).
    assert!((result.x - 50.0).abs() < 1e-5 && (result.y - 30.0).abs() < 1e-5);
}

#[test]
fn fit_rect_cover_center_anchor_offsets_negative_when_oversized() {
    // Image larger than rect in Cover mode → scaled_w > rect_w.
    let sizing = Sizing::FitRect {
        x: 10,
        y: 20,
        w: 100,
        h: 50,
        mode: FitMode::Cover,
        anchor: FitAnchor::Center,
    };
    let result = compute_placement(&sizing, (400, 100), (1920, 1080));

    // 400x100 image into 100x50 Cover. Scale to width: 100/400=0.25, height: 50/100=0.5. Max=0.5.
    // Scaled: 200x50. Offset x = 10 + (100-200)/2 = 10 - 50 = -40.
    assert!((result.x - (-40.0)).abs() < 1e-5);
}
