use ss_render::{ProgressTracker, RenderPhase};

#[test]
fn new_tracker_has_zero_rendered() {
    // Given a new tracker with 100 total frames.
    let tracker = ProgressTracker::new(100);

    // Then no frames have been rendered.
    let snap = tracker.snapshot();
    assert_eq!(snap.frames_rendered, 0);
    assert_eq!(snap.total_frames, 100);
    assert_eq!(snap.phase, RenderPhase::Rendering);
}

#[test]
fn update_increments_rendered_count() {
    // Given a tracker.
    let tracker = ProgressTracker::new(100);

    // When updating progress.
    tracker.update(42, 4.2);

    // Then the snapshot reflects the update.
    let snap = tracker.snapshot();
    assert_eq!(snap.frames_rendered, 42);
    assert_eq!(snap.current_time, 4.2);
}

#[test]
fn set_phase_changes_phase() {
    let tracker = ProgressTracker::new(100);

    tracker.set_phase(RenderPhase::MuxingAudio);
    assert_eq!(tracker.snapshot().phase, RenderPhase::MuxingAudio);

    tracker.set_phase(RenderPhase::Complete);
    assert_eq!(tracker.snapshot().phase, RenderPhase::Complete);
}

#[test]
fn fraction_computes_correctly() {
    let tracker = ProgressTracker::new(200);

    tracker.update(50, 1.0);
    assert!((tracker.snapshot().fraction() - 0.25).abs() < 0.001);
}

#[test]
fn fraction_is_zero_when_no_frames() {
    let tracker = ProgressTracker::new(0);
    assert_eq!(tracker.snapshot().fraction(), 0.0);
}

#[test]
fn is_complete_when_phase_is_complete() {
    let tracker = ProgressTracker::new(10);

    assert!(!tracker.snapshot().is_complete());

    tracker.set_phase(RenderPhase::Complete);
    assert!(tracker.snapshot().is_complete());
}

#[test]
fn snapshot_is_a_copy() {
    // Given a tracker.
    let tracker = ProgressTracker::new(100);

    // When taking a snapshot and then updating.
    let snap1 = tracker.snapshot();
    tracker.update(50, 5.0);

    // Then the first snapshot is unchanged.
    assert_eq!(snap1.frames_rendered, 0);
    assert_eq!(tracker.snapshot().frames_rendered, 50);
}

#[test]
fn cancelled_phase_is_not_complete() {
    let tracker = ProgressTracker::new(10);
    tracker.set_phase(RenderPhase::Cancelled);
    assert!(!tracker.snapshot().is_complete());
}
