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

impl PlacedRect {
    /// Scale this placement from project-space to viewport-space.
    ///
    /// Multiplies all fields by the ratio `(viewport_size / project_resolution)`.
    /// When viewport equals project resolution, this is an identity transform.
    pub fn scale_to_viewport(
        &self,
        project_resolution: (u32, u32),
        viewport_size: (u32, u32),
    ) -> PlacedRect {
        let sx = viewport_size.0 as f32 / project_resolution.0 as f32;
        let sy = viewport_size.1 as f32 / project_resolution.1 as f32;
        PlacedRect {
            x: self.x * sx,
            y: self.y * sy,
            w: self.w * sx,
            h: self.h * sy,
        }
    }
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
    fn natural_sizing_returns_image_dimensions_at_origin() {
        // Given a Natural sizing mode.
        let sizing = Sizing::Natural;

        // When computing placement for a 200x100 image.
        let result = compute_placement(&sizing, (200, 100), (1920, 1080));

        // Then the placement is at (0, 0) with the image's natural dimensions.
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 0.0).abs() < 1e-5);
        assert!((result.w - 200.0).abs() < 1e-5);
        assert!((result.h - 100.0).abs() < 1e-5);
    }

    #[test]
    fn explicit_sizing_returns_given_dimensions_at_origin() {
        // Given an Explicit sizing of 800x600.
        let sizing = Sizing::Explicit {
            width: 800,
            height: 600,
        };

        // When computing placement.
        let result = compute_placement(&sizing, (200, 100), (1920, 1080));

        // Then the placement is at (0, 0) with the explicit dimensions.
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 0.0).abs() < 1e-5);
        assert!((result.w - 800.0).abs() < 1e-5);
        assert!((result.h - 600.0).abs() < 1e-5);
    }

    #[test]
    fn scale_sizing_multiplies_dimensions_at_origin() {
        // Given a Scale sizing of 2.0.
        let sizing = Sizing::Scale(2.0);

        // When computing placement for a 100x50 image.
        let result = compute_placement(&sizing, (100, 50), (1920, 1080));

        // Then the placement is at (0, 0) with doubled dimensions.
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 0.0).abs() < 1e-5);
        assert!((result.w - 200.0).abs() < 1e-5);
        assert!((result.h - 100.0).abs() < 1e-5);
    }

    #[test]
    fn fit_rect_contain_wider_image_fits_to_width_and_centers_vertically() {
        // Given a FitRect Contain with a 100x100 rect and a wider image (200x100).
        let sizing = Sizing::FitRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
            mode: FitMode::Contain,
            anchor: FitAnchor::Center,
        };

        // When computing placement.
        let result = compute_placement(&sizing, (200, 100), (1920, 1080));

        // Then the image scales to fit width (100x50), centered vertically at y=25.
        assert!((result.w - 100.0).abs() < 1e-5);
        assert!((result.h - 50.0).abs() < 1e-5);
        assert!((result.x - 0.0).abs() < 1e-5);
        assert!((result.y - 25.0).abs() < 1e-5);
    }

    #[rstest::rstest]
    #[case::cover_same_aspect(
        FitMode::Cover, (400, 200), 0, 0, 200, 100,
        0.0, 0.0, 200.0, 100.0,
    )]
    #[case::contain_same_aspect(
        FitMode::Contain, (400, 200), 0, 0, 200, 100,
        0.0, 0.0, 200.0, 100.0,
    )]
    #[case::cover_wider(
        FitMode::Cover, (200, 100), 0, 0, 100, 100,
        -50.0, 0.0, 200.0, 100.0,
    )]
    #[case::cover_taller(
        FitMode::Cover, (100, 200), 0, 0, 100, 100,
        0.0, -50.0, 100.0, 200.0,
    )]
    #[case::contain_taller(
        FitMode::Contain, (100, 200), 0, 0, 100, 100,
        25.0, 0.0, 50.0, 100.0,
    )]
    fn fit_rect_scales_and_positions_correctly(
        #[case] mode: FitMode,
        #[case] image_dims: (u32, u32),
        #[case] rx: i32, #[case] ry: i32,
        #[case] rw: u32, #[case] rh: u32,
        #[case] expected_x: f32, #[case] expected_y: f32,
        #[case] expected_w: f32, #[case] expected_h: f32,
    ) {
        // Given a FitRect with the given mode, rect dimensions, and image.
        let sizing = Sizing::FitRect {
            x: rx, y: ry, w: rw, h: rh,
            mode,
            anchor: FitAnchor::Center,
        };

        // When computing placement.
        let result = compute_placement(&sizing, image_dims, (1920, 1080));

        // Then the placement matches expected values.
        assert!((result.x - expected_x).abs() < 1e-5, "x: got {}", result.x);
        assert!((result.y - expected_y).abs() < 1e-5, "y: got {}", result.y);
        assert!((result.w - expected_w).abs() < 1e-5, "w: got {}", result.w);
        assert!((result.h - expected_h).abs() < 1e-5, "h: got {}", result.h);
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

    // --- scale_to_viewport tests ---

    use super::PlacedRect;

    #[test]
    fn scale_to_viewport_identity_when_resolutions_match() {
        // Given a placement and matching project/viewport resolutions.
        let placed = PlacedRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
        };

        // When scaling to viewport with the same resolution.
        let result = placed.scale_to_viewport((200, 100), (200, 100));

        // Then the placement is unchanged.
        assert!((result.x - 10.0).abs() < 1e-5);
        assert!((result.y - 20.0).abs() < 1e-5);
        assert!((result.w - 100.0).abs() < 1e-5);
        assert!((result.h - 50.0).abs() < 1e-5);
    }

    #[test]
    fn scale_to_viewport_halves_all_fields() {
        // Given a placement in a 200x100 project.
        let placed = PlacedRect {
            x: 40.0,
            y: 30.0,
            w: 100.0,
            h: 50.0,
        };

        // When scaling to a half-resolution viewport.
        let result = placed.scale_to_viewport((200, 100), (100, 50));

        // Then all fields are halved.
        assert!((result.x - 20.0).abs() < 1e-5);
        assert!((result.y - 15.0).abs() < 1e-5);
        assert!((result.w - 50.0).abs() < 1e-5);
        assert!((result.h - 25.0).abs() < 1e-5);
    }

    #[test]
    fn scale_to_viewport_anisotropic_scales_each_axis_independently() {
        // Given a placement in a 200x100 project.
        let placed = PlacedRect {
            x: 40.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
        };

        // When scaling to a 100x200 viewport (x halved, y doubled).
        let result = placed.scale_to_viewport((200, 100), (100, 200));

        // Then x/w are halved and y/h are doubled.
        assert!((result.x - 20.0).abs() < 1e-5);
        assert!((result.y - 40.0).abs() < 1e-5);
        assert!((result.w - 50.0).abs() < 1e-5);
        assert!((result.h - 100.0).abs() < 1e-5);
    }

    #[test]
    fn scale_to_viewport_fit_rect_with_offset() {
        // Given a FitRect placement at (100, 50) with size 640x480 in a 1920x1080 project.
        let placed = PlacedRect {
            x: 100.0,
            y: 50.0,
            w: 640.0,
            h: 480.0,
        };

        // When scaling to half resolution (960x540).
        let result = placed.scale_to_viewport((1920, 1080), (960, 540));

        // Then offset and size are both halved.
        assert!((result.x - 50.0).abs() < 1e-5);
        assert!((result.y - 25.0).abs() < 1e-5);
        assert!((result.w - 320.0).abs() < 1e-5);
        assert!((result.h - 240.0).abs() < 1e-5);
    }
}
