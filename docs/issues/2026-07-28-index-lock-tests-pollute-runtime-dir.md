---
id: '1d2cc991f2ec2b1a'
kind: bug
status: open
title: index_lock and lsp/mux tests write lock files into the real per-user runtime dir and never clean up
tags:
- retrieval
- index-lock
- test-isolation
- diagnostics
closed: ''
opened: 2026-07-28
owner: marius
related:
- docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md
- docs/issues/2026-07-27-test-env-isolation-doc-prescribes-rejected-remedy.md
severity: low
---

# BUG: index_lock tests write lock files into the real per-user runtime dir and never clean up

## Summary

The `index_lock` test module builds its project ids with `unique_project()`, which
embeds the test process's PID and thread id, then calls the production `lock_path()`
— which resolves against the **real** `per_user_runtime_dir()`. Every `cargo test`
run therefore leaks ~6 permanent 8-byte files into `$XDG_RUNTIME_DIR`, and nothing
ever removes them. Found while diagnosing an unrelated GPU spike: the runtime dir
held **203** lock files, 201 with dead holders, which buried the one lock that
mattered.

## Symptom (Effect)

```
$ ls /run/user/1000/codescout-index-*.lock | wc -l
203
$ # holders still alive:
alive=2 dead=201
$ # distinct PIDs recorded inside all 203 files:
45
```

One file additionally carries a mutation-test artifact rather than a PID:

```
$ tr '\n' '|' < /run/user/1000/codescout-index-b773983c06aa5772.lock
2025853|stale-tail-that-must-be-truncated|
```

That string is the fixture from `preexisting_lock_file_does_not_block`
(`src/retrieval/index_lock.rs:178`). Its presence *undamaged* means it survives from
a run where `acquire`'s `set_len(0)` was absent — i.e. the deliberate mutation-testing
run that proved the test can fail. The mutation artifact was never cleaned up.

## Reproduction

```
ls /run/user/1000/codescout-index-*.lock | wc -l    # note the count
cargo test --lib index_lock
ls /run/user/1000/codescout-index-*.lock | wc -l    # +6
```

Six, not five, because `different_projects_do_not_contend`
(`src/retrieval/index_lock.rs:143-148`) calls `unique_project` twice.

## Environment

Linux, `experiments` @ `d531ee76`. `XDG_RUNTIME_DIR=/run/user/1000` (tmpfs). Any
platform where `per_user_runtime_dir()` resolves to a real directory — which is all
of them; the non-XDG fallback is `std::env::temp_dir()/codescout-<uid>`
(`src/socket_discovery.rs:17-29`).

## Root cause

`lock_path` (`src/retrieval/index_lock.rs`) has no injection seam:

```rust
pub fn lock_path(project_id: &str) -> PathBuf {
    // ... sha256(project_id) ...
    crate::socket_discovery::per_user_runtime_dir()
        .join(format!("codescout-index-{}.lock", &digest[..16]))
}
```

The directory is resolved internally, so a test cannot redirect it. The test module
worked around the *collision* problem (concurrent `cargo test` threads sharing a lock
file) by making the project id unique per test-thread:

```rust
fn unique_project(tag: &str) -> String {
    format!("test-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id())
}
```

That solves contention and *causes* the leak: a unique project id means a unique
`sha256` means a **new filename every run**, so files accumulate instead of being
reused. Nothing unlinks them — no test cleans up, and `IndexLock`'s drop
deliberately does not unlink (correctly: unlinking a flock'd path races, since a
second process can hold the fd of a file a third has already replaced).

The arithmetic confirms the mechanism: 45 distinct PIDs recorded across 203 files is
~4.5 files per process, and PID is part of the project id — so those 45 are 45
distinct *test binary invocations*, not 45 indexers.

## Evidence
### MEASURED 2026-07-28 — controlled run, replaces the estimates above

The first pass of this bug estimated "~6 files per run" from reading the test
module. Measured instead: delete every lock file, run `cargo test` once, count.

