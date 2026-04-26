# Performance Optimization Plan

## Current State

| Phase | Time | % of Total |
|-------|------|-----------|
| resolve_items | ~20µs | 0% |
| **parallel warp** | **~220ms** | **79%** |
| parallel composite | ~56ms | 20% |
| send_frame | ~4ms | 1% |
| **Total** | **~280ms** | |

Target: reduce total frame time below 100ms for real-time preview at 10+ fps.

## Completed Optimizations

1. ✅ `Arc<RgbaImage>` for ImageProvider — eliminates ~8MB clone per clip per frame
2. ✅ Bbox-scoped warp + composite — allocates only AABB-sized buffers
3. ✅ Zero-opacity clip skip — early return for transparent clips
4. ✅ Parallel clip warping via rayon — `par_iter().map(prepare_clip)` across 12 threads
5. ✅ FilesystemImageProvider `RwLock<HashMap>` for concurrent reads
6. ✅ Row-parallel composite via `par_chunks_mut` — 358ms → 56ms (6.4× speedup)
7. ✅ Tracing with `FmtSpan::CLOSE` and `#[instrument]` on all render pipeline functions

## Key Insight

**All 20 clips in the queens test project are Ken Burns effects — scale + translate only, zero rotation.** The full general affine warp (imageproc `warp_into` with bilinear interpolation) is overkill. The transform pipeline reduces to a pure scale + translate when rotation is zero.

---

## Phase 1: Fast resize path for scale-only clips

**Expected impact:** warp 220ms → ~25ms (9× speedup), total ~85ms

### Problem

`imageproc::warp_into` with `Interpolation::Bilinear` is a general-purpose affine warper:
- Per-pixel inverse projection (f64 matrix multiply)
- 4 pixel reads per bilinear sample through `Pixel::map2` (generic trait dispatch)
- Bounds-checked `get_pixel` calls
- No SIMD — pure scalar f32

For scale+translate-only transforms (Ken Burns), this is ~100× slower than a purpose-built resampler.

### Solution: `fast-image-resize`

The `fast-image-resize` crate provides SIMD-accelerated (SSE4.1/AVX2/NEON) image resizing with:
- Native RGBA support with premultiplied alpha blending
- 5–10× faster than imageproc for resize operations
- Supports different resize filters (Bilinear, Bicubic, Lanczos3, etc.)

### Implementation

#### 1.1 Classify transform type in `warp_clip`

When `resolved.rotation` is effectively zero (within some epsilon, e.g. `rotation.abs() < 1e-4`), the forward matrix reduces to:

```
Scale(source → placement) → Translate(placement position)
→ Scale(around pivot, animation) → Translate(away from pivot)
→ Translate(animation offset) → Translate(camera pan)
```

With zero rotation, `rotate(0)` is identity and the entire chain collapses to a single `Scale + Translate` — a pure resize + offset.

Add a helper to detect this:

```rust
/// The class of an affine transform.
enum TransformClass {
    /// Scale + translate only (no rotation/skew).
    /// Contains the effective scale factors and translation.
    ScaleTranslate {
        scale_x: f32,
        scale_y: f32,
        translate_x: f32,
        translate_y: f32,
    },
    /// General affine (rotation, skew, non-uniform scale + rotation).
    General,
}
```

When the rotation is ~0, compute the effective scale+translate from the forward matrix directly:
- `scale_x = forward[0]`, `scale_y = forward[4]`
- `translate_x = forward[2]`, `translate_y = forward[5]`

(For a pure scale+translate affine, the matrix is `[[sx, 0, tx], [0, sy, ty], [0, 0, 1]]`.)

#### 1.2 Add `fast-image-resize` fast path

In `warp_clip`, when `TransformClass::ScaleTranslate` is detected:

1. Compute the output dimensions as `(source_w × scale_x, source_h × scale_y)`
2. Use `fast-image-resize::Resizer` to resize the source image to those dimensions
3. Place the resized image at the computed offset on the canvas

This bypasses `imageproc::warp_into` entirely for the common case.

The resized image may extend beyond canvas bounds. Clamp to the canvas AABB as before. The output is placed at the clamped offset.

#### 1.3 Keep `warp_into` as fallback for general transforms

When rotation or skew is present, fall through to the existing `warp_into` path. This ensures correctness for all transform types.

#### 1.4 Handle non-uniform scale

Ken Burns clips use uniform scale (scale_x == scale_y), but the general `ScaleTranslate` path must handle non-uniform scale too. `fast-image-resize` supports independent width/height scaling, so this is straightforward.

### Files to modify

- `crates/ss-compositor/Cargo.toml` — add `fast-image-resize` dependency
- `crates/ss-compositor/src/rendering/compositor.rs` — add `TransformClass` enum, detection, and fast path in `warp_clip`

### Risks

- `fast-image-resize` has its own image types — may need conversion from/to `RgbaImage`. Check if it implements `From<ImageBuffer>` or if we need `std::mem::transmute` style tricks.
- Edge quality: bilinear in `fast-image-resize` vs `imageproc` may differ slightly. Run visual comparison on queens project.
- Premultiplied alpha: `fast-image-resize` uses premultiplied alpha by default for RGBA. Our pipeline uses straight alpha. Need to ensure correct conversion.

