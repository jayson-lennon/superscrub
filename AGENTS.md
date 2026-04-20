# Style Guide

This document defines the coding conventions and architectural patterns for the SuperScrub codebase.

## 1. Overview

SuperScrub is a programmatic video editor built in Rust with an egui-based GUI. The workspace is organized into six crates under `crates/`:

- **ss-core** — Data model, project config parsing, animation interpolation. Pure data + algorithms, no I/O.
- **ss-compositor** — Frame rendering (image loading, transforms, compositing).
- **ss-audio** — Audio engine (playback, seek, pause via rodio).
- **ss-preview** — Preview cache (pre-renders frames in background via rayon).
- **ss-render** — Headless renderer (ffmpeg pipe, audio muxing, progress tracking).
- **ss-editor** — egui application (viewport, timeline, transport controls, settings).

## 2. Core Patterns

### Error Handling

Use `wherror::Error` with `error_stack::Report` for all fallible operations.

```rust
use wherror::Error;

#[derive(Debug, Error)]
#[error("failed to load image")]
pub struct ImageLoadError;
```

```rust
use error_stack::{Report, ResultExt};

fn load(path: &Path) -> Result<RgbaImage, Report<ImageLoadError>> {
    image::open(path)
        .change_context(ImageLoadError)
        .attach(format!("image path: {}", path.display()))?
}
```

- Document errors with `# Errors` doc sections on all fallible public functions.
- NEVER make an `errors.rs` file. ALWAYS colocate error types near the trait or function that produces them.
- NEVER make a `traits.rs` file. Use separate modules for each trait and its implementations.

### Trait Usage

Every external dependency or service must have a trait abstraction. All traits must be `Send + Sync` and include a `name(&self) -> &'static str` method for debugging.

```rust
pub trait AudioEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn load(&self, path: &Path) -> Result<(), Report<AudioError>>;
    fn play(&self) -> Result<(), Report<AudioError>>;
}
```

**Service wrapper pattern** — wrap `Arc<dyn Trait>` in a typed service struct:

```rust
use std::sync::Arc;
use derive_more::Debug;

#[derive(Debug, Clone)]
pub struct AudioEngineService {
    #[debug("AudioEngine<{}>", self.backend.name())]
    svc: Arc<dyn AudioEngine>,
}

impl AudioEngineService {
    pub fn new(svc: Arc<dyn AudioEngine>) -> Self {
        Self { svc }
    }
}
```

Import `derive_more::Debug` in the module, then use `#[debug(...)]` on fields that can't automatically derive `Debug`.

### Module Structure

Each crate uses a flat module layout under `src/`. A typical service crate looks like:

```
src/
  lib.rs          # Module declarations and re-exports
  engine.rs       # Trait + error (parent module, declares submodules)
  engine/
    service.rs    # Service wrapper
    rodio.rs      # Real implementation
    fake.rs       # Test fake
```

Trait + error live in the parent `.rs` file. Real impl, fake, and service each get their own file in the subdirectory.

### Dependency Injection

The `ss-editor` crate wires all services together in a `Services` container. Application code depends on this container, not on concrete backends.

```rust
#[derive(Debug, Clone)]
pub struct Services {
    pub audio: AudioEngineService,
    pub preview: PreviewCacheService,
    pub renderer: FrameRendererService,
}
```

Each field must be a dedicated struct or facade that wraps `Arc` internally (or is trivially copied). Never expose raw `Arc<dyn Trait>` as a field — wrap it in a typed service struct. The `Services` struct is shared between threads and tasks.

```rust
// DO NOT
pub struct Services {
    pub audio: Arc<dyn AudioEngine>,  // raw trait object
}

// DO
pub struct Services {
    pub audio: AudioEngineService,   // dedicated struct, Arc is internal
}
```

Library crates (ss-compositor, ss-audio, ss-preview) expose their own service wrapper but do not have a `Services` container — they get wired into the editor at the application layer.

## 3. Data Flow

The editor flow: `EditorApp` (egui) → `Services` → service wrappers → trait backends.

The render flow: `RenderJob` orchestrates `FrameRendererService` + `FrameEncoder` to produce video output.

## 4. Tests

- Tests should only verify **observable behavior**. Testing internal details is an anti-pattern.
- If observable behavior cannot be tested, an abstraction needs to be created. Ask the user how to proceed in this case.

