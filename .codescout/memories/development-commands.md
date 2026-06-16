# Workspace Development Commands

## codescout (main MCP server)

```bash
cargo rb                     # ALIAS (.cargo/config.toml) = build --release --features server-stack
                             # OUR STACK's live-MCP release build: with server-stack on, VectorBackend::resolve()
                             # defaults to Qdrant — the full hybrid retrieval path against the running
                             # llm-infra stack (.env.amd). This is what we run locally for live MCP testing.
cargo build --release        # lean release (sqlite-vec lite, no Qdrant) — the daemon-free default repo
                             # cloners get; NOT what we run locally. Used as the crates.io publish-verification build.
cargo test                   # unit + integration tests (excludes #[ignore])
cargo clippy -- -D warnings  # lint (must be clean before commit)
cargo fmt                    # format (run before commit)
# After `cargo rb`, run /mcp to reconnect. Both `rb` and `build --release` emit the same
# target/release/codescout (the feature flag doesn't change the output path), and the symlink
# auto-updates: ~/.cargo/bin/codescout → target/release/codescout

# Edit eval harness (ignored by default):
cargo test --test e2e -- edit_eval_harness --ignored
```

## codescout-embed

```bash
cargo test                   # unit tests (chunker, smoke) — no I/O required
cargo test -- --ignored      # integration tests (require Ollama or model download)
```

## edit-eval-rust (fixture)

```bash
cargo check                                          # verify fixture compiles
git restore tests/fixtures/edit-eval-rust/src        # reset mutations between eval cases
```

## nav-eval-rust (fixture)

```bash
cargo check    # verify fixture compiles
```

## java-library (fixture)

```bash
./gradlew build    # compile + assemble (requires JDK 21+)
```

## kotlin-library (fixture)

```bash
./gradlew build    # requires JDK + Kotlin 2.1 toolchain
```

## rust-library (fixture)

```bash
cargo check    # verify compilation
```

## python-library (fixture)

```bash
python -c "import library"    # verify imports (stdlib only, no build step)
```

## typescript-library (fixture)

```bash
tsc    # compile src/ → dist/ (no test runner configured)
```