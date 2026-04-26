# Phase 1 Detailed Plan: fast-image-resize for Scale-Only Clips

## Problem

The warp phase is **220ms** (79% of total frame time). All 20 clips in the queens test project
are Ken Burns effects — scale + translate only, zero rotation. The current path uses
`imageproc::warp_into` with `Interpolation::Bilinear`, which is a general-purpose affine warper:

- Per-pixel inverse projection (f64 matrix multiply)
- 4 pixel reads per bilinear sample through `Pixel::map2` (generic trait dispatch)
- Bounds-checked `get_pixel` calls
- No SIMD — pure scalar f32

For scale+translate transforms, the forward matrix is diagonal:
```
[[sx,  0, tx],
 [ 0, sy, ty],
 [ 0,  0,  1]]
```

This is just a resize + offset — no need for general affine warping.

## Solution

Use `fast_image_resize` (v5) for scale+translate clips. It provides:
- SIMD-accelerated resizing (SSE4.1/AVX2 on x86, NEON on ARM)
- Native `RgbaImage` support via `IntoImageView`/`IntoImageViewMut` traits (no data conversion)
- Crop box for extracting only the visible source region
- Premultiplied alpha handling during resize (higher quality than current straight-alpha bilinear)
- Bilinear, Bicubic, Lanczos3, and other filter types

**Expected result:** warp 220ms → ~25ms, total frame ~85ms.

---

## Transform Classification

### How to detect scale+translate

When `resolved.rotation` is effectively zero, the transform pipeline collapses:

```
Scale(sx, sy) → Translate(px, py) → Translate(-pivot_x, -pivot_y)
→ Rotate(0) [identity] → Scale(asx, asy) → Translate(pivot_x, pivot_y)
→ Translate(anim_tx, anim_ty) → Translate(cam_x, cam_y)
```

All these are scale or translate — the result is always `Scale + Translate`.

**Robust detection:** check the off-diagonal elements of the forward matrix directly:
- `forward[1]` (the xy element) ≈ 0
- `forward[3]` (the yx element) ≈ 0
- `forward[0]` > 0 and `forward[4]` > 0 (positive scales — no flip)

If all four conditions hold, the transform is pure scale+translate.

### Why check the matrix instead of `rotation == 0`?

