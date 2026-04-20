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
