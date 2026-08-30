---
id: '00c57991620f6e05'
kind: bug
status: open
title: 'BUG: a peer''s lean build clobbers target/debug/codescout, reddening 10 cli_artifact tests with a gating-regression message'
tags:
- test-isolation
- shared-checkout
- feature-gating
- false-red
closed: ''
opened: 2026-08-30
owner: marius
related:
- docs/trackers/reconnaissance-patterns.md
severity: medium
---

# BUG: a peer's lean build clobbers `target/debug/codescout`, reddening 10 `cli_artifact` tests with a gating-regression message

## Summary

`tests/cli_artifact.rs` execs the root binary by **path** (`target/debug/codescout`). Every
session in this checkout shares one `target/`, so a peer running the documented lean gate
(`cargo test --workspace --no-default-features`) rewrites that path with a binary built
**without** the `librarian` feature. A concurrent default-features run then execs it and 10
of 11 tests fail with `error: unrecognized subcommand 'artifact'` — which reads exactly like
a librarian feature-gating regression, and is not one.

## Symptom (Effect)

`cargo test --workspace` at `ce2b847f`, exit 101:

```
test result: FAILED. 1 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

All ten carry the same stderr:

```
thread 'artifact_find_on_empty_catalog_returns_empty_items_json' panicked:
Unexpected failure.
code=2
stderr=```
2026-08-30T12:54:17.857176Z  WARN codescout::config::global: CODESCOUT_ENV_FILE set to /tmp/.tmpg44mVw/no-startup.env but the file was not found
error: unrecognized subcommand 'artifact'

Usage: codescout <COMMAND>

For more information, try '--help'.
```
```

The failing set is the whole `artifact_*` family: `artifact_find_on_empty_catalog_returns_empty_items_json`,
`artifact_find_semantic_without_embedder_reports_hint`, `artifact_graph_missing_id_runs`,
`artifact_refresh_list_stale_empty_catalog_succeeds`, `artifact_create_then_get_round_trip`,
`artifact_link_then_graph_shows_edge`, `artifact_state_at_requires_commit_or_timestamp`,
`artifact_find_bad_filter_reports_error`, `artifact_get_missing_id_errors_and_names_both_recovery_paths`,
`artifact_update_status_archived_then_find_excludes`. The one that passed
(`artifact_event_list_empty_catalog_runs`) had already run before the clobber landed.

The 0.04s runtime is itself a tell — none of these tests did any work; they all died on
argument parsing.

## Reproduction

Two shells in the same checkout, no `CARGO_TARGET_DIR` set:

```
# shell A
cargo test --workspace --no-default-features