| | baseline | after one run | delta |
|---|---|---|---|
| `codescout-index-*.lock` | 0 | 7 | **+7** |
| `codescout-rust-mux-*.lock` | 468 | 486 | **+18** |

Gate on the same run: 3430 passed, 0 failed, 44 ignored, 18 binaries.

**Index locks: 7, not 6.** All seven carry the same PID (3337647 — one lib test
binary). Six come from `index_lock`'s own tests as described below; the seventh is
`sync_project_holds_index_lock_for_its_full_duration` (`src/retrieval/sync.rs:736`),
which drives `sync_project` and so hits the production `acquire` at
`src/retrieval/sync.rs:234`. The estimate missed it because it scoped the count to
one module. Confirmed that the fixed-id tests contribute nothing by hashing
`some-project-for-siting-check`, `project-one` and `project-two` and matching none
of the seven filenames — `lock_path` alone creates no file, only `acquire` does.

**A second module leaks the same way: `lsp/mux`.** 16 of the 18 new mux locks were
stamped inside a single second (02:52:36.236 to 02:52:36.743), three seconds after
the index locks. That timing rules out the 13 ambient Claude sessions whose servers
also create mux locks — ambient traffic does not land 16 files in one tick.
`lock_path_for_workspace` (`src/lsp/mux/mod.rs:23-29`) resolves
`per_user_runtime_dir()` internally exactly as `lock_path` does, and
`claim_mux_lock` (`src/lsp/manager.rs:483`) creates the file. Keyed on
`workspace_hash(workspace_root)`, so a test using a fresh `TempDir` as the workspace
root mints a new filename every run.

Two hypotheses for the mux producer were tested and rejected before the timing
evidence settled it:

1. `get_or_start_via_mux_surfaces_wedged_error_when_flock_held_socket_absent`
   (`src/lsp/manager.rs:2347`) does textbook-exactly the leaking thing —
   `lock_path_for_workspace("rust", tempdir)` then `std::fs::write(&lock_path, b"")`,
   which matches the observed 0-byte size. **Rejected:** it is `#[ignore]`d, so it
   does not run in a default `cargo test`.
2. `src/lsp/mux/coherence_rust.rs`, which calls `get_or_start("rust", &root,
   Some(true))`. **Rejected as sufficient:** the module holds exactly one test
   (`two_agents_coherent_after_edit`), which cannot account for 16 files.

The actual producer is not yet pinned to a test name. Whoever fixes this should
identify it by bisecting `cargo test <filter>` against the file count rather than by
reading — two readings already failed.

### Corollary worth preserving: absence of a lock file is evidence

Because lock files are never unlinked, the *absence* of one proves no `sync_project`
ran for that project since the directory was last cleared. That property was
load-bearing in a live diagnostic on 2026-07-28: 900+ dense-embed requests were
attributed to search/librarian traffic rather than indexing precisely because no
index lock existed for the project in question. **A fix must preserve this** — a
scratch directory for tests does; unlink-on-drop would destroy it. That is a second,
independent reason to reject unlink-on-drop beyond the fd race in Hypothesis 3.

### Corrects an inference made during this investigation

The first read of `203 files / 45 distinct PIDs` was "one process locking several
projects" — plausible for an 8-project workspace, and wrong. Reading
`unique_project` is what settled it: PID appears *inside* the hashed project id, so
distinct PIDs count test processes, and per-run-unique names are the leak mechanism.
Recorded because the wrong reading was the more natural one.

### Concrete harm: it degraded a live diagnostic

Investigating GPU at 98% / 86 degC, the runtime lock dir is the correct place to ask
"who is indexing?". It returned 203 candidates for a question with one answer. The
`grep -l` that found the real holder only worked because the PID was already known
from `pgrep`; the lock dir contributed nothing and cost a detour.

### The stale file contradicts the operator-facing error text

`acquire`'s error says the lock file's *"first line is the holder's PID"*. For
`codescout-index-b773983c06aa5772.lock` line 1 is `2025853`, a PID belonging to no
indexer. An operator following the error message would chase a dead process.

