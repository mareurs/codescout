# Development Commands — codescout Workspace

## Workspace-Level (run from repo root)

```bash
# Build
cargo build                    # dev build
cargo build --release          # release build (required for live MCP testing)

# Test
cargo test                     # all tests (workspace)
cargo test -p codescout        # main crate only
cargo test -p codescout-embed  # embed crate only
cargo test -p librarian-mcp    # librarian crate only

# Lint / Format
cargo fmt                      # format all
cargo fmt --check              # CI check
cargo clippy -- -D warnings    # lint with errors

# E2E tests (requires real LSP servers installed)
cargo test --features e2e-rust     # Rust LSP tests
cargo test --features e2e-python   # Python LSP tests
cargo test --features e2e-ts       # TypeScript LSP tests
```

## Live MCP Testing

```bash
cargo build --release          # rebuild first
# Then restart MCP server in Claude Code:
#   /mcp → restart codescout
```

## Release (from master only)

```bash
# 1. Bump version in Cargo.toml
# 2. Build + test + clippy
cargo build --release && cargo test && cargo clippy -- -D warnings
# 3. Commit + tag
git add Cargo.toml Cargo.lock && git commit -m "chore: bump version to X.Y.Z"
git tag vX.Y.Z
# 4. Publish
CARGO_REGISTRY_TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2) cargo publish
# 5. Push + release
git push && git push --tags
gh release create vX.Y.Z --title "vX.Y.Z" --notes "..."
# 6. Rebase experiments
git checkout experiments && git rebase master
```

## Dashboard

```bash
codescout dashboard            # starts Axum HTTP dashboard (opt-in feature)
```

## Semantic Index

```bash
# Via MCP tool:
# index(action="build", force=true)    — full reindex
# index(action="status")               — check progress / drift
```
