---
id: b9a5f911cf1cfcfa
kind: tracker
status: active
title: Rust toolchain upgrade 1.97.1 to 1.99.1, and what it leaves open
owners:
- marius
tags:
- toolchain
- rust
- upgrade
- gate
- build-speed
- resume-queue
topic: rust toolchain pin and build speed
---

# Rust toolchain upgrade 1.97.1 to 1.99.1, and what it leaves open

**Deliberately owns no `PREFIX-N` namespace.** This is a resume/handoff document, not a
ledger: nothing here is citable as an entry and nothing allocates ids. See
`docs/TAXONOMY.md` § *Trackers that deliberately own no prefix*. Edit it directly.

**Status 2026-11-05:** the operator decided to move the pin from 1.97.1 to 1.99.1. The bump is
prepared and gated on branch `chore/rust-1.99.1` (worktree `codescout.worktrees/rust-1.99.1`).
The bump landed on `experiments` the same day by fast-forward (tip `76335cb39`, operator said "da"). The gate on the merged tree was green (exit 0 on all lanes, TOTAL 387 s with a crate rebuild). Nothing was pushed by this session. On 2026-11-06 another session pushed `experiments` (`537eb2cdf..f7d65b877`) on its operator's decision, so the bump is now on `origin/experiments`. Verified with `git branch -r --contains 4098f07bb`.

## Why we move, stated honestly

The 2026-11-05 measurement found **no compile-speed gain**. The reasons to move are to stay
current and to be able to use what 1.98 and 1.99 added. Nothing below claims a speed win.

- Warm gate, no rebuild, same frozen HEAD `6bd3745e4`, foreign CPU under 3.5 cores:
  TOTAL 151 s on 1.97.1, 153 s on 1.99.1. Both are the Python lane (151 / 153 s).
- Test build of the rebuilt `codescout` crate: lean 52.8 / 54.8 s (1.97.1) against
  50.6 / 52.0 s (1.99.1); default 92 / 90 s against 87 / 94 s. Differences of at most 6%, n = 2 to 3.
- Test counts are identical on both: 6162 lean, 9030 default, 1599 Python.
- The official 1.98 and 1.99 posts give no rustc compile-speed number. The one speed figure
  (rustdoc about 20% faster) is for documentation generation, which the gate does not run.

Evidence and method: codescout memory `research/rust-toolchain-upgrade-20261105`.

## What the bump changes

| File | Change |
|---|---|
| `rust-toolchain.toml` | `channel = "1.99.1"`, plus a pointer to this tracker |
| `Cargo.lock` | `async-trait` 0.1.89 to 0.1.92 (and `syn 3.0.9` added as a build-time dependency) |
| `.github/workflows/ci.yml` | comment on the `msrv` job no longer names a version |
| `tests/test_harness_formatter.py` | default `RUSTUP_TOOLCHAIN` 1.97.1 to 1.99.1 |
| `Cargo.toml` | `rust-version` 1.88 to 1.99 (policy: track the newest stable) |
| `.github/workflows/ci.yml` | job `msrv` renamed `MSRV (1.99)`, installs 1.99, runs `cargo +1.99 check` |
| `src/server.rs` | `check_tool_access` returns `Result<(), Box<CallToolResult>>` (`result_large_err`) |
| 9 files under `src/` | `strip_circumfix` (11 sites) and `String::from_utf8_lossy_owned` (3 sites), commit `2933e242c` |

CI installs no toolchain: it reads `rust-toolchain.toml`, so CI follows the pin. The `msrv` job
now installs 1.99 (`cargo +1.99 check`), because the project tracks the newest stable (row 4).

