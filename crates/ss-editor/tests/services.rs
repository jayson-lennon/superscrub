use std::sync::atomic::Ordering;

use ss_audio::PlaybackState;

mod test_utils;

#[test]
fn services_holds_all_deps() {
    let (services, _, _) = test_utils::fakes::create_test_services();

    // Verify services are accessible.
    assert!(services.audio.state() == PlaybackState::Paused);
    assert_eq!(services.preview.progress().total, 0);
}

#[test]
fn services_audio_delegates() {
    let (services, fake_audio, _) = test_utils::fakes::create_test_services();

    services.audio.play();
    assert_eq!(fake_audio.play_count.load(Ordering::SeqCst), 1);
}
