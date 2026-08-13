---
id: '2faaf32abe069400'
kind: bug
status: open
title: 'BUG: every `cargo test` run leaks ~144 sqlite-vec databases into the real `~/.codescout/embeddings/` — 69,429 files / 23 GB accumulated'
owners:
- marius
tags:
- test-isolation
- sqlite-vec
- disk-leak
- env-isolation
topic: test-env-isolation
---

## Summary

Tests that construct a `SqliteVecCodeStore` without setting `CODESCOUT_SQLITE_DIR`
fall back to the real `~/.codescout/embeddings/` and create a `<project_id>.db` per
temp project. Nothing removes them. On this host the directory holds **69,429 files
totalling 23 GB**, of which **69,420** are test-generated (`_tmp*` tempdir basenames
and `cache-sandwich-*` ids). A single `cargo test --lib` run adds **144** files.

Unbounded, silent, and on the developer's home filesystem rather than a temp dir.

## Symptom (Effect)

```
file count: 69429
total size: 23G	/home/marius/.codescout/embeddings/
tmp-pattern dbs: 69420
```

Per-run growth, measured directly around one `cargo test --lib`:

```
before=69429
test result: ok. 3524 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out; finished in 11.44s
after=69573 delta=144
```

Representative entries — note most are the empty-schema size (16384 bytes), i.e.
a store was opened and its schema initialized but nothing was ever written:

```
-rw-r--r-- 1 marius marius     16384 Jul 19 19:32 cache-sandwich-1005757.db
-rw-r--r-- 1 marius marius     16384 Jul 10 07:21 _tmpZZRMEO.db
-rw-r--r-- 1 marius marius     16384 Jul 10 07:21 _tmpZZRMEO.memories.db
-rw-r--r-- 1 marius marius   3203072 Jul 19 21:37 _tmpzZXTtG.memories.db
```

## Reproduction

Commit `6e1fa4fa`, branch `feat/local-onnx-query-path` (not branch-specific — the
timestamps span June through August, so this predates the branch by months).

```
B=$(ls -1 ~/.codescout/embeddings/ | wc -l)
cargo test -q --lib
A=$(ls -1 ~/.codescout/embeddings/ | wc -l)
echo "delta=$((A-B))"
```

Measured 2026-08-13: `delta=144`.

## Environment

Linux 7.1.5-zen1-2-zen. Any developer machine running the suite with
`CODESCOUT_SQLITE_DIR` unset — which is the default, since nothing in the repo sets it.
CI runners are ephemeral, so this is invisible there and only bites local developers.

## Root cause

`SqliteVecCodeStore::from_env` (`src/retrieval/sqlite_code_store.rs:45-57`) resolves its
data directory as: `CODESCOUT_SQLITE_DIR` if set and non-empty, else
`home_dir()/.codescout/embeddings`. The fallback is correct for production — a
user-scoped server should keep its stores in the user's home — but it is also what
tests get, because no test harness sets the variable.

Each per-project operation then opens `<dir>/<project_id>.db`, where `project_id` is
derived from the project directory's basename. Tests build temp projects
(`tempfile::tempdir()` → `_tmpXXXXXX`) or synthesize ids (`cache-sandwich-<pid>`,
`src/tools/config/tests.rs:1734`), so **every run mints fresh, never-colliding
filenames**. Deletion never happens: the tempdir's `Drop` removes the *project*
directory under `/tmp`, but the database lives in `$HOME` and is not owned by that
guard.

This is precisely the failure mode `docs/conventions/test-env-isolation.md` prescribes
option A against — "resolve env at the edge into a struct, pass inward". `from_env`
reads the environment *inside* the store constructor, so a test has no seam to inject
an isolated directory through short of setting a process-global env var (the banned
`EnvGuard` pattern).

Measured 2026-08-13: file counts and the 144-file delta are from the commands above,
run on this host. The `resolve_first_probe` path was checked and **excluded** — running
`index_status_cache_serves_stale_then_refreshes` alone produced `delta=0`, so the
`cache-sandwich-*` files come from a different consumer of that id, not from that test's
in-memory cache calls.

## Evidence

### Directory census

69,429 files, 23 GB, 69,420 matching `^(_tmp|cache-sandwich)`. Timestamps run from
June through 2026-08-13, so this has been accumulating for at least three months.

### Per-run delta

144 new files from one `cargo test --lib` (3524 tests, all passing). At that rate the
observed 69k population corresponds to roughly 480 suite runs.

### Negative control

`cargo test --lib index_status_cache_serves_stale_then_refreshes` → `delta=0`. The
named test is not the writer; the id-shape is shared with whatever is.

## Hypotheses tried

1. **Hypothesis:** `index_status_cache_serves_stale_then_refreshes` creates the
   `cache-sandwich-*.db` files, since it mints exactly that id shape.
   **Test:** run that single test and count before/after.
   **Verdict:** rejected — `delta=0`. `resolve_first_probe`
   (`src/tools/config/mod.rs:539-550`) only touches an in-memory cache.

2. **Hypothesis:** the files are production state from real projects, not test leakage.
   **Test:** classify by name — count entries matching `_tmp*` / `cache-sandwich-*`.
   **Verdict:** rejected — 69,420 of 69,429 are test-shaped names.

## Fix

Not yet implemented. Two candidate levels:

**Test-harness level (narrow, safe):** have the test helpers that build stores point
`CODESCOUT_SQLITE_DIR` at a per-test temp directory. Must not use the banned
`EnvGuard` + `#[serial]` pattern (`docs/conventions/test-env-isolation.md` marks it
NOT VIABLE) — which is the argument for the next option.

**Constructor level (option A, preferred):** give `SqliteVecCodeStore` an explicit
`at(dir)` seam that tests use directly — it already exists, `from_env` merely wraps it
— and audit which test paths reach `from_env` instead. This removes the env read from
the test path entirely rather than racing it.

Separately worth deciding: whether the production fallback should self-limit at all
(the 3.2 MB preallocation per `vec0` table means a handful of stale ids is cheap, but
69k is not).

## Tests added

None yet — bug filed on discovery.

## Workarounds

- Reclaim the space now: the `_tmp*` and `cache-sandwich-*` entries are inert test
  residue and safe to delete. Verify the classification first, since real project
  stores share the directory.
- Set `CODESCOUT_SQLITE_DIR` to a scratch path when running the suite locally.

## Resume

Identify the actual writer: instrument or bisect which test paths reach
`SqliteVecCodeStore::from_env` (`src/retrieval/sqlite_code_store.rs:45-57`) during
`cargo test --lib`, by adding a temporary `tracing::warn!` on the home-dir fallback
branch and running the suite once. The 144-per-run delta gives a precise target — the
fix is verified when that delta reaches 0.

## References

- `src/retrieval/sqlite_code_store.rs:45-57` — `from_env`, the home-dir fallback
- `src/tools/config/tests.rs:1734` — `cache-sandwich-<pid>` id generator
- `src/tools/config/mod.rs:539-550` — `resolve_first_probe` (excluded by the control)
- `docs/conventions/test-env-isolation.md` — option A doctrine; option B is banned
- `docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md` — same class, lock files
- `docs/issues/2026-07-XX` sibling: `/tmp` probe rows leaking into the shared global catalog