**Upgrade problems found (two, both fixed on the branch).** The first: `clippy::double_must_use` fired on `#[async_trait::async_trait]
pub trait Embedder` (`crates/codescout-embed/src/embedder.rs:107`), under `-D warnings`. The
attribute comes from `async-trait` 0.1.89's expansion, not from our code. `async-trait` 0.1.92 drops
it (upstream dtolnay/async-trait#303), so the fix is the lockfile bump, not an `allow`. The second was hidden behind the first, because clippy stops per crate: `clippy::result_large_err` on `CodeScoutServer::check_tool_access` (`src/server.rs`, Err of at least 176 bytes, on 1.99 only). The error is now `Box<CallToolResult>`; the one caller unboxes it.

## Work queue

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | Land the 1.99.1 bump on `experiments` | done 2026-11-05, published 2026-11-06 | Branch `chore/rust-1.99.1`: `4098f07bb` (toolchain, MSRV 1.99, async-trait 0.1.92, boxed refusal) and `2933e242c` (std APIs). Gate green in the worktree, base `6bd3745e4`: all lanes exit 0, 6162 lean, 9030 default, 1599 Python. Landed on `experiments` by `git merge --ff-only` at `76335cb39` after merging `experiments` into the branch twice; the final gate on the merged tree was green (FMT, CLIPPY, LEAN, DEFAULT all exit 0). The last merge added one memory file only and was not re-gated. Pushed by another session on 2026-11-06. A commit hook (`dead-artifact-ids`) skipped itself because the worktree has no `codescout` binary. |
| 2 | `build.rs` re-runs after any commit in the repo | open | `bake_git_sha` watches `refs` and `packed-refs` of the shared git dir. Observed cost about 3.5 minutes per gate (clippy 72 s, lean 52 s, default 90 s). Candidate fix: watch `HEAD` and the one ref it points to. Must keep the baked SHA correct after a commit. Bug: `docs/issues/2026-11-05-build-rs-watches-shared-refs-so-any-commit-rebuilds-codescout.md`. |
| 3 | `pytest-xdist` in the light Python lane | open | `-n 16`: 20 s against 140 s, 1599 passed in all 6 runs. Not wired in. Warm TOTAL would then follow the Rust lanes (about 119 s), an estimate, not a measured gate run. |
| 4 | MSRV decision | decided 2026-11-05 | Operator: MSRV does not matter, the project tracks the newest stable. `rust-version` is 1.99 and the `msrv` job follows the pin. GitHub checked with `gh` on the same day: no classic branch protection on `master` or `experiments`, and the two rulesets only require a pull request (0 approvals, owner bypasses), so no required check names the job. Not checked: organisation-level settings. |
| 5 | Adopt new std APIs | done, except `filetime` | Done in `2933e242c` (11 `strip_circumfix`, 3 `from_utf8_lossy_owned`). Still open: `fs::set_times` could drop the `filetime` dev-dependency (about 100 uses in tests). Original candidates: `String::from_utf8_lossy_owned` (1.99) at `src/tools/run_command/inner.rs:481-482`, `run_command/output.rs:360`, `tools/output_buffer.rs:1928`; `str::strip_circumfix` (1.98), about 19 sites of `strip_prefix(..).and_then(..strip_suffix..)`; `fs::set_times` (1.99) could drop the `filetime` dev-dependency (`Cargo.toml:273`). All small. |
| 6 | Flaky test | open | `tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs` failed once in 8 runs (1.97.1, lean and default of one run). Bug: `docs/issues/2026-11-05-truncated-tool-query-test-failed-once-in-eight-gate-runs.md`. |
| 7 | Read the clippy 1.98 and 1.99 changelogs | open | The blog posts only link to them. New lints could be useful. Not read yet. |
| 8 | Edition 2024 | not planned | Cargo 1.99 lets edition-2024 members override an inherited workspace dependency's `default-features`. We are on edition 2021. Separate, large job. |
| 9 | sccache cache size | done 2026-11-05 | `sccache --show-stats` reports max cache size 20 GiB (11 GiB used). The daemon restarted on its own. Machine-local, not in the repo. |
| 10 | Clean up the experiment | done 2026-11-05, except 1.97.1 | Removed: worktree `rust-1.99.1`, branch `chore/rust-1.99.1` (merged, `git branch -d`), RAM slot-3 contents (9.8 GB), toolchains 1.88 and 1.95.0 (no live file pins them). Kept on purpose: toolchain 1.97.1, because worktrees `mutation-slot-0`, `mutation-slot-1` and `.codex/worktrees/615e/codescout` still pin it. Remove it when those move. |
| 11 | First CI run on 1.99.1 | done 2026-11-06, bump green | Run 42205085200 on `f7d65b877`: 31 of 32 jobs green on 1.99.1 (Clippy, Format, `MSRV (1.99)`, Test on ubuntu, macOS and windows in all three configurations, Python suites). rustup installed 1.99.1 on the runners from `rust-toolchain.toml`. The one red job, `Audit Doc Refs`, is not caused by the bump: 3 `high` findings in docs written 2026-11-04 (`src/prompts/README.md:64` and `docs/evals/harness-channel-hint-authority-preregistration.md:203,242`). The owner of the README line (session `732f1c09`) was told on 2026-11-06; session `677dd210`, owner of the other two, was not live. Not fixed here. |

## How to bump next time

The procedure is in the header of `rust-toolchain.toml`: change `channel`, run the gate, expect new
lints, commit on its own. Add to it: run the gate in a worktree first, so other sessions keep
their compiler and their warm target dirs until the commit lands; then check whether the new
clippy lints come from a dependency's macro expansion, where a lockfile bump is cleaner than an `allow`.

## History

### 2026-11-05 measurement and decision

Toolchain 1.99.1 installed beside 1.97.1. Worktree and RAM slots created. Fifteen gate runs
(2 cold, 6 warm, plus the earlier old-versus-new `gate.sh` runs) on a frozen HEAD. The warm runs
after a repo commit rebuilt the crate; only the runs without a rebuild are a fair comparison. The
operator then decided to move to 1.99.1.
