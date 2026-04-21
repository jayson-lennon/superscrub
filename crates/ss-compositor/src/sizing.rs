//! Sizing mode computation.
//!
//! Converts a clip's [`Sizing`] mode into a concrete placement rectangle
//! on the canvas, given the source image dimensions and canvas resolution.

use ss_core::clip::{FitAnchor, FitMode, Sizing};

/// Result of computing a clip's base placement on the canvas.
#[derive(Debug, Clone)]
pub struct PlacedRect {
    /// Position of the clip's top-left corner on the canvas.
    pub x: f32,
    /// Y position of the clip's top-left corner on the canvas.
    pub y: f32,
    /// Width of the clip on the canvas (after sizing, before animation transforms).
    pub w: f32,
    /// Height of the clip on the canvas (after sizing, before animation transforms).
    pub h: f32,
}

/// Compute the base placement rect for a clip given its sizing mode,
/// the source image dimensions, and the canvas resolution.
pub fn compute_placement(
    sizing: &Sizing,
    image_dimensions: (u32, u32),
    _canvas_resolution: (u32, u32),
) -> PlacedRect {
    match sizing {
        Sizing::Natural => PlacedRect {
            x: 0.0,
            y: 0.0,
            w: image_dimensions.0 as f32,
            h: image_dimensions.1 as f32,
        },
        Sizing::Explicit { width, height } => PlacedRect {
            x: 0.0,
            y: 0.0,
            w: *width as f32,
            h: *height as f32,
        },
        Sizing::FitRect {
            x,
            y,
            w,
            h,
            mode,
            anchor,
        } => {
            let (scaled_w, scaled_h) = compute_fit_dimensions(image_dimensions, (*w, *h), mode);

            let (offset_x, offset_y) = compute_anchor_offset(
                *x as f32, *y as f32, *w as f32, *h as f32, scaled_w, scaled_h, anchor,
            );

            PlacedRect {
                x: offset_x,
                y: offset_y,
                w: scaled_w,
                h: scaled_h,
            }
        }
        Sizing::Scale(factor) => PlacedRect {
            x: 0.0,
            y: 0.0,
            w: image_dimensions.0 as f32 * factor,
            h: image_dimensions.1 as f32 * factor,
        },
    }
}

/// Compute the scaled dimensions for FitRect modes.
fn compute_fit_dimensions(
    image_dimensions: (u32, u32),
    rect_dimensions: (u32, u32),
    mode: &FitMode,
) -> (f32, f32) {
    let img_w = image_dimensions.0 as f32;
    let img_h = image_dimensions.1 as f32;
    let rect_w = rect_dimensions.0 as f32;
    let rect_h = rect_dimensions.1 as f32;

    match mode {
        FitMode::Cover => {
            let scale_x = rect_w / img_w;
            let scale_y = rect_h / img_h;
            let scale = scale_x.max(scale_y);
            (img_w * scale, img_h * scale)
        }
        FitMode::Contain => {
            let scale_x = rect_w / img_w;
            let scale_y = rect_h / img_h;
            let scale = scale_x.min(scale_y);
            (img_w * scale, img_h * scale)
        }
    }
}

/// Compute the offset based on the anchor within the FitRect.
fn compute_anchor_offset(
    rect_x: f32,
    rect_y: f32,
    rect_w: f32,
    rect_h: f32,
    scaled_w: f32,
    scaled_h: f32,
    anchor: &FitAnchor,
) -> (f32, f32) {
    match anchor {
        FitAnchor::Center => (
            rect_x + (rect_w - scaled_w) / 2.0,
            rect_y + (rect_h - scaled_h) / 2.0,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::compute_placement;
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
}
