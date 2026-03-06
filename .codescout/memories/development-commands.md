# Development Commands

See `CLAUDE.md § Development Commands` for the canonical list. This supplements it.

## Before Completing Any Task
1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
All three must pass. Do not commit until they do.

## Testing via Live MCP Server

`cargo build --release` first, then restart with `/mcp`. The MCP server runs the **release**
binary — dev builds are not picked up.

## Feature-Gated Builds

```bash
# Local embedding (downloads model on first run ~20-300MB):
cargo build --features local-embed --no-default-features

# No optional features:
cargo build --no-default-features

# E2E tests (require live LSP servers installed):
cargo test --features e2e-rust        # needs rust-analyzer
cargo test --features e2e-python      # needs pyright-langserver
cargo test --features e2e-typescript  # needs typescript-language-server
cargo test --features e2e-kotlin      # needs kotlin-language-server
cargo test --features e2e-java        # needs jdtls
```

## Running the Server

```bash
cargo run -- start --project .         # stdio transport (default)
cargo run -- start --project . --sse   # HTTP/SSE transport
cargo run -- index --project .         # build embedding index
codescout dashboard --project .        # web UI (port 8099 default)
```

## Logging

Set `RUST_LOG=debug` or `RUST_LOG=codescout=trace` for verbose output.

## Branch Strategy

- `master` is protected — only cherry-picked, tested commits land here
- All experimental work goes on `experiments` (or a feature branch)
- Cherry-pick to master only after: tests pass, clippy clean, verified via live MCP
- No `.github/` CI workflows exist yet
