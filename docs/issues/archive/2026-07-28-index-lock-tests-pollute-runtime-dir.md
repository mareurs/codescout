---
id: '5d41159bfe9ca76e'
kind: bug
status: fixed
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

Because lock files are never unlinked, the *presence* of one is durable proof that
`sync_project` ran for that project since the directory was last cleared — and it
names the project, since the filename is `sha256(project_id)[..16]`.

This was load-bearing in a live diagnostic on 2026-07-28, but only on the second
look, and the first look is the instructive part. 900+ flat dense-embed requests
(~30/min for ~28 min) were initially attributed to search/librarian traffic rather
than indexing, on the stated grounds that "no index lock exists for it". That was
wrong. A lock file *did* exist: `codescout-index-c2f5622c4ae66298.lock`, created
09:25:06, i.e. at the start of the traffic window, and
`sha256("MRV-poc")[..16] == c2f5622c4ae66298` identifies the project exactly. The
load was a ~28-minute index of `/home/marius/work/stefanini/southpole/MRV-poc`.

The observation failed, not the reasoning. The check enumerated only holders passing
`kill -0`; the indexer PID had exited seconds earlier, so a real, informative file
printed nothing and was then excluded from a count reused further down. **A liveness
filter is the wrong lens for a forensic artifact whose whole value is outliving its
holder.** List the files first, resolve liveness second.

A parallel mis-attribution in the same diagnostic: the busy process was identified by
sorting all servers on cumulative CPU, which nominated a 12.9-hour-old server on an
unrelated project. Cumulative CPU cannot isolate a 28-minute batch, and the actual
indexer was a short-lived child that had already exited and so never appeared in the
table at all.

**A fix must preserve the durability** — a scratch directory for tests does;
unlink-on-drop would destroy the one artifact that identified this run. That is a
second, independent reason to reject unlink-on-drop beyond the fd race in
Hypothesis 3. It also argues the opposite of what the noise suggests: the
production files are worth *keeping*, and only the test-generated ones should stop
being written — which is exactly what parameter injection achieves.

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

Shipped on `experiments` 2026-07-28. Two modules, two different correct seams —
the distinguishing question is *who calls the path helper*.

**`src/retrieval/index_lock.rs` — parameter injection.** Added
`lock_path_in(dir, project_id)` and `acquire_in(dir, project_id)`; `lock_path` and
`acquire` delegate, passing `per_user_runtime_dir()`. The five leaking tests use a
fresh `TempDir` via `acquire_in`. `unique_project` is deleted: with a scratch
*directory* per test, isolation no longer depends on a per-run-unique *name*, so the
ids are plain literals and there is nothing left to accumulate.

**`src/retrieval/sync.rs` — the 7th file.** That test drives production
`sync_project`, so no test-local seam reaches it. Added
`SyncOpts::index_lock_dir: Option<PathBuf>` (`None` at every production call site,
including `src/bin/sync_project.rs`). The load-bearing line became a `match`, still
bound to `_index_lock`, so the `let _ =` mutation that
`sync_project_holds_index_lock_for_its_full_duration` guards stays detectable. The
test's lock dir is separate from its workspace root — otherwise the lock file lands
inside the tree being indexed.

**`src/lsp/mux/mod.rs` — a `cfg(test)` seam, deliberately not injection.** Here the
callers that create the files are *production* code: 17 lib tests reach
`claim_mux_lock` through `LspManager::get_or_start`. A parameter would put a test
concern in the LSP manager's public signature and require all 17 tests to opt in
individually. `mux_dir()` returns `per_user_mux_dir()` in production and a
per-process scratch subdirectory *inside* it under `cfg(test)` — nested rather than
in bare `temp_dir()` so the `0o700` protection still applies.

**Plus a sweep, because relocation alone was only half a fix.** Each scratch dir
still held 17 files, so total inodes per run were unchanged — only the shared
directory got clean. `sweep_dead_test_mux_dirs` removes scratch dirs whose owning
PID has exited, bounding the total to the number of *concurrently running* test
processes. `pid_is_alive` uses `kill(pid, 0)` and treats `EPERM` as alive (the
process exists but is not ours). Non-unix gets a no-op: no portable `kill(pid, 0)`,
so keep the per-process dirs rather than risk removing a live one.