Checking the matrix is more robust:
- Works regardless of how the matrix was constructed
- Catches edge cases (e.g., rotation that's a multiple of 2π but represented as non-zero due to interpolation)
- No need to reason about the transform pipeline

### `TransformClass` enum

```rust
/// The class of an affine transform, used to select the warp implementation.
enum TransformClass {
    /// Scale + translate only (off-diagonal elements are zero).
    /// fast-image-resize can handle this directly.
    ScaleTranslate,
    /// General affine (rotation, skew, or negative scale).
    /// Falls through to imageproc::warp_into.
    General,
}
```

### Detection function

```rust
fn classify_transform(forward: &[f32; 9]) -> TransformClass {
    const EPSILON: f32 = 1e-4;
    let off_diag_zero = forward[1].abs() < EPSILON && forward[3].abs() < EPSILON;
    let positive_scale = forward[0] > 0.0 && forward[4] > 0.0;
    if off_diag_zero && positive_scale {
        TransformClass::ScaleTranslate
    } else {
        TransformClass::General
    }
}
```

---

## Resize Math

Given a scale+translate forward matrix:
```
forward[0] = sx   forward[2] = tx
forward[4] = sy   forward[5] = ty
```

The AABB is already computed by `transformed_aabb()`. It gives us the visible portion
of the transformed image clamped to canvas bounds: `(aabb_x, aabb_y, aabb_w, aabb_h)`.

For each output pixel `(ox, oy)` in the AABB buffer:
- Canvas position: `(aabb_x + ox, aabb_y + oy)`
- Source position: `((aabb_x + ox - tx) / sx, (aabb_y + oy - ty) / sy)`

So the source crop box that maps to the visible AABB is:
```
crop_left   = (aabb_x as f64 - tx as f64) / sx as f64
crop_top    = (aabb_y as f64 - ty as f64) / sy as f64
crop_width  = aabb_w as f64 / sx as f64
crop_height = aabb_h as f64 / sy as f64
```

The destination size is `(aabb_w, aabb_h)`, and the offset is `(aabb_x, aabb_y)`.

This means we resize from `source[crop]` → `output[aabb_w × aabb_h]`, placing the result
at `(aabb_x, aabb_y)` on the canvas.

### Example: ken_burns_0 at t=0

```
forward = [[1.2, 0, -192], [0, 1.2, -108], [0, 0, 1]]
source = 1920×1080
aabb = (0, 0, 1920, 1080)  // full canvas

crop_left   = (0 - (-192)) / 1.2 = 160
crop_top    = (0 - (-108)) / 1.2 = 90
crop_width  = 1920 / 1.2 = 1600
crop_height = 1080 / 1.2 = 900

// Resize source[160..1760, 90..990] → 1920×1080
// Scale factor: 1920/1600 = 1.2 (upscale within the cropped region)
```

---

## Implementation Steps

### Step 1: Add dependency

**File: `Cargo.toml` (workspace)**
```toml
fast_image_resize = { version = "5", features = ["image"] }
```

**File: `crates/ss-compositor/Cargo.toml`**
```toml
fast_image_resize = { workspace = true }
```

The `image` feature enables `IntoImageView`/`IntoImageViewMut` impls for `RgbaImage`,
so no data conversion is needed — `&RgbaImage` and `&mut RgbaImage` are passed directly.

### Step 2: Add `TransformClass` and detection

**File: `crates/ss-compositor/src/rendering/compositor.rs`**

Add the enum and classification function near the `Affine3x3` struct:

```rust
/// The class of an affine transform, used to select the optimal warp implementation.
enum TransformClass {
    /// Scale + translate only (off-diagonal elements are zero, positive scales).
    /// Can use fast-image-resize instead of general affine warp.
    ScaleTranslate,
    /// General affine (rotation, skew, or negative scale).
    /// Falls through to imageproc::warp_into.
    General,
}

/// Classify the forward transform by inspecting its matrix elements.
///
/// When rotation is zero and no negative scaling is applied, the off-diagonal
/// elements of the 2×2 sub-matrix are zero, indicating a pure scale + translate.
fn classify_transform(forward: &[f32; 9]) -> TransformClass {
    const EPSILON: f32 = 1e-4;
    let off_diag_zero = forward[1].abs() < EPSILON && forward[3].abs() < EPSILON;
    let positive_scale = forward[0] > 0.0 && forward[4] > 0.0;
    if off_diag_zero && positive_scale {
        TransformClass::ScaleTranslate
    } else {
        TransformClass::General
    }
}
```

### Step 3: Add `warp_resize` function

**File: `crates/ss-compositor/src/rendering/compositor.rs`**

Add a new function that performs the fast resize path:

```rust
/// Warp a source image using fast SIMD resize for scale+translate transforms.
///
/// Extracts the visible source region (crop box) and resizes it to the AABB dimensions
/// using `fast_image_resize`, which uses SIMD-accelerated bilinear filtering.
///
/// # Arguments
///
/// * `source` - Source image.
/// * `forward` - Forward transform matrix (must be scale+translate).
/// * `aabb` - The `(x, y, w, h)` bounding box clamped to canvas.
///
/// # Panics
///
/// Panics if the forward matrix is not scale+translate (off-diagonal non-zero).
fn warp_resize(
    source: &RgbaImage,
    forward: &[f32; 9],
    (aabb_x, aabb_y, aabb_w, aabb_h): (u32, u32, u32, u32),
) -> WarpedClip {
    let sx = forward[0] as f64;
    let sy = forward[4] as f64;
    let tx = forward[2] as f64;
    let ty = forward[5] as f64;

    // Compute the source crop box: which source pixels map to the visible AABB.
    let crop_left = (aabb_x as f64 - tx) / sx;
    let crop_top = (aabb_y as f64 - ty) / sy;
    let crop_width = aabb_w as f64 / sx;
    let crop_height = aabb_h as f64 / sy;

    let mut output = RgbaImage::new(aabb_w, aabb_h);

    let mut resizer = fast_image_resize::Resizer::new();
    let options = fast_image_resize::ResizeOptions::new()
        .resize_alg(fast_image_resize::ResizeAlg::Convolution(
            fast_image_resize::FilterType::Bilinear,
        ))
        .crop(crop_left, crop_top, crop_width, crop_height);

    // RgbaImage implements IntoImageView/IntoImageViewMut via fast_image_resize's
    // "image" feature — no data conversion needed.
    resizer
        .resize(source, &mut output, &options)
        .expect("resize parameters are derived from valid AABB; pixel types match");

    WarpedClip {
        buffer: output,
        offset: (aabb_x, aabb_y),
    }
}
```

### Step 4: Dispatch in `warp_clip`

**File: `crates/ss-compositor/src/rendering/compositor.rs`**

Modify `warp_clip` to classify the transform and dispatch:

```rust
fn warp_clip(...) -> WarpedClip {
    // ... existing matrix and AABB computation (unchanged) ...

    if aabb_w == 0 || aabb_h == 0 {
        // ... existing early return (unchanged) ...
    }

    info!(...); // existing tracing log

    // Dispatch based on transform class.
    let mut output = match classify_transform(&forward.0) {
        TransformClass::ScaleTranslate => {
            warp_resize(source, &forward.0, (aabb_x, aabb_y, aabb_w, aabb_h)).buffer
        }
        TransformClass::General => {
            // Existing imageproc::warp_into path for rotated/skewed clips.
            let adjusted = Affine3x3::translate(-(aabb_x as f32), -(aabb_y as f32)).then(forward);
            let projection = Projection::from_matrix(adjusted.0)
                .expect("transform matrix is invertible");
            let mut buf = RgbaImage::from_pixel(aabb_w, aabb_h, Rgba([0, 0, 0, 0]));
            warp_into(source, &projection, Interpolation::Bilinear, Rgba([0, 0, 0, 0]), &mut buf);
            buf
        }
    };

    // Apply opacity (unchanged — applies to both paths).
    if resolved.opacity < 1.0 {
        for pixel in output.pixels_mut() {
            pixel.0[3] = (pixel.0[3] as f32 * resolved.opacity) as u8;
        }
    }

    WarpedClip {
        buffer: output,
        offset: (aabb_x, aabb_y),
    }
}
```

### Step 5: Add import for `fast_image_resize`

**File: `crates/ss-compositor/src/rendering/compositor.rs`**

No module-level import needed if using fully-qualified paths (`fast_image_resize::Resizer::new()`).
Alternatively, add a `use` at the top:

```rust
use fast_image_resize::{self, FilterType, ResizeAlg, ResizeOptions, Resizer};
```

Prefer fully-qualified in the function body to keep imports minimal, since this is the
only file that uses `fast_image_resize`.

### Step 6: Add tracing log for transform class

Add the classification result to the existing `info!` log so we can verify the fast path
is being hit:

```rust
info!(
    aabb = %format!("{aabb_w}x{aabb_h}+{aabb_x}+{aabb_y}"),
    source = %format!("{}x{}", source.width(), source.height()),
    fast_path = matches!(classify_transform(&forward.0), TransformClass::ScaleTranslate),
    "warping clip"
);
```

---

## Testing Plan

### Unit tests for `classify_transform`

```rust
#[test]
fn classify_identity_is_scale_translate() {
    // Given an identity matrix.
    let matrix = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    // When classifying.
    assert!(matches!(classify_transform(&matrix), TransformClass::ScaleTranslate));
}

#[test]
fn classify_rotation_is_general() {
    // Given a 45-degree rotation matrix.
    let (s, c) = std::f32::consts::FRAC_PI_4.sin_cos();
    let matrix = [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0];
    // When classifying.
    assert!(matches!(classify_transform(&matrix), TransformClass::General));
}

#[test]
fn classify_scale_translate_is_scale_translate() {
    // Given a scale(2, 3) + translate(100, 200) matrix.
    let matrix = [2.0, 0.0, 100.0, 0.0, 3.0, 200.0, 0.0, 0.0, 1.0];
    // When classifying.
    assert!(matches!(classify_transform(&matrix), TransformClass::ScaleTranslate));
}

#[test]
fn classify_negative_scale_is_general() {
    // Given a negative scale matrix (flip).
    let matrix = [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    // When classifying.
    assert!(matches!(classify_transform(&matrix), TransformClass::General));
}
```

### Integration test for `warp_resize` correctness

Add a test that verifies the fast path produces visually equivalent output to the
general `warp_into` path for a scale+translate-only transform:

```rust
#[test]
fn warp_resize_matches_warp_into_for_scale_translate() {
    // Given a 100×100 source image with a gradient pattern.
    let mut source = RgbaImage::new(100, 100);
    for y in 0..100 {
        for x in 0..100 {
            source.put_pixel(x, y, Rgba([x as u8, y as u8, 128, 255]));
        }
    }

    // And a scale(2.0, 2.0) + translate(10, 20) transform.
    let forward = [2.0f32, 0.0, 10.0, 0.0, 2.0, 20.0, 0.0, 0.0, 1.0];
    let aabb = (10u32, 20u32, 150u32, 150u32); // visible portion

    // When using the fast resize path.
    let fast_result = warp_resize(&source, &forward, aabb);

    // And using the general warp_into path.
    let adjusted = Affine3x3::translate(-(aabb.0 as f32), -(aabb.1 as f32))
        .then(Affine3x3([forward]));
    let projection = Projection::from_matrix(adjusted.0).unwrap();
    let mut general_buf = RgbaImage::from_pixel(aabb.2, aabb.3, Rgba([0, 0, 0, 0]));
    warp_into(&source, &projection, Interpolation::Bilinear, Rgba([0, 0, 0, 0]), &mut general_buf);

    // Then the outputs should be very similar.
    // Allow small differences due to different bilinear implementations.
    let fast_raw = fast_result.buffer.as_raw();
    let general_raw = general_buf.as_raw();
    let mut max_diff = 0u8;
    for (a, b) in fast_raw.iter().zip(general_raw.iter()) {
        max_diff = max_diff.max(a.abs_diff(*b));
    }
    // Bilinear implementations may differ by a few values.
    assert!(max_diff <= 5, "max pixel difference: {max_diff}");
}
```

### Existing tests must pass

- `just test` — all 511+ tests must pass
- `just clippy` — clean
- `just check` — clean

### Manual verification

Run the queens project render and verify:
1. All clips hit the fast path (`fast_path=true` in tracing logs)
2. Output video is visually correct (no artifacts, no missing regions)
3. Timing shows warp phase significantly faster

---

## Risks and Mitigations

### Risk: Output quality difference

`fast_image_resize` uses premultiplied alpha during bilinear interpolation (it multiplies
alpha before blending, then divides back). The current `imageproc::warp_into` uses straight
alpha. This means edge pixels near transparent regions may differ slightly.

**Mitigation:** For fully opaque clips (alpha=255 everywhere), the results are identical.
For semi-transparent clips, the premultiplied alpha result is actually *more correct*
(no dark fringing). The integration test with a tolerance of ±5 per channel accounts for
this difference.

### Risk: Crop box edge cases

If the crop box extends beyond the source image bounds (shouldn't happen if AABB computation
is correct, but could occur due to floating-point precision):

**Mitigation:** `fast_image_resize` handles out-of-bounds crop boxes by clamping. Verify
with the `ken_burns_0` example above that crop values are always within `[0, src_w] × [0, src_h]`.

### Risk: Zero-size output

If `aabb_w` or `aabb_h` is 0, we skip (existing early return). But what if the crop box
results in 0 width/height due to floating-point? The `Resizer::resize` call would return
an error.

**Mitigation:** The early return for `aabb_w == 0 || aabb_h == 0` already handles this.
The crop box is derived from the AABB, so if AABB is non-zero, the crop is non-zero.

### Risk: Thread safety

`Resizer` is created per-call inside `warp_clip` / `warp_resize`, which runs inside
`par_iter().map(prepare_clip)`. Each call gets its own `Resizer`.

**Mitigation:** No shared mutable state. `Resizer` is not `Send+Sync` but doesn't need
to be — each thread creates its own instance.

---

## Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `fast_image_resize = { version = "5", features = ["image"] }` to workspace deps |
| `crates/ss-compositor/Cargo.toml` | Add `fast_image_resize = { workspace = true }` |
| `crates/ss-compositor/src/rendering/compositor.rs` | Add `TransformClass` enum, `classify_transform()`, `warp_resize()`. Modify `warp_clip()` to dispatch. Add tests. |

---

## Expected Performance

| Metric | Before | After |
|--------|--------|-------|
| Warp (per clip) | ~12ms scalar | ~1.5ms SIMD |
| Warp (20 clips parallel, 12 threads) | ~220ms | ~25ms |
| Composite (already parallel) | ~56ms | ~56ms |
| **Total frame** | **~280ms** | **~85ms** |

The 9× speedup on warp comes from:
- SIMD bilinear interpolation (SSE4.1/AVX2 processes 4–8 pixels per cycle)
- No per-pixel inverse projection math (just direct crop + resize)
- No generic trait dispatch overhead
- No bounds checking per pixel