# shell B, while A is in its build phase or just after
cargo test --workspace
```

B's `tests/cli_artifact.rs` fails with `unrecognized subcommand 'artifact'` whenever it
happens to exec after A has written the binary.

Deterministic single-shell version:

```
cargo build --bin codescout --no-default-features
cargo test --test cli_artifact --no-run     # do NOT let cargo rebuild the bin
./target/debug/codescout --help             # no `artifact` subcommand
```

**Do not skip the reproduction in favour of the Fix section below** — the fix options there
are a hypothesis about it, per CLAUDE.md.

## Environment

- Linux 7.1.11-arch1-1, rustc 1.97.1, branch `experiments`, main checkout
  `/home/marius/work/claude/codescout`.
- `CARGO_TARGET_DIR` **unset** (verified 2026-08-30).
- Three linked worktrees exist; `.claude/worktrees/peer-delegation` and
  `.worktrees/vdi-windows` have no `target/` dir, `.claude/worktrees/operator-rules-phase-2`
  has its own. So the producing build ran in the **main checkout**, not a worktree.
- Two to four Claude sessions concurrently active in this checkout.

## Root cause

Two independent facts meet at one path:

1. `Cargo.toml` — `default = ["remote-embed", "http", "librarian"]`. The `artifact`
   subcommand exists only when `librarian` is compiled in, so `--no-default-features`
   legitimately produces a binary without it.
2. `tests/cli_artifact.rs:11` — `let mut cmd = Command::cargo_bin("codescout").unwrap();`.
   `assert_cmd`'s `cargo_bin` resolves the binary **by path at run time**. It reads whatever
   `target/debug/codescout` happens to be when the test executes; it carries no record of
   the feature set the test binary was compiled against.

So the compiled artifact is shared mutable state between sessions in a way the source tree
is not. Cargo's build-directory lock serialises the *writes* — it does not stop a later
default-features test process from execing a binary an earlier lean build left behind.

**Measured 2026-08-30, 15:53–15:55 local:**

- 15:54:10 — `./target/debug/codescout --help` listed `start index migrate-memories version
  operator-rules help`. No `artifact`.
- 15:55:12 — after `cargo test --test cli_artifact` alone, cargo printed `Compiling codescout
  … Finished` (6.98s) and the same command's `--help | grep -c artifact` returned `4`.
  Cargo only recompiles when the fingerprint changed; the feature set is what changed.
- A later `cargo build --bin codescout` printed `Blocking waiting for file lock on build
  directory`, confirming a peer cargo process held the lock in this window.

**Inferred, not measured:** *which* command produced the librarian-less binary. No session
announced it, and cargo leaves no attribution. `cargo test --workspace --no-default-features`
is the documented gate command that has this effect, and is the likely producer — but any
`--no-default-features` build in the main checkout does it.

## Evidence

### The control run — this is the discriminator

`cargo test --test cli_artifact` alone, immediately after the failing full run:

```
running 11 tests
test artifact_state_at_requires_commit_or_timestamp ... ok
[…]
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
EXIT=0
```

11/11, with a recompile in front of it. Nothing in the source tree changed between the two
runs. A full re-run of `cargo test --workspace` minutes later reproduced none of the ten
failures (4712 passed / 1 failed, that one unrelated —
the wall-clock flake, since fixed and archived at
`docs/issues/archive/2026-08-30-concurrency-timing-test-flakes-as-its-own-regression-signature.md`).

### Why the timestamp in the panic is not evidence of a stale run

The captured stderr reads `2026-08-30T12:54:17Z` against a local wall clock of 15:54. That
is UTC vs EEST (UTC+3), not a stale log. Noted because it is the first thing that looks
wrong and it is a dead end.

## Hypotheses tried

1. **Hypothesis:** the peer commits landed a librarian-gating regression.
   **Test:** `cargo test --test cli_artifact` alone at the same commit.
   **Verdict:** rejected — 11/11 pass, no source change between runs.
2. **Hypothesis:** ambient environment (a stray `CODESCOUT_*` var) disabled the librarian.
   **Test:** read the actual failure — it is `clap` refusing to *parse* the subcommand, which
   happens before any config or env is consulted. `LIBRARIAN_ENABLED=0` disables *runtime
   registration* of a compiled-in librarian; it cannot remove a `clap` subcommand.
   **Verdict:** rejected.
3. **Hypothesis:** the binary on disk was built without `librarian`.
   **Test:** `./target/debug/codescout --help` before and after the control run.
   **Verdict:** confirmed — subcommand absent, then present after cargo recompiled.

## Fix

Not applied. Options, cheapest first:

- **Preflight assertion in the CLI tests.** Have `run_cmd` (`tests/cli_artifact.rs:11`)
  check the binary it is about to exec actually carries the subcommand, and fail with a
  message naming the cause: *"the shared `target/debug/codescout` was built without
  `--features librarian`; rebuild with default features"*. This does not prevent the clobber
  — it converts a cryptic false-defect report into a self-explaining one. Note that switching
  to `env!("CARGO_BIN_EXE_codescout")` is **not** a fix: it resolves to the same path.
- **A separate `CARGO_TARGET_DIR` for the lean gate.** Removes the clobber entirely at the
  cost of a second full build tree (disk + a cold build the first time). No such convention
  exists in this repo today — `memory(recall)` for it returns nothing above 0.29 similarity.
- **Per-session worktrees.** The structural fix, and the one `R-129` argues is not free:
  `append_entry` refuses id allocation from a worktree, so every ledger append becomes a
  two-step with a merge between. Operator's call, not a drive-by.

## Tests added

None yet — the bug is in the test harness's own isolation, so a regression test would need
to build a lean binary into the shared path and assert the preflight message fires. Worth
writing alongside whichever fix is chosen; recording the absence rather than excusing it.

## The documented gate ENDS in the hazard state

*Added 20:2x. The observation is `git-travel-augmentation-shape`'s, who hit it from the
producing side thirty seconds before reading about it: their gate runs the lean lane last, so
`target/debug/codescout` was sitting librarian-less while they committed and walked away.
The generalisation below is what their observation implies once checked against CLAUDE.md.*

This is not an accident of two sessions happening to overlap. CLAUDE.md § *Development
Commands* specifies the gate in this order:

```
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace
cargo test --workspace --no-default-features        <- LAST
```

The lean lane is **terminal**. So a session that runs the documented gate correctly, passes
it, and stops, leaves the shared `target/debug/codescout` without the `librarian` feature —
every time, by following instructions. **The hazard state is the gate's exit state**, not a
deviation from it.

That changes the shape of the problem in three ways:

- **It is the diligent sessions that arm it.** A session that skips the lean lane never
  creates it. The more faithfully the project's own gate is followed, the more reliably the
  trap is left behind — the same inversion `reconnaissance-patterns:R-129` names for
  announcements, arriving here through a build artifact instead of a message.
- **The arming session never observes the consequence.** Its own gate passed; the failure
  lands on whoever next runs a default-features test without an intervening default build.
  So the producer gets no signal at all, which is why this went unnoticed long enough to be
  filed as a suspected feature-gating regression.
- **It is not self-healing across sessions, only within one.** A later `cargo test
  --workspace` in the *same* invocation rebuilds the bin and repairs it. The window is
  between a lean build finishing and any default-features build completing — which is
  exactly where a concurrent session's already-built test binary execs the wrong file.

### The cheap mitigation, and where it belongs

`cargo build --bin codescout` after the lean lane closes the class from the producing side
entirely, and costs one incremental build. Both sessions that have hit this are now doing it.

Verify it positively rather than assuming the build fixed it —
`./target/debug/codescout artifact --help` resolving is the check;
`./target/debug/codescout --help | grep -c artifact` returning 4 is the same check in one
line. A `cargo build` that no-ops because nothing changed looks identical to one that
repaired the binary.

The natural home for the mitigation is the gate line in CLAUDE.md itself, as a fifth step or
as a reordering that does not end on the lean lane. **Not applied here — CLAUDE.md is an
operator-owned surface and the gate's composition has been argued at length in that section;
changing it is the operator's call, not a drive-by from a bug file.**

## Workarounds

Right now, if `cli_artifact` tests fail with `unrecognized subcommand 'artifact'`:

```
cargo build --bin codescout        # restores the default-features binary
cargo test --test cli_artifact     # confirm 11/11
```

And after running the lean gate, rebuild the default binary before walking away, so the next
session does not inherit the false red.

## Resume

Decide between the preflight assertion and a separate lean `CARGO_TARGET_DIR`. If preflight:
edit `run_cmd` at `tests/cli_artifact.rs:11` to probe `--help` once (or check a
`cfg!(feature = "librarian")` guard on the test module) and produce the naming message. Then
reproduce with the deterministic single-shell recipe above and confirm the new message
appears instead of `unrecognized subcommand`.

## References

- `docs/trackers/reconnaissance-patterns.md` — `R-129`, shared-checkout false reds. This bug
  is the mechanism R-129 does not cover: every R-129 instance originates in a **source file**
  on disk, so `git status` and "ask whose dirty file this is" can reach it. This one
  originates in a **build artifact**. The tree can be pristine and the failure still fires,
  with nothing to look at.
- `docs/issues/archive/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md`
  and `docs/issues/archive/2026-08-27-cross-process-write-lock-test-passes-when-it-does-not-run.md`
  — the neighbouring class, about `target/debug/codescout`'s **existence**. This one is about
  its **feature set**, which fails in the opposite direction: those tests self-skip when the
  binary is missing, this one runs and reports a defect.