That sweep is the only `read_dir` over the runtime directory in the tree — the very
pattern a blast-radius scout greps for. It carries a comment saying so: the hit is
*cleanup, not discovery* (it resolves nothing, no caller depends on what it finds),
so a future relocation stays safe. See `docs/trackers/reconnaissance-patterns.md`
R-45.

**Measured, against a recorded baseline:**

| | before | after |
|---|---|---|
| `codescout-index-*.lock` per run | +7 | **0** |
| `codescout-rust-mux-*.lock` in the shared dir per run | +18 | **+1** |
| scratch dirs | grew every run | **bounded at 1** |

Gate: 18 binaries, 3433 passed, 0 failed, 44 ignored; `cargo clippy --all-targets
-- -D warnings` clean.

The residual +1 is the documented limit: `cfg(test)` covers unit tests only, and
`tests/*.rs` link the lib built without it.

Per CLAUDE.md the **master-side** SHAs go here after cherry-pick — the
`experiments`-side originals orphan on rebase and are deliberately not recorded.
## Tests added

Three new tests, all asserting the **seam** rather than the symptom. Counting files
in a shared directory would be flaky: a concurrent `cargo test`, or a genuine index
run, can add one at any moment — which is exactly how the original diagnostic went
wrong.

- `acquire_in_does_not_touch_the_real_runtime_dir`
  (`src/retrieval/index_lock.rs`) — asserts one specific path was never created and
  that the lock is sited in the injected dir. Fails if `acquire_in` is ever reverted
  to resolving the directory itself.
- `socket_and_lock_share_a_parent_inside_the_per_user_dir`
  (`src/lsp/mux/mod.rs`) — build-agnostic. The socket and lock MUST share a parent:
  `get_or_start_via_mux` diagnoses "lock held but socket absent" as the wedged
  state, and splitting them across directories breaks that diagnosis. Also pins the
  dir inside `per_user_mux_dir()`, so re-siting to bare `temp_dir()` fails.
- `test_builds_redirect_the_mux_dir_away_from_the_shared_one`
  (`src/lsp/mux/mod.rs`) — asserts the redirect is actually active in the build where
  it is supposed to be.

The five rewritten `index_lock` tests keep their original assertions, including
`preexisting_lock_file_does_not_block`'s two-mutation-killing planted content (live
PID + a longer tail, which pins both the liveness-check and the missing-`set_len(0)`
mutations).

Not covered by a test: the sweep itself. Asserting "a dead PID's dir was removed"
requires spawning and reaping a process whose PID then stays unreused, which is
timing-dependent and flaky by construction. The `dirs` 2 -> 1 transition was
verified by direct measurement instead, and is recorded in the table above.
## Workarounds

`rm /run/user/1000/codescout-index-*.lock` between diagnostic sessions. To find the
real holder, prefer `pgrep -af 'codescout index'` over listing the lock dir until
this is fixed.

## Resume

N/A — fixed and verified. Two follow-ups, neither blocking:

1. `peer_socket_path_for_workspace` / `peer_lock_path_for_workspace`
   (`src/socket_discovery.rs:43-56`) resolve the same real directory with the same
   latent exposure. Dormant only because no `codescout-peer-*` files exist on disk,
   so it is not leaking today. If the peer path ever gets test coverage that drives
   it, apply the same treatment. Logged as the third instance in
   `docs/trackers/bug-fix-session-log.md` W-25.
2. Archive this file only after the fix reaches `master`
   (`git branch --contains <sha>`), per CLAUDE.md — not on this status flip.
## References

- `src/retrieval/index_lock.rs` — `lock_path`, `acquire`, and the test module
- `src/retrieval/index_lock.rs:110-117` — `unique_project`, the leak mechanism
- `src/retrieval/index_lock.rs:143-148` — the test calling `unique_project` twice
- `src/socket_discovery.rs:17-29` — `per_user_runtime_dir` and its non-XDG fallback
- `src/retrieval/sync.rs:234` — the production `acquire` call site
