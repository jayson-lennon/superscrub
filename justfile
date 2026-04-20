test:
    cargo test --workspace

check:
    cargo check --workspace

clippy:
    cargo clippy --workspace -- -D warnings

fmt:
    cargo fmt --all

run project:
    cargo run -p ss-editor -- {{project}}

render project:
    cargo run -p ss-render -- {{project}}

# Generate test images and render frames at key timestamps for visual inspection.
# Output goes to examples/test-frame/output/
test-frame: (test-frame-gen) (test-frame-render "0.0" "start") (test-frame-render "3.0" "mid-slide") (test-frame-render "5.0" "peak") (test-frame-render "9.0" "fade-out")

test-frame-gen:
    cargo run --bin gen-test-images -- examples/test-frame
    @mkdir -p examples/test-frame/output

test-frame-render time label:
    cargo run -p ss-compositor --bin ss-compositor -- examples/test-frame/project.json --time {{time}} --output examples/test-frame/output/frame-{{label}}-t{{time}}.png
    @echo ""

# Render a sequence of evenly-spaced frames as PNGs for visual inspection.
# Generates 5 frames across the 10s duration by default.
# Override count: just test-frame-seq 10
test-frame-seq count='5': (test-frame-gen)
    #!/usr/bin/env bash
    set -euo pipefail
    dir="examples/test-frame/output/sequence"
    rm -rf "$dir"
    mkdir -p "$dir"
    dur=10.0
    count={{count}}
    for i in $(seq 0 $((count - 1))); do
        t=$(echo "scale=2; $dur * $i / ($count - 1)" | bc)
        printf "  rendering frame %d/%d  t=%5.2fs\n" "$((i + 1))" "$count" "$t"
        cargo run -q -p ss-compositor --bin ss-compositor -- \
            examples/test-frame/project.json \
            --time "$t" \
            --output "$dir/frame-$(printf '%03d' "$i")-t${t}.png"
    done
    echo ""
    echo "  wrote $count frames to $dir/"
