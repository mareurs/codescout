# Development Commands

## Core Build & Test (run from workspace root)

```bash
cargo build                          # debug build (all workspace members)
cargo build --release                # release build (required before MCP restart)
cargo test                           # all unit + integration tests
cargo clippy -- -D warnings          # lint (must be clean before completing)
cargo fmt                            # format (must be run before completing)
cargo fmt --check                    # CI-style format check (no changes)
```

## Pre-Completion Checklist

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```
Then `cargo build --release` + `/mcp` restart for live MCP verification.

## Feature-Gated Tests

```bash
cargo test --features e2e-rust       # Rust LSP e2e tests (needs rust-analyzer)
cargo test --features e2e-python     # Python LSP e2e tests (needs pyright)
cargo test --features e2e-typescript # TypeScript LSP e2e tests (needs tsserver)
cargo test --features e2e-kotlin     # Kotlin LSP e2e tests (needs kotlin-language-server)
cargo test --features e2e-java       # Java LSP e2e tests (needs jdtls)
```

## Individual Crates

```bash
cargo test -p codescout-embed        # embed crate only
cargo test -p librarian-mcp          # librarian crate only
```

## MCP Server

```bash
cargo build --release                # build the release binary
# then in Claude Code: /mcp          # restart MCP server to pick up new binary
```

## Release

See `CLAUDE.md § Release Cycle` for full checklist (version bump → build → tag → publish → push → GitHub release).
```bash
CARGO_REGISTRY_TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2) cargo publish
```

## Docs

```bash
cd docs/manual && mdbook build       # build the manual
cd docs/manual && mdbook serve       # local preview at localhost:3000
```
