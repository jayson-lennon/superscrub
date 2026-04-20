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
