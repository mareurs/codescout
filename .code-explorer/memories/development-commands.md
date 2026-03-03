# Development Commands

See CLAUDE.md for primary commands (926 tests passing as of 2026-03-03). This memory adds gotchas and extras.

## Extras Not in CLAUDE.md

### Feature-specific builds
```bash
cargo build --features local-embed       # Local ONNX embedding
cargo build --no-default-features        # Minimal (no remote-embed, no dashboard)
```

### E2E tests (require real LSP servers)
```bash
cargo test --features e2e-rust           # Rust LSP tests
cargo test --features e2e-python         # Python LSP tests
cargo test --features e2e                # All languages
```

### Dashboard
```bash
cargo run -- dashboard --project .       # Launch web UI on port 8099
```

### LSP server management
```bash
./scripts/install-lsp.sh --check         # See what's installed/missing
./scripts/install-lsp.sh --all           # Install all LSP servers
./scripts/install-lsp.sh rust python     # Install specific ones
```

## Before Completing Work
1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
4. Check `docs/TODO-tool-misbehaviors.md` — log any tool issues found during work
5. Batch changes into a single commit — don't commit intermediate steps
