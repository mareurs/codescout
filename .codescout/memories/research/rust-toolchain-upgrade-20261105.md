# Rust 1.97.1 -> 1.99.1: findings and decision (2026-11-05)

Tracker that carries the work queue: docs/trackers/rust-toolchain-upgrade.md (id b9a5f911cf1cfcfa). Bug records: docs/issues/2026-11-05-build-rs-watches-shared-refs-so-any-commit-rebuilds-codescout.md (f8d06297d629928e) and docs/issues/2026-11-05-truncated-tool-query-test-failed-once-in-eight-gate-runs.md (649df5b30d126f2b).

NAMING PITFALL: a memory topic name must contain no dot. `research/rust-1.99-upgrade-20261105` was stored as `rust-1.md` (the name was cut at the first dot) and its anchors file kept the full name. This topic replaces it.

## Decision
The operator decided on 2026-11-05 to move the pin from 1.97.1 to 1.99.1 and said MSRV does not matter ("we are on the cutting edge"). The measurement below found NO compile-speed gain; the reasons to move are to stay current and to use what 1.98 and 1.99 added.

## Release notes (read, not measured)
- 1.98.0 (2026-08-20), 1.99.0 (2026-10-01), 1.99.1 (2026-10-15; fixes 3 regressions of 1.99.0 incl. an x86_64 FFI bool miscompile visible in release mode only, so use 1.99.1, never 1.99.0). Official posts give NO rustc compile-speed number. rustdoc ~20% faster (docs generation); Cargo 1.99 starts faster; Cargo 1.99 disables incremental when CI is set; cargo -Zfine-grain-locking is nightly only.
- New lints: warn c_void_returns, suspicious_runtime_symbol_definitions (1.98); deny invalid_runtime_symbol_definitions (1.98); hard error no_mangle_generic_items (1.99).

## Measurement (worktree codescout.worktrees/rust-1.99.1, branch chore/rust-1.99.1, HEAD 6bd3745e4 frozen, own RAM slots, gate.sh --check, runner scratchpad tc-ab/tc-ab.py, not in git)
- Truly warm gate (no rebuild, foreign CPU <= 3.5 cores): 1.97.1 TOTAL 151 s (PYTHON 151, LEAN 49, DEFAULT 65, CLIPPY 1); 1.99.1 TOTAL 153 s (PYTHON 153, LEAN 52, DEFAULT 63, CLIPPY 1 but failed). Equal within noise; TOTAL is the Python lane in both.
- Rebuilt codescout crate (lean/default test build, 2 warm runs each + cold): lean 52.8 / 54.8 s (1.97.1) vs 50.6 / 52.0 s (1.99.1); default 1m32 / 1m30 vs 1m27 / 1m34; cold default 1m55 vs 1m48. Differences <= 6%, foreign CPU 3-7 cores, n = 2-3: no measurable difference.
- Test counts identical: 6162 lean, 9030 default, 1599 Python. fmt green on both. The 29.5 s doctest step seen once on 1.99.1 (cold) did not repeat.

## Upgrade problems
1. clippy::double_must_use at crates/codescout-embed/src/embedder.rs:107 (`#[async_trait::async_trait] pub trait Embedder`, -D warnings). The must_use comes from async-trait 0.1.89's expansion. FIX: `cargo update -p async-trait` to 0.1.92 (upstream dtolnay/async-trait#303); this adds syn 3.0.9 as a build-time dependency. After it, codescout-embed passes clippy. Verified by the gate log (the crate no longer errors).
2. clippy::result_large_err at src/server.rs:674 (`async fn check_tool_access(...) -> Result<(), CallToolResult>`, Err at least 176 bytes). Hidden behind problem 1 until it was fixed. Single function, single caller (src/server.rs:1598). Plan: box the error (`Box<CallToolResult>`); src/tools/core/types.rs:1039 already keeps RecoverableError under this lint's threshold. More errors may hide behind it, because clippy stops per crate.
- The dead_code warnings for ReadDenyList / resolve / refuses in src/util/path_security.rs appear identically on 1.97.1: in the code, not the compiler.

## Finding that matters more than the compiler: a warm gate rebuilds after ANY commit
- build.rs (bake_git_sha) puts rerun-if-changed on HEAD, index, refs and packed-refs. In a linked worktree refs and packed-refs are the COMMON git dir's, so a commit by another session on any branch re-runs the build script and Cargo rebuilds crate codescout. No env override exists.
- Observed: runs after commits at 14:49 / 15:00 / 15:07 rebuilt (CLIPPY 72-74 s, LEAN test build ~52 s, DEFAULT ~90 s, about 3.5 minutes per gate); the run after the last commit did not. Time-order match plus the code read; no controlled experiment. Same on the main checkout, where `git add` changes the shared index.

## Other
- Flaky test tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs failed once (lean and default of one run, 1.97.1) at src/tools/run_command/tests.rs:3243; passed in the other 7 runs. Cause unknown.
- pytest-xdist 3.8.0 (only in scratchpad/xdist-site): light Python lane 145 / 135 s serial, 30.9 / 28.6 s with -n 8, 20.2 / 20.0 s with -n 16; 1599 passed, 0 failed in all 6 runs. Not wired into the gate. With the Rust lanes warm at ~119 s the warm TOTAL would fall from ~151 s to about 119 s: arithmetic, not a measured gate run.
- sccache: wraps rustc via the repo .cargo/config.toml; the 10 GiB cache was full; ~/.config/sccache/config now sets 20 GiB, effective at the next daemon restart. bin and proc-macro crates are not cacheable.
- lean vs default dependency graphs (cargo tree): 237 vs 431 packages; 219 of the 237 have identical features, so one target dir already shares them. cargo-hakari could fix at most ~16 third-party packages.
- A /mcp reconnect kills background jobs started through run_command (a gate run was lost this way). Start long runs with `setsid nohup ... > log 2>&1 < /dev/null` so they survive.
- The RAM pool was emptied at 16:32 (all slot dirs gone, only .lock files left); a named slot (--slot ram-N) needs its directory to exist, so create it with mkdir first.

## State

- LANDED 2026-11-05 on `experiments` by `git merge --ff-only` (tip 76335cb39), operator said "da". Not pushed. Contents: rust-toolchain.toml channel 1.99.1, rust-version 1.99, Cargo.lock async-trait 0.1.92, ci.yml msrv job on 1.99, boxed `check_tool_access` error, `strip_circumfix` and `from_utf8_lossy_owned` adopted.
- Final gate on the merged tree: all lanes exit 0 (TOTAL 387 s, crate rebuilt). The last merge of `experiments` added one memory file and was not re-gated.
- Toolchains in rustup now: stable (1.99.1), 1.99.1, 1.97.1. The pacman `rustup` package has no update. Only the Docker eval image (`tools/setup-ab-eval/agent-cs.Dockerfile`) still pins 1.97.1; the operator told us to leave it.
- Cleaned 2026-11-05: worktree rust-1.99.1, branch chore/rust-1.99.1, RAM slot-3 contents, toolchains 1.88 and 1.95.0. Kept 1.97.1: worktrees mutation-slot-0/1 and .codex/worktrees/615e/codescout pin it. sccache max is 20 GiB.
- Open queue: tracker b9a5f911cf1cfcfa.