### Testing

- Add `TransformClass` unit tests for zero rotation, non-zero rotation, pure scale, scale+translate
- Add integration test: given a scale+translate-only clip, verify fast path produces visually equivalent output to the `warp_into` path (allow small floating-point differences)
- Run `just test` (511+ tests) and `just clippy`

---

## Phase 2: SIMD alpha blending in composite

**Expected impact:** composite 56ms → ~15ms, total reduction ~40ms (minor at this point since composite is already 20%)

### Problem

The `composite_onto_band` function uses scalar f32 math per pixel:
- 4 divisions by 255.0
- 6 multiplications, 3 additions per blend
- Individual bounds-checked pixel access

### Solution: `wide` crate for portable SIMD

The `wide` crate provides `f32x4` — a portable SIMD type that processes 4 floats in one operation. Since we're always processing RGBA (4 channels), this is a perfect fit:

```rust
use wide::f32x4;

// Load 4 channels at once
let layer_px = f32x4::new(arrayf_from_4_bytes(layer_pixel.0));
let canvas_px = f32x4::new(arrayf_from_4_bytes(canvas_pixel.0));
let la = layer_px / f32x4::splat(255.0);
let ca = canvas_px / f32x4::splat(255.0);
// ... blend math using f32x4 operations ...
```

Also switch from `layer.get_pixel(lx, ly)` to direct `&[u8]` indexing with pre-computed row offsets to eliminate bounds checks.

### Files to modify

- `crates/ss-compositor/Cargo.toml` — add `wide` dependency
- `crates/ss-compositor/src/rendering/compositor.rs` — rewrite blend math in `composite_onto_band` using `f32x4`

### Priority

**Low.** Composite is already at 56ms (20% of total). After Phase 1, total will be ~85ms and composite will be an even smaller fraction. Only worth doing if we want to push below 50ms total.

---

## Phase 3: Custom SIMD bilinear warp for general transforms

**Expected impact:** general warp ~220ms → ~55ms (4× speedup for rotated clips)

### Problem

When rotation is present, we must fall through to `imageproc::warp_into` which is scalar-only. For projects with many rotated clips, this is still slow.

### Solution

Write a custom bilinear sampler that operates on raw `&[u8]` slices using `wide::f32x4`:

1. Pre-compute the inverse projection as `(a, b, c, d, e, f)` — 6 f32 values
2. For each output pixel, compute source `(sx, sy)` as `a*x + b*y + c, d*x + e*y + f`
3. Read 4 source pixels for bilinear blend using direct `&[u8]` indexing
4. Blend all 4 RGBA channels simultaneously with `f32x4`

This eliminates `imageproc`'s generic trait dispatch and bounds checking.

### Files to modify

- `crates/ss-compositor/Cargo.toml` — add `wide` (already added in Phase 2)
- `crates/ss-compositor/src/rendering/compositor.rs` — add `warp_bilinear_simd` function, use it instead of `warp_into` for all clips

### Priority

**Medium.** Only needed if we have projects with rotated clips. For the queens project (all Ken Burns), Phase 1 already solves the warp bottleneck. However, this is the right generalization for arbitrary projects.

---

## Phase 4: GPU rendering via wgpu

**Expected impact:** potential 10–50× speedup; total frame time <10ms

### Problem

CPU rendering is fundamentally limited by memory bandwidth and scalar SIMD width. Even with all optimizations above, we're still doing ~2M pixel operations per frame on the CPU.

### Solution

A single wgpu compute shader handles the entire per-clip pipeline:
1. Each pixel thread computes the inverse transform → source coordinates
2. Bilinear sample from source texture
3. Alpha blend into output framebuffer
4. Z-order handled by processing clips sequentially (or using atomic blend operations)

This replaces both `warp_clip` and `composite_onto_band` with GPU operations.

### Implementation sketch

- Add `wgpu` and `pollster` dependencies
- Create a `GpuCompositor` that implements `FrameRenderer`
- Upload source images to GPU textures once (cache across frames)
- Per frame: dispatch a compute shader per clip, blending into a shared output texture
- Read back the output texture to CPU `RgbaImage`

### Risks

- Requires a GPU (not all headless servers have one)
- GPU readback latency may negate gains for small images
- Significantly more complex code, harder to test
- wgpu API churn

### Priority

**Long-term.** This is the nuclear option for when CPU optimization is exhausted. The architecture (`FrameRenderer` trait) already supports swapping implementations, so this can be added without modifying the CPU path.

---

## Recommended Order

| Phase | Impact | Effort | When |
|-------|--------|--------|------|
| **Phase 1** (fast-image-resize) | **220ms → 25ms** | 1–2 days | Now |
| Phase 2 (SIMD composite) | 56ms → 15ms | 0.5 day | After Phase 1 if needed |
| Phase 3 (SIMD warp) | ~55ms for rotated | 2–3 days | When rotation projects exist |
| Phase 4 (GPU) | <10ms total | 1–2 weeks | Long-term |

**Phase 1 alone should get total frame time from ~280ms to ~85ms** — a 3.3× overall speedup and the biggest single improvement remaining.
