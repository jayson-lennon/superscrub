//! Integration tests for the audio mixing pipeline.
//!
//! Tests exercise the mixer, WAV writer, and audio_mixer orchestration
//! without requiring ffmpeg or actual audio files.

use ss_core::project::{AudioClipDef, EncodingConfig, Project};
use ss_render::mixer::{MixedClip, mix_clips};

mod test_utils;

fn make_project_with_audio_clips(clips: Vec<AudioClipDef>) -> Project {
    Project {
        resolution: [100, 50],
        fps: 10,
        duration: std::time::Duration::from_secs_f64(2.0),
        output: "out.mp4".into(),
        background: [0, 0, 0, 255],
        audio_clips: clips,
        encoding: EncodingConfig::default(),
        clips: vec![],
    }
}

// ================================================================
// render_audio tests
// ================================================================

#[test]
fn render_audio_with_no_clips_returns_error() {
    // Given a project with no audio clips.
    let project = make_project_with_audio_clips(vec![]);
    let dir = tempfile::tempdir().unwrap();

    // When calling render_audio.
    let result = ss_render::render_audio(
        &project,
        std::path::Path::new("/test/project.json"),
        dir.path(),
        0.0,
        2.0,
    );

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn render_audio_with_nonexistent_file_returns_error() {
    // Given a project with a clip referencing a nonexistent file.
    let project = make_project_with_audio_clips(vec![AudioClipDef {
        id: "missing".into(),
        path: "nonexistent.wav".into(),
        track: 0,
        start_time: 0.0,
        end_time: 2.0,
        volume: 1.0,
    }]);
    let dir = tempfile::tempdir().unwrap();

    // When calling render_audio.
    let result = ss_render::render_audio(
        &project,
        std::path::Path::new("/test/project.json"),
        dir.path(),
        0.0,
        2.0,
    );

    // Then an error is returned.
    assert!(result.is_err());
}

#[test]
fn mix_clips_produces_correct_duration() {
    // Given clips mixed for a 10-second duration at 44100 Hz stereo.
    let clip = MixedClip {
        samples: vec![0.5; 44100 * 2], // 1 second of stereo audio
        channels: 2,
        sample_rate: 44100,
        start_time: 0.0,
        volume: 1.0,
    };
    let duration = std::time::Duration::from_secs_f64(10.0);

    // When mixing.
    let output = mix_clips(&[clip], 44100, 2, duration);

    // Then the output has exactly 10 seconds worth of samples.
    let expected_len = (10.0 * 44100.0 * 2.0) as usize;
    assert_eq!(output.len(), expected_len);
}
