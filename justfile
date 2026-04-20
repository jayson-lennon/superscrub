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

# Generate demo images and open the editor.
demo: demo-gen
    cargo run -p ss-editor -- examples/demo/project.json

demo-gen:
    cargo run --bin gen-demo-images -- examples/demo

render project:
    cargo run -p ss-render -- {{project}}

# Quick render test: generate test-frame images and render to MP4.
# Full 10s:  just test-render
# Short 3s:  just test-render 0 3
# Sub-range: just test-render 2 7
test-render start='0' end='10': (test-frame-gen)
    @mkdir -p examples/test-frame/output
    cargo run -q -p ss-render -- examples/test-frame/project.json --start {{start}} --end {{end}} --output examples/test-frame/output/test-render.mp4
    @echo ""
    @echo "  wrote examples/test-frame/output/test-render.mp4"

# Generate test images and render frames at key timestamps for visual inspection.
# Output goes to examples/test-frame/output/
test-frame: (test-frame-gen) (test-frame-render "0.0" "start") (test-frame-render "3.0" "mid-slide") (test-frame-render "5.0" "peak") (test-frame-render "9.0" "fade-out")

test-frame-gen:
    cargo run --bin gen-test-images -- examples/test-frame
    @mkdir -p examples/test-frame/output

test-frame-render time label:
    cargo run -p ss-compositor --bin ss-compositor -- examples/test-frame/project.json --time {{time}} --output examples/test-frame/output/frame-{{label}}-t{{time}}.png
    @echo ""

# Test audio playback with the sample mp3.
# play from start:
#   just test-audio
# seek to 5s then play:
#   just test-audio-seek 5
# play, pause after 3s, resume after 2s pause:
#   just test-audio-pause 3
test-audio:
    cargo run -p ss-audio --bin ss-audio -- examples/test-audio/counting.mp3

test-audio-seek seconds:
    cargo run -p ss-audio --bin ss-audio -- examples/test-audio/counting.mp3 --seek {{seconds}}

test-audio-pause seconds:
    cargo run -p ss-audio --bin ss-audio -- examples/test-audio/counting.mp3 --pause {{seconds}}

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
