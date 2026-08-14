---
id: '2faaf32abe069400'
kind: bug
status: investigating
title: 'BUG: every `cargo test` run leaks ~144 sqlite-vec databases into the real `~/.codescout/embeddings/` — 69,429 files / 23 GB accumulated'
owners:
- marius
tags:
- test-isolation
- sqlite-vec
- disk-leak
- env-isolation
topic: test-env-isolation
severity: high
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


### Re-census 2026-08-14 — still growing, and the writer chain is now identified

```
total files                                   75489
test-generated (_tmp* / cache-sandwich*)      75480
NOT test-generated                                9
```

Up from 69,429 / 23 GB to **75,489 / 25 GB** in one day. ~6,000 of those are from
*this* session's own gate runs — roughly eight `cargo test` invocations while fixing
unrelated bugs. The leak is active, not historical.

The nine real stores are `api.db`, `mcp-server.db`, `test.db`, `oom-verify-bk.db`,
`code-explorer.db`, `prompt-test-mcp-cli-iyok5akq.db`, `backend-kotlin.db`,
`MRV-poc.db`, `codescout.db`. So the two filename patterns partition the directory
cleanly at 99.99%, which makes a targeted cleanup safe — see *Workarounds*.

### The writer chain, without instrumentation

The Resume proposed adding a `tracing::warn!` on the fallback branch and bisecting.
Not needed — `references` answers it directly:

```
SqliteVecCodeStore::from_env   ← exactly ONE production caller
  src/retrieval/client.rs:238, inside RetrievalClient::from_env,
  in the `VectorBackend::SqliteVec` arm
```

And `VectorBackend::resolve` (`src/retrieval/code_store.rs:205-227`) defaults to
`SqliteVec` under `#[cfg(not(feature = "server-stack"))]`. **That is why the leak is
specific to the default `cargo test` lane** and why the `--features server-stack` lane
does not show it: the server lane resolves to Qdrant and never constructs the sqlite
store at all.

`RetrievalClient::from_env` has 14 call sites across `index.rs`, `agent/mod.rs`,
`config/mod.rs`, `memory/mod.rs`, `onboarding.rs`, `semantic_search.rs`, `main.rs` and
`bin/sync_project.rs`. Every test that exercises one of those tool paths against a
`tempfile::tempdir()` project mints a store in `$HOME`.
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

Not implemented, and **this bug is larger than it was filed as.** Reclassified from a
mechanical fix to one needing a design decision, for a reason worth recording.

### Why the preferred option does not reach the leak

`docs/conventions/test-env-isolation.md` prescribes **option A**: resolve env at the
edge into a struct, pass the resolved value inward (`LibrarianEnv::from_env`,
`ServerEnv::from_env` are the exemplars). Option B — `EnvGuard` + `#[serial]` — is
marked NOT VIABLE and the default test build is deliberately at **zero** `set_var`
occurrences (119 → 0, `a656f8cec220d347`). So no fix here may set
`CODESCOUT_SQLITE_DIR` from a test.

The original *Fix* section proposed giving the store an explicit `at(dir)` seam "that
tests use directly." That seam **already exists** — `from_env` is a thin wrapper over
`Self::at(dir)` — and it does not help, because **the leaking tests never construct the
store.** They call tools. The chain is:

```
test → index/memory/semantic_search/config tool
     → RetrievalClient::from_env(root)
     → SqliteVecCodeStore::from_env()      ← env read happens HERE, 3 frames deep
     → $HOME/.codescout/embeddings/<project_id>.db
```

For a test to inject a directory, the value has to travel from the *tool call* down
through `RetrievalClient` to the store — 14 call sites, several of them on the
production MCP path. That is the real shape of option A here, and it is a refactor, not
a patch.

### The decision to make

1. **Thread it properly (option A, full).** Add `sqlite_dir` to `RetrievalConfig` —
   which `RetrievalClient::from_env` already builds via
   `RetrievalConfig::from_env_and_project`, so the env read lands exactly at the edge
   the convention names — then pass it to `SqliteVecCodeStore::at`. Tests that build a
   config literally get isolation for free. Cost: the 14 call sites need to reach a
   config they mostly already have, and the tool-level tests need a way to supply one.
2. **Derive the store dir from the project root** instead of `$HOME`. Kills the leak
   outright, since test projects are temp dirs that get cleaned up. But it relocates
   every existing user's index and changes a user-scoped store into a per-project one —
   a product decision, not a test-isolation one.
3. **`#[cfg(test)]` fallback to a temp dir.** Smallest change, and it would have cut
   the measured 144-per-run figure (that was `cargo test --lib`). **Rejected as a
   knowing half-fix**: `cfg(test)` does not apply to integration tests in `tests/`,
   which link the lib compiled without it, so those would keep leaking silently. Two
   bugs closed earlier today (`4eabe442`, `4c9c23b8`) were *caused* by exactly this —
   fixing one member of a set and leaving its siblings guarded by a passing test.

