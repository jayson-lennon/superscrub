use ss_render::{clamp_time_range, range_frame_count};

#[test]
fn default_range_is_full_duration() {
    let (start, end) = clamp_time_range(None, None, 30.0);
    assert_eq!(start, 0.0);
    assert_eq!(end, 30.0);
}

#[test]
fn custom_start_end_clamped() {
    let (start, end) = clamp_time_range(Some(-5.0), Some(100.0), 30.0);
    assert_eq!(start, 0.0);
    assert_eq!(end, 30.0);
}

#[test]
fn start_greater_than_end_clamped() {
    let (start, end) = clamp_time_range(Some(20.0), Some(10.0), 30.0);
    // end is clamped to max(start, end), so both become 20.0
    assert_eq!(start, 20.0);
    assert_eq!(end, 20.0);
}

#[test]
fn frame_count_for_full_range() {
    // 10 seconds at 30 fps = 300 frames.
    assert_eq!(range_frame_count(0.0, 10.0, 30), 300);
}

#[test]
fn frame_count_for_sub_range() {
    // 5 seconds at 30 fps = 150 frames.
    assert_eq!(range_frame_count(5.0, 10.0, 30), 150);
}

#[test]
fn frame_count_zero_when_start_equals_end() {
    assert_eq!(range_frame_count(5.0, 5.0, 30), 0);
}

#[test]
fn frame_count_zero_when_start_after_end() {
    assert_eq!(range_frame_count(10.0, 5.0, 30), 0);
}

#[test]
fn frame_count_zero_when_fps_zero() {
    assert_eq!(range_frame_count(0.0, 10.0, 0), 0);
}
