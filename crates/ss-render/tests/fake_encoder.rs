use image::{Rgba, RgbaImage};
use ss_render::{FakeFrameEncoder, FrameEncoder};

#[test]
fn name_returns_fake() {
    let encoder = FakeFrameEncoder::new();
    assert_eq!(encoder.name(), "fake");
}

#[test]
fn send_frame_stores_frame() {
    // Given a fake encoder.
    let encoder = FakeFrameEncoder::new();

    // When sending a frame.
    let frame = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
    encoder.send_frame(&frame).unwrap();

    // Then the frame count is 1.
    assert_eq!(encoder.frame_count(), 1);
}

#[test]
fn frame_count_matches_sent() {
    let encoder = FakeFrameEncoder::new();
    let frame = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));

    encoder.send_frame(&frame).unwrap();
    encoder.send_frame(&frame).unwrap();
    encoder.send_frame(&frame).unwrap();

    assert_eq!(encoder.frame_count(), 3);
}

#[test]
fn finish_sets_finished_flag() {
    let encoder = FakeFrameEncoder::new();

    assert!(!encoder.is_finished());
    encoder.finish().unwrap();
    assert!(encoder.is_finished());
}

#[test]
fn finish_call_count_increments() {
    let encoder = FakeFrameEncoder::new();

    assert_eq!(encoder.finish_call_count(), 0);
    encoder.finish().unwrap();
    assert_eq!(encoder.finish_call_count(), 1);
}
