# Development Commands

See `CLAUDE.md § Development Commands` for the primary list. Supplements below.

## Extra Commands

```bash
# Run a specific test by name
cargo test test_name_substring

# Run tests with output visible
cargo test -- --nocapture

# Run integration tests only
cargo test --test integration

# Run with local embedding backend (no Ollama needed)
cargo build --features local-embed --no-default-features

# Run with no optional features (minimal build)
cargo build --no-default-features

# Check MSRV compatibility (Rust 1.75)
rustup run 1.75 cargo check

# Build and launch the dashboard (separate from MCP server)
cargo run -- dashboard --project . --port 8099

# Index the project (build embedding index)
cargo run -- index --project .
```

## Before Completing Work

1. `cargo fmt` — format all files
2. `cargo clippy -- -D warnings` — no warnings allowed
3. `cargo test` — all tests pass
4. If changes affect tool dispatch or MCP behavior:
   `cargo build --release && /mcp` (restart server in Claude Code)
5. If tools were renamed: update all 3 prompt surfaces
   (`server_instructions.md`, `onboarding_prompt.md`, `build_system_prompt_draft()`)

## CI Matrix

- ubuntu-latest / macos-latest / windows-latest
- 3 feature variants: default, `local-embed --no-default-features`, `--no-default-features`
- Plus: `cargo fmt --check`, `cargo clippy -- -D warnings`, MSRV 1.75, tool-docs-sync
