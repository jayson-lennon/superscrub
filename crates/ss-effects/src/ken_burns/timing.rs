//! Segment timing math for the Ken Burns stack-based pop model.
//!
//! All clips start visible, stacked by z-index. The topmost clip holds,
//! then fades 100%→0%, revealing the clip below. This repeats for each layer.
//!
//! ```text
//! Clip 2 (z=2):  |--hold--|--fade 100→0--|                          (gone)
//! Clip 1 (z=1):  |----hold----|----------|----hold----|--fade 100→0--|  (gone)
//! Clip 0 (z=0):  |----hold----|----------|----------|------hold------|--fade 100→0--|
//! ```

/// When a clip fades in the stack-based pop model.
///
/// Each clip holds at full opacity from t=0 until `fade_start`, then fades
/// to zero opacity by `fade_end`.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentTiming {
    /// Time when this clip begins fading from 1.0 → 0.0.
    pub fade_start: f64,
    /// Time when this clip reaches 0.0 opacity.
    pub fade_end: f64,
}

/// Computes fade timing for each segment in the stack.
///
/// Segments are ordered top-to-bottom: segment 0 is the top of the stack
/// (highest z-index, fades first).
///
/// For segment `i`:
/// - `fade_start[i] = sum(hold[0..=i]) + sum(fade[0..i])`
/// - `fade_end[i] = fade_start[i] + fade[i]`
///
/// # Panics
///
/// Panics if `hold_durations` and `fade_durations` have different lengths.
pub fn compute_segment_timings(
    hold_durations: &[f64],
    fade_durations: &[f64],
) -> Vec<SegmentTiming> {
    assert_eq!(
        hold_durations.len(),
        fade_durations.len(),
        "hold_durations and fade_durations must have the same length"
    );

    let mut timings = Vec::with_capacity(hold_durations.len());
    let mut accumulated = 0.0;

    for i in 0..hold_durations.len() {
        let fade_start = accumulated + hold_durations[i];
        let fade_end = fade_start + fade_durations[i];
        timings.push(SegmentTiming {
            fade_start,
            fade_end,
        });
        accumulated = fade_end;
    }

    timings
}

/// Returns the total time needed for all segments (sum of all holds + fades).
pub fn total_segment_time(hold_durations: &[f64], fade_durations: &[f64]) -> f64 {
    assert_eq!(
        hold_durations.len(),
        fade_durations.len(),
        "hold_durations and fade_durations must have the same length"
    );
    hold_durations.iter().sum::<f64>() + fade_durations.iter().sum::<f64>()
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn single_segment_timing() {
        // Given one segment with hold=5.0, fade=2.0.
        let holds = [5.0];
        let fades = [2.0];

        // When computing timings.
        let timings = compute_segment_timings(&holds, &fades);

        // Then fade starts after hold, ends after hold+fade.
        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].fade_start, 5.0);
        assert_eq!(timings[0].fade_end, 7.0);
    }

    #[test]
    fn three_equal_segments() {
        // Given three segments with equal hold=3.0, fade=1.0.
        let holds = [3.0, 3.0, 3.0];
        let fades = [1.0, 1.0, 1.0];

        // When computing timings.
        let timings = compute_segment_timings(&holds, &fades);

        // Then timings cascade: each segment's fade starts after the previous ends + hold.
        assert_eq!(timings.len(), 3);

        // Segment 0: hold 0→3, fade 3→4
        assert_eq!(timings[0].fade_start, 3.0);
        assert_eq!(timings[0].fade_end, 4.0);

        // Segment 1: hold 0→4, then hold 4→7, fade 7→8
        assert_eq!(timings[1].fade_start, 7.0);
        assert_eq!(timings[1].fade_end, 8.0);

        // Segment 2: hold 0→4, then 4→8, then hold 8→11, fade 11→12
        assert_eq!(timings[2].fade_start, 11.0);
        assert_eq!(timings[2].fade_end, 12.0);
    }

    #[test]
    fn per_segment_different_durations() {
        // Given segments with different hold/fade durations.
        let holds = [2.0, 5.0, 3.0];
        let fades = [1.0, 2.0, 1.5];

        // When computing timings.
        let timings = compute_segment_timings(&holds, &fades);

        // Then each segment's timing is independent.
        assert_eq!(timings.len(), 3);

        // Segment 0: fade 2→3
        assert_eq!(timings[0].fade_start, 2.0);
        assert_eq!(timings[0].fade_end, 3.0);

        // Segment 1: fade (3+5)→(8+2) = 8→10
        assert_eq!(timings[1].fade_start, 8.0);
        assert_eq!(timings[1].fade_end, 10.0);

        // Segment 2: fade (10+3)→(13+1.5) = 13→14.5
        assert_eq!(timings[2].fade_start, 13.0);
        assert_eq!(timings[2].fade_end, 14.5);
    }

    #[test]
    fn total_segment_time_matches_sum() {
        // Given three segments.
        let holds = [2.0, 5.0, 3.0];
        let fades = [1.0, 2.0, 1.5];

        // When computing total time.
        let total = total_segment_time(&holds, &fades);

        // Then it equals the sum of all holds and fades.
        assert_eq!(total, 14.5);
    }

    #[test]
    fn empty_segments_returns_empty_vec() {
        // Given no segments.
        let holds: [f64; 0] = [];
        let fades: [f64; 0] = [];

        // When computing timings.
        let timings = compute_segment_timings(&holds, &fades);

        // Then result is empty.
        assert!(timings.is_empty());
    }

    #[test]
    fn empty_segments_total_time_is_zero() {
        // Given no segments.
        let holds: [f64; 0] = [];
        let fades: [f64; 0] = [];

        // When computing total time.
        let total = total_segment_time(&holds, &fades);

        // Then it is zero.
        assert_eq!(total, 0.0);
    }
}