### BDD-Style Tests (Given/When/Then)

Structure tests with clear Given/When/Then comments. Each section should contain actual code — do not bury test inputs inside the comment:

```rust
// DO NOT — inputs are only in the comment, not in code
fn time_to_frame_index_at_one_second() {
    // Given time=1.0, fps=30, dur=10.0.
    // When converting to frame index.
    // Then the result is 30.
    assert_eq!(time_to_frame_index(1.0, 30, 10.0), Some(30));
}
```

```rust
// DO — each section has a corresponding code line
fn time_to_frame_index_at_one_second() {
    // Given time=1.0, fps=30, dur=10.0.
    let (time, fps, dur) = (1.0, 30, 10.0);

    // When converting to frame index.
    let result = time_to_frame_index(time, fps, dur);

    // Then the result is 30.
    assert_eq!(result, Some(30));
}
```

Do not test default values — that is testing the struct definition, not behavior:

```rust
// DO NOT — testing default values is irrelevant
fn default_window_size() {
    let config = AppConfig::default();
    assert_eq!(config.window_size, [1280, 720]);
}
```

```rust
// DO NOT — no BDD structure, inputs not separated from assertion
fn preview_resolution_full_res() {
    let mut config = AppConfig::default();
    config.preview_divisor = 1;
    let res = config.preview_resolution([1920, 1080]);
    assert_eq!(res, (1920, 1080));
}
```

```rust
// DO — tests meaningful behavior with proper BDD structure
fn preview_resolution_correctly_calculates_with_divisor_of_one() {
    // Given a configuration having a preview_divisor of 1.
    let mut config = AppConfig::default();
    config.preview_divisor = 1;

    // When we calculate the preview resolution.
    let res = config.preview_resolution([1920, 1080]);

    // Then the resolution is the same.
    assert_eq!(res, (1920, 1080));
}
```

### Parameterized Tests with rstest

Use `rstest` as-needed for parameterized tests:

```rust
#[rstest::rstest]
#[case(Easing::Linear, 0.0, 0.0)]
#[case(Easing::Linear, 1.0, 1.0)]
fn apply_easing_linear(#[case] easing: Easing, #[case] t: f64, #[case] expected: f64) {
    assert_eq!(apply_easing(easing, t), expected);
}
```

### Test Utilities

Integration tests live in `tests/` with a `test_utils/` subdirectory:

```
tests/
  playback_controller.rs
  services.rs
  test_utils/
    mod.rs        # Re-exports
    fakes.rs      # Fake implementations shared across tests
    fixtures.rs   # Builder helpers for constructing test data
```

### Fake Implementations

Fakes colocate with the trait they implement (in the library crate). Use atomic counters for call tracking and `Mutex` for stateful behavior:

```rust
pub struct FakeAudioEngine {
    pub load_count: AtomicUsize,
    pub play_count: AtomicUsize,
    state: Mutex<FakeState>,
}

impl AudioEngine for FakeAudioEngine {
    fn name(&self) -> &'static str { "fake" }

    fn load(&self, path: &Path) -> Result<(), Report<AudioError>> {
        self.load_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
```

## 5. Documentation

- Module-level `//!` docs on every module.
- `///` doc comments on all public types and functions.
- `# Errors` sections on all fallible public functions.

## 6. Modification Guide

When implementing features:

1. **Find related patterns** — Look at existing modules in the relevant crate for similar features.
2. **Create trait first** — Define the abstraction in its own module before implementing.
3. **Implement real and fake** — Both must satisfy the trait.
4. **Wire into Services** — If the crate has a service container, add the new service.
5. **Write tests** — Use Given/When/Then with fakes. Test observable behavior only.
6. **Add documentation** — Module docs, type docs, error docs.

## 7. Tooling

Read the `justfile` to determine available commands. Prioritize running commands from the `justfile` instead of manual invocation. Key commands:

- `just test` — run all tests (nextest + doc tests)
- `just check` — workspace check
- `just clippy` — lint
- `just fmt` — format
- `just coverage` — generate coverage report

## 8. Misc

- NEVER manually split a string using `.chars` or by indexing. Use the `unicode-segmentation` crate.
- Avoid trivial setters for struct methods. Prefer meaningful semantic actions.
