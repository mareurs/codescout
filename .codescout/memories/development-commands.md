# Workspace Development Commands

## The pre-commit gate — `./scripts/gate.sh`; the ORDER of its Cargo commands is load-bearing

Run `./scripts/gate.sh` before completing any task (CLAUDE.md § Development Commands is the
canonical statement). It runs, in this order: the light Python lane
(`python3 ./scripts/python-tests.py --lane light`), `./scripts/fmt-mine.sh`, and then the three
Cargo commands below, inside a `target/` **leased from a pool for that one run**
(`~/.cache/codescout-gate`, `scripts/slot-pool.sh`). `docs/conventions/gate-ordering.md`
carries the full evidence for each command; this is the executable summary.

```bash
./scripts/fmt-mine.sh                           # replaces bare `cargo fmt`: formats only own/CLEAN paths
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features    # LEAN lane — runs THIRD
cargo test --workspace                          # default lane — runs LAST
```

How the script behaves, so you read its output right:

- The lanes are chained with `;`, never `&&`, so the default lane still runs (and rebuilds the
  binary) after an earlier failure. It prints `PYTHON LIGHT EXIT` and
  `GATE EXITS -> FMT= CLIPPY= LEAN= DEFAULT= FMT_STATE=` and exits non-zero if ANY lane failed.
- Options: `--check` (formatter read-only, needs no identity — the right mode for
  verification-only runs); `--context <abs path>` (harness-neutral session context for the
  formatter); `--exclusive-checkout <abs path>` (operator declaration that you own the checkout;
  conflicts with `--check`). Without a resolvable context the formatter is reported as blocked
  (`FMT_STATE=context-blocked`) and every other lane still runs.
  `docs/conventions/harness-neutral-session-context.md` has the modes and numeric outcomes.
- `fmt-mine.sh` exit codes: 0 clean/formatted, 1 differences or ownership refusal, 2 context/tool/usage error.
- It sets `CODESCOUT_SKIP_ONNX_TESTS=1` for the whole script (the two ONNX weight tests in
  `crates/codescout-embed/src/local.rs` panic without weights on disk; CI's `local-embed` job
  runs them) and prints what a green run does NOT cover (heavy Python suites, `server-stack`,
  librarian code in the lean lane, the ONNX tests).
- For a targeted run lease your own slot: `scripts/with-slot.sh cargo test --lib -- <filter>`
  (`--slot N` leases exactly slot-N). Do not reuse the path gate.sh printed by hand: it was
  leased for that run only.

**Why the order matters, not just the commands.** A shared `target/` and
`tests/cli_doc.rs` resolving `target/debug/codescout` at run time make a terminal lean lane
leave a librarian-less binary there, reading exactly like a feature-gating regression in
whatever you just committed, for whichever session runs the default lane next. Ending on the
default lane rebuilds it, so following the gate cannot arm the trap for anyone else — provided
both lanes actually run. Two later layers narrow that exposure, and neither makes the order
optional for someone who types the commands directly into a shared `target/` (the leased pool is
a mechanism for whoever runs `gate.sh` and a policy for everyone else): `gate.sh` leases a
private `target/`, and `tests/cli_doc.rs` now executes a **hardlink of the built binary**
(`pinned_binary()`, pinned to the inode, so a peer's rebuild-by-rename cannot swap it
mid-suite — `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md`).
An older reproduction, made against the predecessor test file before the `artifact` CLI
subcommand was collapsed into `doc`, measured a librarian-less binary failing most of that
file's tests; that specific number is deliberately not restated here — re-deriving it means
arming the shared trap on purpose while other sessions build against the same `target/`. The
collapse itself is pinned by `the_old_artifact_subcommand_is_gone` in `tests/cli_doc.rs`, which
asserts Clap's own "unrecognized subcommand" error text for `artifact find`. Verified
2026-10-19: the target is `cli_doc`, it holds **16** tests, and all 16 pass against a
librarian-bearing binary.

**Each command is the wide form for a measured reason. Do not narrow any of them:**

- **The long clippy form is the gate, not garnish.** Bare `cargo clippy -- -D warnings`
  lints only the root package's *non-test* targets with default features, so it passes trees
  CI fails. CI runs BOTH (`ci.yml:369` narrow, `:380` wide — verified 2026-10-19); only the
  second reaches `#[test]` code and `codescout-embed`'s feature-gated `local` module.
- **The lean lane is `test`, not `check`.** A `check` compiles the lean test targets and
  never runs them, so a lean-only *runtime* failure is invisible to it. It is also the only
  gate command that runs *without* default features, so an unconditional module reaching a
  `#[cfg(feature = "librarian")]` one compiles clean locally and fails only in CI's slow
  3-OS `no-features` test-matrix lane (`ci.yml:394`), never in the fast clippy job.
- **Both test lanes carry `--workspace`.** Bare `cargo test` builds only the **root
  package's** targets, so every test in a workspace member is invisible to it — not a
  `tests/`-directory quirk, an inline `#[cfg(test)]` module is equally unreachable.
  `--workspace` strictly subsumes the bare form (verified by `-- --list` set-difference: 0
  tests present in bare and absent from workspace). CI's own test job uses
  `cargo test --workspace ${{ matrix.config.flags }}` (`ci.yml:493`).

## codescout (main MCP server) — builds

```bash
./scripts/rb.sh              # THE release build: runs `cargo rb` only after checking HEAD contains origin
                             # (see below), then prints which sessions still serve the replaced binary
cargo rb                     # ALIAS (.cargo/config.toml) = build --release --features server-stack
                             # OUR STACK's live-MCP release build: with server-stack on, VectorBackend::resolve()
                             # defaults to Qdrant — the full hybrid retrieval path against the running
                             # llm-infra stack (.env.amd). This is what we run locally for live MCP testing.
                             # It does NOT pass --no-default-features, so it also builds `local-embed`
                             # (in `default` since 2026-09-17).
cargo build --release        # sqlite-vec lite, no Qdrant — the daemon-free default repo
                             # cloners get; NOT what we run locally. Used as the crates.io publish-verification build.

# After `./scripts/rb.sh` (or `cargo rb`), run /mcp to reconnect. Both emit the same
# target/release/codescout (the feature flag doesn't change the output path), and the symlink
# auto-updates: ~/.cargo/bin/codescout → target/release/codescout

# Edit eval harness (ignored by default; the test target is `e2e_tests`, there is no `e2e` target):
cargo test --test e2e_tests -- edit_eval_harness --ignored
```

`scripts/rb.sh` exists because a cargo alias cannot run a precondition. It `git fetch`es
`origin/<branch>` first and refuses (exit 1) when HEAD is behind it — a stale `origin/*` ref
would read as "not behind" — and refuses (exit 2) when the fetch itself fails.
`CODESCOUT_RB_ACK=behind ./scripts/rb.sh` builds anyway as a recorded decision. It never pulls for
you (shared checkout). It does NOT check that the working tree carries uncommitted Rust, yours or
a peer's, so after any rebuild you rely on, ask the binary what it does rather than trusting mtime
or exit status. `gate.sh` deliberately leaves the release profile on the shared `target/`, because
the `~/.cargo/bin/codescout` symlink points at it.

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

Note the member lists **84** tests with `remote-embed` (83 in the lib plus the one in
`tests/ollama_probe_installs_its_own_crypto_provider.rs`; 80 executing, 4 `ollama_*` are
`#[ignore]`d) and **36** without (`cargo test -p codescout-embed --no-default-features --
--list`, measured 2026-10-19) — which is why the gate's `--workspace` matters: bare
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
