# Development Commands

See `CLAUDE.md § Development Commands` for the canonical list. This supplements it.

## Before Completing Any Task
1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
All three must pass. Do not commit until they do.

## Feature-Gated Builds

```bash
# Local embedding (downloads model on first run ~20-300MB):
cargo build --features local-embed --no-default-features

# No optional features (CI matrix "no-features"):
cargo build --no-default-features

# E2E tests (require live LSP servers installed):
cargo test --features e2e-rust        # needs rust-analyzer
cargo test --features e2e-python      # needs pyright-langserver
cargo test --features e2e-typescript  # needs typescript-language-server
cargo test --features e2e-kotlin      # needs kotlin-language-server
cargo test --features e2e-java        # needs jdtls
```

## CI Checks

CI runs (`.github/workflows/ci.yml`): fmt check, clippy, test (3 OS × 3 feature combos),
tool-docs-sync (verifies every tool name in `src/tools/*.rs` has a docs page), MSRV 1.75.

**Tool docs sync:** When adding a new tool, add a matching `## \`tool_name\`` section in
`docs/manual/src/tools/`. CI will fail if the counts don't match.

## Running the Server

```bash
cargo run -- start --project .         # stdio transport (default)
cargo run -- start --project . --sse   # HTTP/SSE transport
cargo run -- index --project .         # build embedding index
codescout dashboard --project .        # web UI (port 8099 default)
```

## Logging

Set `RUST_LOG=debug` or `RUST_LOG=codescout=trace` for verbose output.