Separately still worth deciding, and unchanged from the original filing: whether the
production fallback should self-limit at all. 3.2 MB preallocated per `vec0` table
makes a handful of stale ids cheap; 75,480 of them is 25 GB.
## Tests added

None yet — bug filed on discovery.

## Workarounds

**Cleanup performed 2026-08-14 (operator-authorised). `~/.codescout` went from 25 GB to
285 MB — ~24.7 GB reclaimed.**

| Deleted | Count | Freed |
|---|---|---|
| test DBs (`_tmp*`, `cache-sandwich*`) | 75,480 | ~24.4 GB |
| logs `>30d` in `~/.codescout` (newest 2026-06-30) | 11 | ~24 MB |
| `codescout/.codescout/usage.db.bak-2026-05-31` (75d) | 1 | 70 MB |
| orphaned project stores `>30d` (`oom-verify-bk.db`, `code-explorer.db`) | 2 | 309 MB |

Age was deliberately **not** the criterion for the test DBs: they are stores for
`tempfile::tempdir()` projects that no longer exist, so a 3-day-old one is exactly as
worthless as a 3-month-old one. A literal 30-day cut would have left 42,700 files /
~14 GB of known garbage. Logs and the `usage.db` backup *did* use the 30-day rule.

Untouched, deliberately: this repo's `.codescout/*.log` (12 files, all within 30 days —
active), `usage.db` itself (59.6 MB, live observability data), and
`codescout/.codescout/embeddings/` (162 MB — one `project.db` plus `lib/`, the current
per-project store, not leakage).

### The survivor check that mattered

A shallow existence check across three parent directories reported `backend-kotlin` and
`MRV-poc` as orphaned. **Both exist** — `/home/marius/work/mirela/backend-kotlin` and
`/home/marius/work/stefanini/southpole/MRV-poc`, found only by widening to
`find -maxdepth 4` over `work/`. Deleting on the shallow result would have destroyed
118 MB of live index for the very project that task #46 tracks 13 pending
`worktree_scoped_row` merges against.

Only two stores were deleted as orphans, each satisfying **both** conditions
(no project dir found at depth 4 **and** older than 30 days):
`oom-verify-bk.db` and `code-explorer.db` — the latter's root having been measured gone
earlier the same session when task #45 removed its registry entry.

Four 16 KB stubs (`api.db`, `mcp-server.db`, `test.db`,
`prompt-test-mcp-cli-iyok5akq.db`) were kept: their names match many candidate
directories, attribution is ambiguous, and the total is 64 KB.

### Cleanup is not a fix

The next `cargo test` starts refilling the directory. What the cleanup *does* buy is a
clean measurement baseline — see *Resume*.
## Resume

**The writer is identified; do not re-run the instrumentation the original Resume asked
for.** It is `RetrievalClient::from_env` → `SqliteVecCodeStore::from_env`
(`src/retrieval/client.rs:238`), the sole production caller, reached because
`VectorBackend::resolve` defaults to `SqliteVec` on the non-`server-stack` build. Found
with `references`; no tracing needed.

What remains is the **decision** among the three options under *Fix*. Option A as the
convention prescribes it is a 14-call-site refactor, not a patch. Option 3
(`#[cfg(test)]`) is the tempting one and is a trap — it fixes `--lib` and leaves
`tests/` leaking.

**Verification is now easy, because the directory is clean.** As of 2026-08-14 it holds
exactly **7 files** and 285 MB, every one attributable to a live project or a 16 KB
stub. So:

```bash
E="$HOME/.codescout/embeddings"
before=$(find "$E" -maxdepth 1 -type f | wc -l)   # 7 on a clean base
cargo test
after=$(find "$E" -maxdepth 1 -type f | wc -l)
echo "delta=$((after - before))"                  # must be 0
```

Re-baseline before trusting the old 144-per-run figure: that was measured on
`cargo test --lib` alone, and a full `cargo test` run also exercises the integration
tests in `tests/`, which is exactly the population option 3 would miss.
## References

- `src/retrieval/sqlite_code_store.rs:45-57` — `from_env`, the home-dir fallback
- `src/tools/config/tests.rs:1734` — `cache-sandwich-<pid>` id generator
- `src/tools/config/mod.rs:539-550` — `resolve_first_probe` (excluded by the control)
- `docs/conventions/test-env-isolation.md` — option A doctrine; option B is banned
- `docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md` — same class, lock files
- `docs/issues/2026-07-XX` sibling: `/tmp` probe rows leaking into the shared global catalog