## Hypotheses tried

1. **Hypothesis:** 203 files means ~203 real projects have been indexed (a
   multi-project workspace legitimately needs one lock per project).
   **Test:** count distinct PIDs inside the files, then read `unique_project`.
   **Verdict:** rejected — the PID is part of the hashed project id, so the files
   are per-test-process, and each run mints new names.
2. **Hypothesis:** the lock is per-invocation rather than per-project, so it never
   excluded anything and the measured ~3x throughput gain had another cause.
   **Test:** `grep -l '^<pid>$'` against the live indexer, and re-read `lock_path`.
   **Verdict:** rejected — `lock_path` hashes only `project_id`, and the live CLI
   indexer held exactly one lock (`...ee26de4c61f6f20e`). Production ids are stable;
   only the *test* ids are unique-per-run.
3. **Hypothesis:** `IndexLock::drop` should unlink the file.
   **Verdict:** rejected as the fix — unlink-on-drop races with a concurrent
   `acquire` that already holds an fd on the old inode, which would let two runs
   believe they hold the lock. Not unlinking is correct; the tests are what need a
   scratch directory.

## Fix

Not implemented. Add a directory injection seam and point the tests at a `TempDir`:

```rust
pub fn lock_path_in(dir: &Path, project_id: &str) -> PathBuf { /* hash + join */ }
pub fn lock_path(project_id: &str) -> PathBuf {
    lock_path_in(&crate::socket_discovery::per_user_runtime_dir(), project_id)
}
pub fn acquire_in(dir: &Path, project_id: &str) -> Result<IndexLock> { /* ... */ }
pub fn acquire(project_id: &str) -> Result<IndexLock> {
    acquire_in(&crate::socket_discovery::per_user_runtime_dir(), project_id)
}
```

**Do not fix this with an env-var override.** A `CODESCOUT_INDEX_LOCK_DIR` read via
`std::env::set_var` in tests reintroduces the exact unsound pattern that bit this
project twice in one session — see
`docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` and
`docs/issues/2026-07-27-test-env-isolation-doc-prescribes-rejected-remedy.md`.
Parameter injection is the pattern the codebase already uses for the embedder's
test overrides; mirror that.

With a `TempDir` the tests also no longer need `unique_project`'s PID/thread
mangling for isolation — a per-test temp dir is the isolation — though keeping a
readable tag is fine.

Housekeeping for the existing 203 files: they live in tmpfs and vanish on logout, so
no migration is needed. `rm /run/user/1000/codescout-index-*.lock` is safe while no
indexer runs, and harmless even if one does (the holder keeps its fd; only the name
disappears).

## Tests added

None yet — no fix. When fixed, assert the seam rather than the leak: a test using
`acquire_in(tempdir.path(), ...)` must create no file under
`per_user_runtime_dir()`. That is the assertion a future refactor can actually
violate; counting files in a shared directory would be flaky under concurrent
`cargo test`.

## Workarounds

`rm /run/user/1000/codescout-index-*.lock` between diagnostic sessions. To find the
real holder, prefer `pgrep -af 'codescout index'` over listing the lock dir until
this is fixed.

## Resume

Add `lock_path_in` / `acquire_in` to `src/retrieval/index_lock.rs`, delegate the
existing two functions to them, and switch the five tests in that module to a
`tempfile::TempDir`. Then run `cargo test --lib index_lock` twice and confirm
`ls /run/user/1000/codescout-index-*.lock | wc -l` is unchanged across both runs.

## References

- `src/retrieval/index_lock.rs` — `lock_path`, `acquire`, and the test module
- `src/retrieval/index_lock.rs:110-117` — `unique_project`, the leak mechanism
- `src/retrieval/index_lock.rs:143-148` — the test calling `unique_project` twice
- `src/socket_discovery.rs:17-29` — `per_user_runtime_dir` and its non-XDG fallback
- `src/retrieval/sync.rs:234` — the production `acquire` call site
