//! Direction-to-keyframe mapping for Ken Burns pan and zoom.
//!
//! Each [`KenBurnsDirection`](super::KenBurnsDirection) variant maps to
//! translate and scale keyframe values. Translation values are scaled by
//! `pan_fraction × resolution` so the effect works for any output resolution.

/// Translate and scale keyframe values derived from a direction.
pub struct DirectionKeyframes {
    /// (start_value, end_value) for translate_x. `None` means no horizontal pan.
    pub translate_x: Option<(f32, f32)>,
    /// (start_value, end_value) for translate_y. `None` means no vertical pan.
    pub translate_y: Option<(f32, f32)>,
    /// Starting scale value.
    pub scale_start: f32,
    /// Ending scale value.
    pub scale_end: f32,
}

/// Maps a direction to translate and scale keyframe values.
///
/// Translation pixel values are computed as `pan_fraction * resolution_axis`.
/// For `ZoomIn` variants, scale goes from `zoom_start` to `zoom_end` (typically up).
/// For `ZoomOut` variants, scale goes from `zoom_end` to `zoom_start` (typically down).
///
/// `FocalPoint` values are fractional [0..1] coordinates where [0.5, 0.5] is center.
pub fn direction_to_keyframes(
    direction: &super::KenBurnsDirection,
    pan_fraction: f32,
    resolution: [u32; 2],
    zoom_start: f32,
    zoom_end: f32,
) -> DirectionKeyframes {
    let pan_x = pan_fraction * resolution[0] as f32;
    let pan_y = pan_fraction * resolution[1] as f32;

    match direction {
        super::KenBurnsDirection::ZoomInCenter => DirectionKeyframes {
            translate_x: None,
            translate_y: None,
            scale_start: zoom_start,
            scale_end: zoom_end,
        },
        super::KenBurnsDirection::ZoomInLeft => DirectionKeyframes {
            translate_x: Some((0.0, pan_x)),
            translate_y: None,
            scale_start: zoom_start,
            scale_end: zoom_end,
        },
        super::KenBurnsDirection::ZoomInRight => DirectionKeyframes {
            translate_x: Some((0.0, -pan_x)),
            translate_y: None,
            scale_start: zoom_start,
            scale_end: zoom_end,
        },
        super::KenBurnsDirection::ZoomInUp => DirectionKeyframes {
            translate_x: None,
            translate_y: Some((0.0, pan_y)),
            scale_start: zoom_start,
            scale_end: zoom_end,
        },
        super::KenBurnsDirection::ZoomInDown => DirectionKeyframes {
            translate_x: None,
            translate_y: Some((0.0, -pan_y)),
            scale_start: zoom_start,
            scale_end: zoom_end,
        },
        super::KenBurnsDirection::ZoomOutCenter => DirectionKeyframes {
            translate_x: None,
            translate_y: None,
            scale_start: zoom_end,
            scale_end: zoom_start,
        },
        super::KenBurnsDirection::ZoomOutLeft => DirectionKeyframes {
            translate_x: Some((pan_x, 0.0)),
            translate_y: None,
            scale_start: zoom_end,
            scale_end: zoom_start,
        },
        super::KenBurnsDirection::ZoomOutRight => DirectionKeyframes {
            translate_x: Some((-pan_x, 0.0)),
            translate_y: None,
            scale_start: zoom_end,
            scale_end: zoom_start,
        },
        super::KenBurnsDirection::ZoomOutUp => DirectionKeyframes {
            translate_x: None,
            translate_y: Some((pan_y, 0.0)),
            scale_start: zoom_end,
            scale_end: zoom_start,
        },
        super::KenBurnsDirection::ZoomOutDown => DirectionKeyframes {
            translate_x: None,
            translate_y: Some((-pan_y, 0.0)),
            scale_start: zoom_end,
            scale_end: zoom_start,
        },
        super::KenBurnsDirection::FocalPoint { start, end } => {
            let res_x = resolution[0] as f32;
            let res_y = resolution[1] as f32;
            let tx_start = (0.5 - start[0]) * res_x * zoom_start;
            let tx_end = (0.5 - end[0]) * res_x * zoom_end;
            let ty_start = (0.5 - start[1]) * res_y * zoom_start;
            let ty_end = (0.5 - end[1]) * res_y * zoom_end;

            DirectionKeyframes {
                translate_x: Some((tx_start, tx_end)),
                translate_y: Some((ty_start, ty_end)),
                scale_start: zoom_start,
                scale_end: zoom_end,
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::KenBurnsDirection;

    const RES_1920X1080: [u32; 2] = [1920, 1080];
    const PAN_FRACTION: f32 = 0.1;

    #[test]
    fn zoom_in_center_no_translate() {
        // Given ZoomInCenter direction.
        let dir = KenBurnsDirection::ZoomInCenter;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then there is no translation and scale goes up.
        assert!(kf.translate_x.is_none());
        assert!(kf.translate_y.is_none());
        assert_eq!(kf.scale_start, 1.0);
        assert_eq!(kf.scale_end, 1.3);
    }

    #[test]
    fn zoom_out_center_no_translate() {
        // Given ZoomOutCenter direction.
        let dir = KenBurnsDirection::ZoomOutCenter;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then there is no translation and scale goes down.
        assert!(kf.translate_x.is_none());
        assert!(kf.translate_y.is_none());
        assert_eq!(kf.scale_start, 1.3);
        assert_eq!(kf.scale_end, 1.0);
    }

    #[test]
    fn zoom_in_left_pans_positive_x() {
        // Given ZoomInLeft direction.
        let dir = KenBurnsDirection::ZoomInLeft;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_x goes from 0 to positive pan_x.
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        assert_eq!(tx_start, 0.0);
        assert_eq!(tx_end, 192.0); // 0.1 * 1920
        assert!(kf.translate_y.is_none());
    }

    #[test]
    fn zoom_in_right_pans_negative_x() {
        // Given ZoomInRight direction.
        let dir = KenBurnsDirection::ZoomInRight;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_x goes from 0 to negative pan_x.
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        assert_eq!(tx_start, 0.0);
        assert_eq!(tx_end, -192.0);
    }

    #[test]
    fn zoom_in_up_pans_positive_y() {
        // Given ZoomInUp direction.
        let dir = KenBurnsDirection::ZoomInUp;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_y goes from 0 to positive pan_y.
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(ty_start, 0.0);
        assert_eq!(ty_end, 108.0); // 0.1 * 1080
    }

    #[test]
    fn zoom_in_down_pans_negative_y() {
        // Given ZoomInDown direction.
        let dir = KenBurnsDirection::ZoomInDown;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_y goes from 0 to negative pan_y.
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(ty_start, 0.0);
        assert_eq!(ty_end, -108.0);
    }

    #[test]
    fn zoom_out_left_starts_positive_x() {
        // Given ZoomOutLeft direction.
        let dir = KenBurnsDirection::ZoomOutLeft;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_x goes from positive pan_x to 0.
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        assert_eq!(tx_start, 192.0);
        assert_eq!(tx_end, 0.0);
    }

    #[test]
    fn zoom_out_right_starts_negative_x() {
        // Given ZoomOutRight direction.
        let dir = KenBurnsDirection::ZoomOutRight;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_x goes from negative pan_x to 0.
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        assert_eq!(tx_start, -192.0);
        assert_eq!(tx_end, 0.0);
    }

    #[test]
    fn zoom_out_up_starts_positive_y() {
        // Given ZoomOutUp direction.
        let dir = KenBurnsDirection::ZoomOutUp;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_y goes from positive pan_y to 0.
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(ty_start, 108.0);
        assert_eq!(ty_end, 0.0);
    }

    #[test]
    fn zoom_out_down_starts_negative_y() {
        // Given ZoomOutDown direction.
        let dir = KenBurnsDirection::ZoomOutDown;

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate_y goes from negative pan_y to 0.
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(ty_start, -108.0);
        assert_eq!(ty_end, 0.0);
    }

    #[test]
    fn focal_point_center_produces_zero_translate() {
        // Given a focal point at center [0.5, 0.5].
        let dir = KenBurnsDirection::FocalPoint {
            start: [0.5, 0.5],
            end: [0.5, 0.5],
        };

        // When mapping to keyframes.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.3);

        // Then translate values are both zero.
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(tx_start, 0.0);
        assert_eq!(tx_end, 0.0);
        assert_eq!(ty_start, 0.0);
        assert_eq!(ty_end, 0.0);
    }

    #[test]
    fn focal_point_top_left_produces_positive_translate() {
        // Given a focal point at top-left [0, 0].
        let dir = KenBurnsDirection::FocalPoint {
            start: [0.0, 0.0],
            end: [1.0, 1.0],
        };

        // When mapping to keyframes at zoom_start=1.0.
        let kf = direction_to_keyframes(&dir, PAN_FRACTION, RES_1920X1080, 1.0, 1.0);

        // Then start translate shifts right+down (positive), end shifts left+up (negative).
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        let (ty_start, ty_end) = kf.translate_y.unwrap();
        assert_eq!(tx_start, 0.5 * 1920.0); // (0.5 - 0) * 1920 * 1.0
        assert_eq!(tx_end, -0.5 * 1920.0); // (0.5 - 1.0) * 1920 * 1.0
        assert_eq!(ty_start, 0.5 * 1080.0);
        assert_eq!(ty_end, -0.5 * 1080.0);
    }

    #[test]
    fn translation_scales_with_resolution() {
        // Given the same direction for landscape vs portrait.
        let dir = KenBurnsDirection::ZoomInLeft;
        let landscape: [u32; 2] = [1920, 1080];
        let portrait: [u32; 2] = [1080, 1920];

        // When mapping to keyframes.
        let kf_landscape = direction_to_keyframes(&dir, 0.1, landscape, 1.0, 1.3);
        let kf_portrait = direction_to_keyframes(&dir, 0.1, portrait, 1.0, 1.3);

        // Then translations scale proportionally.
        let (_, tx_land) = kf_landscape.translate_x.unwrap();
        let (_, tx_port) = kf_portrait.translate_x.unwrap();
        assert_eq!(tx_land, 192.0); // 0.1 * 1920
        assert_eq!(tx_port, 108.0); // 0.1 * 1080
    }

    #[test]
    fn pan_fraction_zero_produces_no_translate() {
        // Given a non-center direction with pan_fraction=0.
        let dir = KenBurnsDirection::ZoomInLeft;

        // When mapping with zero pan fraction.
        let kf = direction_to_keyframes(&dir, 0.0, RES_1920X1080, 1.0, 1.3);

        // Then translate is zero (but still present since direction includes pan).
        let (tx_start, tx_end) = kf.translate_x.unwrap();
        assert_eq!(tx_start, 0.0);
        assert_eq!(tx_end, 0.0);
    }
}
