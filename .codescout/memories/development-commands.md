# Development Commands

## Workspace-wide (run from `/home/marius/work/claude/code-explorer`)

```bash
# Build everything
cargo build

# Build release binary (required before MCP testing)
cargo build --release

# Run all tests
cargo test

# Lint (zero warnings required)
cargo clippy -- -D warnings

# Format
cargo fmt

# Full pre-commit check (required before every commit)
cargo fmt && cargo clippy -- -D warnings && cargo test
```

## Crate-specific

```bash
# Build only librarian-mcp
cargo build -p librarian-mcp

# Test only codescout-embed
cargo test -p codescout-embed

# Test only integration tests
cargo test --test integration

# Test LSP symbol tests
cargo test --test symbol_lsp
```

## MCP Server (code-explorer)

```bash
# After cargo build --release, restart MCP server in Claude Code
/mcp
# This picks up the new release binary — dev builds are not used
```

## librarian-mcp CLI

```bash
# Run as stdio MCP server (default, no args)
librarian-mcp

# Reindex all workspace repos
librarian-mcp reindex

# Import codescout project metadata
librarian-mcp import-codescout
```

## Fixture Libraries

```bash
# Java fixture
cd tests/fixtures/java-library && ./gradlew build

# Kotlin fixture
cd tests/fixtures/kotlin-library && ./gradlew build

# TypeScript fixture (if tsconfig.json present)
cd tests/fixtures/typescript-library && tsc

# Python fixture — no build step needed
```

## Release (from master only)

```bash
# See CLAUDE.md § Release Cycle for full 8-step checklist
# Key steps: bump Cargo.toml version → cargo build --release → cargo test →
# git tag vX.Y.Z → cargo publish → git push --tags → gh release create
```
