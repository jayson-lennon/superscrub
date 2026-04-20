//! CLI binary for testing audio playback.
//!
//! Usage:
//!   `ss-audio <audio.mp3>`                    — play from start
//!   `ss-audio <audio.mp3> --seek <seconds>`   — seek then play
//!   `ss-audio <audio.mp3> --pause <seconds>`  — play, pause after N seconds
//!
//! Use Ctrl+C to stop at any time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use ss_audio::{AudioEngine, AudioPlaybackState, RodioAudioEngine};

fn main() {
    tracing_subscriber::fmt().init();
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: ss-audio <audio.mp3> [--seek <seconds>] [--pause <seconds>]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --seek <seconds>   Seek to position before playing");
        eprintln!(
            "  --pause <seconds>  Pause playback after N seconds (live demo of pause/resume)"
        );
        std::process::exit(1);
    }

    let audio_path = &args[1];
    let seek_time = parse_optional_arg(&args, "--seek");
    let pause_after = parse_optional_arg(&args, "--pause");

    eprintln!("Creating audio engine...");
    let engine =
        RodioAudioEngine::new().expect("failed to create audio engine (no output device?)");

    eprintln!("Loading: {audio_path}");
    engine
        .load(std::path::Path::new(audio_path))
        .expect("failed to load audio");

    eprintln!("Duration: {:.1}s", engine.duration());
    eprintln!("State:    {:?}", engine.state());

    if let Some(secs) = seek_time {
        eprintln!("Seeking to {secs:.1}s...");
        engine.seek(secs).expect("seek failed");
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc_handler(r);

    eprintln!("Playing...");
    engine.play();
    eprintln!("State:    {:?}", engine.state());

    if let Some(pause_secs) = pause_after {
        thread::sleep(Duration::from_secs_f64(pause_secs));
        eprintln!();
        eprintln!("Pausing at position {:.1}s...", engine.position());
        engine.pause();
        eprintln!("State:    {:?}", engine.state());
        thread::sleep(Duration::from_secs(2));
        eprintln!("Resuming...");
        engine.play();
        eprintln!("State:    {:?}", engine.state());
    }

    // Print position until playback ends or Ctrl+C.
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(200));
        let pos = engine.position();
        let dur = engine.duration();
        let state = engine.state();
        let pct = if dur > 0.0 { pos / dur * 100.0 } else { 0.0 };
        eprint!("\r  position: {pos:6.1}s / {dur:.1}s ({pct:5.1}%) [{state:?}]  ");

        if state == AudioPlaybackState::Paused && pos >= dur - 0.1 {
            eprintln!("\nPlayback complete.");
            break;
        }
    }

    eprintln!();
    engine.pause();
    eprintln!("Final position: {:.1}s", engine.position());
}

fn parse_optional_arg(args: &[String], flag: &str) -> Option<f64> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1)?.parse().ok()
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    ctrlc::set_handler(move || {
        eprintln!("\nStopping...");
        running.store(false, Ordering::Relaxed);
    })
    .ok(); // Ignore if handler can't be set (e.g. no terminal).
}
