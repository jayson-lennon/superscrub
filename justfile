COPYRIGHT_NAME := "Jayson Lennon"
COPYRIGHT_YEAR := "2026"

test:
    cargo nextest run
    cargo test --doc

check:
    cargo check --workspace

clippy:
    cargo clippy --all-targets

fmt:
    cargo fmt --all

semver:
    cargo semver-checks

clean:
    rm -rfv .build/
    cargo clean -vv

coverage:
    cargo llvm-cov --lcov --output-path coverage.lcov

coverage-report:
    cargo llvm-cov report --html

debt: coverage
    debtmap analyze . --lcov coverage.lcov

apply-license:
   #!/bin/bash

   # --- CONFIGURATION ---
   NAME="{{COPYRIGHT_NAME}}"
   YEAR="{{COPYRIGHT_YEAR}}"
   # Add the extensions you want to target (space separated)
   EXTENSIONS=("rs")

   # The Header Template
   HEADER="Copyright (C) $YEAR $NAME

   This program is free software: you can redistribute it and/or modify
   it under the terms of the GNU Affero General Public License as
   published by the Free Software Foundation, either version 3 of the
   License, or (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU Affero General Public License for more details.

   You should have received a copy of the GNU Affero General Public License
   along with this program.  If not, see <https://www.gnu.org/licenses/>."

   # Convert header to a commented block (using // for JS/CPP style)
   # If you use Python only, change '// ' to '# ' below.
   COMMENTED_HEADER=$(echo "$HEADER" | sed 's/^/\/\/ /')

   # --- EXECUTION ---
   for ext in "${EXTENSIONS[@]}"; do
       echo "Processing .$ext files..."

       # Find files with the extension, excluding node_modules or hidden git folders
       find . -type f -name "*.$ext" -not -path "*/.*" -not -path "*node_modules*" | while read -r file; do

           # Check if "Copyright" already exists in the first 5 lines
           if head -n 5 "$file" | grep -iq "Copyright"; then
               echo "  Skipping $file (Header already exists)"
           else
               echo "  Adding header to $file"
               # Create a temporary file with header + original content
               { echo "$COMMENTED_HEADER"; echo ""; cat "$file"; } > "$file.tmp" && mv "$file.tmp" "$file"
           fi
       done
   done

   echo "Done!"

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
