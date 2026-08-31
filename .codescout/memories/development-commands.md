# Workspace Development Commands

## The pre-commit gate — four commands, and the ORDER is load-bearing

Run all four before completing any task. CLAUDE.md § *Development Commands* carries the
full evidence for each; this is the executable summary.

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features    # LEAN lane — runs THIRD
cargo test --workspace                          # default lane — runs LAST
```

**Why the order matters, not just the commands.** The workspace shares one `target/`, and
`tests/cli_artifact.rs` resolves `target/debug/codescout` **by path at run time**. The lean
lane leaves a librarian-less binary there — after it, `artifact --help` exits **2**, the
subcommand gone rather than empty. A later default-features run then execs that binary and
dies with `unrecognized subcommand 'artifact'` on 10 of 11 tests, reading exactly like a
librarian feature-gating regression in whatever you just committed. Ending on the default
lane rebuilds it correctly (all **8** verbs restored — the help block prints nine lines
because clap appends its own `help`, so count verbs, not lines), so following the gate no
longer arms the trap for the next session. Cost of the ordering: ~8s (71.8s → 80.4s).

**Each command is the wide form for a measured reason. Do not narrow any of them:**

- **The long clippy form is the gate, not garnish.** Bare `cargo clippy -- -D warnings`
  lints only the root package's *non-test* targets with default features, so it passes trees
  CI fails. CI runs BOTH (`ci.yml:50` narrow, `:61` wide — verified 2026-08-31); only the
  second reaches `#[test]` code and `codescout-embed`'s feature-gated `local` module.
- **The lean lane is `test`, not `check`.** A `check` compiles the lean test targets and
  never runs them, so a lean-only *runtime* failure is invisible to it. It is also the only
  gate command that runs *without* default features, so an unconditional module reaching a
  `#[cfg(feature = "librarian")]` one compiles clean locally and fails only in CI's slow
  3-OS `no-features` test-matrix lane (`ci.yml:76`), never in the fast clippy job.
- **Both test lanes carry `--workspace`.** Bare `cargo test` builds only the **root
  package's** targets, so every test in a workspace member is invisible to it — not a
  `tests/`-directory quirk, an inline `#[cfg(test)]` module is equally unreachable.
  `--workspace` strictly subsumes the bare form (verified by `-- --list` set-difference: 0
  tests present in bare and absent from workspace). CI's own test job uses
  `cargo test --workspace ${{ matrix.config.flags }}` (`ci.yml:174`).

## codescout (main MCP server) — builds

```bash
cargo rb                     # ALIAS (.cargo/config.toml) = build --release --features server-stack
                             # OUR STACK's live-MCP release build: with server-stack on, VectorBackend::resolve()
                             # defaults to Qdrant — the full hybrid retrieval path against the running
                             # llm-infra stack (.env.amd). This is what we run locally for live MCP testing.
cargo build --release        # lean release (sqlite-vec lite, no Qdrant) — the daemon-free default repo
                             # cloners get; NOT what we run locally. Used as the crates.io publish-verification build.

# After `cargo rb`, run /mcp to reconnect. Both `rb` and `build --release` emit the same
# target/release/codescout (the feature flag doesn't change the output path), and the symlink
# auto-updates: ~/.cargo/bin/codescout → target/release/codescout

# Edit eval harness (ignored by default):
cargo test --test e2e -- edit_eval_harness --ignored
```

**Probing which binary actually serves you — `which codescout` is not the test.**
`~/.cargo/bin` is not on PATH in a non-login shell here, so `which codescout` returns
nothing while the symlink is present and correct (verified 2026-08-31: the symlink resolves
to `target/release/codescout`). An empty `which` therefore says nothing about whether the
binary exists or is fresh. Ask the running server instead:
`workspace(action="status")` reports `server.git_sha`, `server.git_dirty` and
`server.exe_deleted` — the copy that is answering you, which is the only one that matters
after a rebuild. A long-lived MCP session outlives any number of rebuilds; `mtime` on the
file answers a different question.

## codescout-embed

```bash
cargo test                   # unit tests (chunker, smoke) — no I/O required
cargo test -- --ignored      # integration tests (require Ollama or model download)
```

Note the member compiles **56** tests with `remote-embed` (52 executing; 4 `ollama_*` are
`#[ignore]`d) and **19** without — which is why the gate's `--workspace` matters: bare
`cargo test` from the root builds none of them.

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
