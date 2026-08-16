---
id: '2785179d2ae84664'
kind: bug
status: fixed
title: 'BUG: every `cargo test` run leaks ~144 sqlite-vec databases into the real `~/.codescout/embeddings/` — 69,429 files / 23 GB accumulated'
owners:
- marius
tags:
- test-isolation
- sqlite-vec
- disk-leak
- env-isolation
topic: test-env-isolation
closed: 2026-08-16
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

### Re-measured 2026-08-16 — the per-run figure, corrected, and decomposed

The Resume asked for a re-baseline against a full `cargo test` rather than the
original `--lib`-only 144. Done, and the decomposition changes an argument in the
*Fix* section:

| scope | files created per run |
|---|---|
| `cargo test --lib` | **132–144** |
| `cargo test --test '*'` (every target in `tests/`) | **4** |
| full `cargo test --workspace --no-fail-fast` | **~148** |

**A first reading of 296 was wrong and is retracted.** It was taken in a window
that overlapped a test run from a second session sharing this checkout, so ~148
of those files were not created by the measured run at all. It was caught by a
behaviour-preservation check rather than by suspicion: the `resolve_sqlite_dir`
extraction should have changed nothing, so a delta that halved on the next run
was a contradiction that had to be resolved. The decomposition above is the
resolution — `132 + 4 ≈ 148`, and 296 is the outlier.

Why it matters: it re-rates option 3 under *Fix* from covering ~49% of the test
leak to covering ~97%. That does **not** revive it — see the amended note there —
but the arithmetic previously offered against it was wrong by roughly a factor of
four, and an argument that leans on a bad number should not be left standing even
when its conclusion survives.

**Standing hazard for anyone re-measuring this:** file counts under
`~/.codescout/embeddings/` are a *machine-wide* signal, not a per-run one. Any
concurrent `cargo test` — another session, another checkout, a background job —
lands in the same directory. Measure with nothing else running, or decompose and
check the parts sum.

### The basename collision — the same design, without any tests involved

`ActiveProject::project_id()` returns `config.project.name`
(`src/agent/mod.rs:311`), and when a project has no config file of its own that
name defaults to the root's **directory basename** (`src/config/project.rs:489`). The
store path is `$HOME/.codescout/embeddings/<project_id>.db`.

So the store is a single global namespace keyed by directory basename. Two
projects named `api` on one machine — `~/work/a/api` and `~/work/b/api`, or any
two monorepo siblings — resolve to the **same database file**, and because the
`project_id` column inside it is that same basename, their rows are
indistinguishable once written. One project's chunks can be served as another's
search results.

This is not a test-isolation problem and no fix aimed only at tests touches it.
It has stayed invisible for the same reason the leak grew: test tempdirs
(`_tmpZZRMEO`) never collide, so the population that made the directory huge is
exactly the population that cannot exhibit the bug. **Inferred from the two cited
lines — not reproduced.** The reproduction is cheap and should be run before the
fix is chosen: index two same-basename projects, then search one for a symbol
that exists only in the other.

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

**FIXED — `experiments:2579e2bd`** (groundwork in `experiments:faae7892`),
not-yet-on-master. Option 2 was chosen: the store directory derives from the
project root.

```
CODESCOUT_SQLITE_DIR (operator override)  ->  that
a project root                            ->  <root>/.codescout/embeddings
no root at all                            ->  <home>/.codescout/embeddings
```

One resolver, `resolve_sqlite_dir` in `src/retrieval/config.rs`, reached through
`RetrievalConfig::sqlite_dir`. Neither store reads the environment any more; both
are handed a directory.

**Measured: 8452 → 8452 files across a full `cargo test --workspace
--no-fail-fast`.** Zero is also the one direction a shared machine cannot fake —
a concurrent writer could only add files — so if anything the number is
conservative.

This closes all three problems, not only the one in the title. Test tempdirs take
their stores with them; a deleted project takes its index; and per-root paths
cannot collide, which closes the basename collision recorded under *Evidence*.

**The 57-file step that nearly shipped as done.** Routing only the *code* store
through the config took the per-run delta from ~148 to 57 with a green suite.
`SqliteVecSemanticMemoryStore::from_env` turned out to be a verbatim twin of the
constructor just deleted — same env var, same fallback, differing only in the
`.memories.db` suffix — and it kept writing into `$HOME`. That is precisely the
sibling-set trap this file cites `4eabe442` and `4c9c23b8` for, reached from the
other direction: not by choosing a half-fix, but by *completing* one and stopping
at the first green suite. The only thing that caught it was requiring the delta to
reach zero rather than "much better". A DRY gate now asserts the env var is read
in exactly one place.

Folding the memory store in also closed a split that its Qdrant arm's own comment
had already flagged in review: that arm resolved a project root, the sqlite arm
was env-only.

**Safety in other people's repositories.** The stores now sit inside the user's
tree. This repo's `.gitignore` covers `.codescout/embeddings/`; nobody else's
does. `open_conn` therefore drops a self-ignoring `.gitignore` (`*`) into the
directory on creation — best-effort, never clobbering an existing file — so a
regenerated multi-megabyte index cannot surface in someone's `git status`.

**Migration — sqlite-vec backend only.** Stores written under the old default are
orphaned in `$HOME` and each project re-indexes once. That was the accepted
trade. The stale directory is then safe to delete — 8,452 files, 2.8 GB at last
count.

**It does not affect the Qdrant server stack at all**, which is worth stating
because it also bounds what could be verified. `VectorBackend::resolve()` returns
`Qdrant` whenever `server-stack` is compiled in (the `cargo rb` alias, i.e. this
machine's live MCP binary); the sqlite path is what a plain lean
`cargo build --release` produces. So on a server-stack host nothing exercises
`sqlite_dir` in normal use, the `$HOME` counter cannot move whatever you do, and a
zero delta measured there is evidence of nothing. The zero recorded above came
from `cargo test`, whose fixtures do drive the sqlite path — that measurement
stands. What has NOT been observed is a live MCP session indexing into
`<root>/.codescout/embeddings/`; doing so needs a lean build or
`CODESCOUT_VECTOR_BACKEND=sqlite-vec`.

The original analysis and the rejected options are kept below, unedited.

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

**Groundwork is done — `experiments:faae7892`.** `RetrievalConfig` now carries
`sqlite_dir`, resolved at the edge by a pure `resolve_sqlite_dir(Option<String>)`
beside `parse_rerank_opt_in`; `src/retrieval/client.rs` builds the store with
`::at(config.sqlite_dir.clone())`; `SqliteVecCodeStore::from_env` is deleted.
This is option A's *mechanism*, and it deliberately changes no behaviour — the
fallback is still `$HOME`, so the leak is untouched.

One fear in the original framing turned out to be unfounded: this was called a
14-call-site refactor, and it was five files. All 11 production callers of
`RetrievalClient::from_env` already pass `root`, and `from_env_and_project`
already accepts it. The parameter was threaded; only the field was missing.

What remains is one question: **what should the fallback be when
`CODESCOUT_SQLITE_DIR` is unset?** It is now a one-line change in one function.

1. **Keep `$HOME` (status quo).** Zero disruption. Leaves ~148 files per test
   run, unbounded production growth, and the basename collision above.
2. **Derive from the project root** (`<root>/.codescout/embeddings/`). Fixes all
   three at once: test tempdirs take their stores with them when they are
   removed, a deleted project takes its index, and per-root paths cannot
   collide. Consistent with the per-project `.codescout/` directory that already
   holds this project's memories and workspace config. Cost: existing users' indexes are
   orphaned in `$HOME` and every project re-indexes once. That is a product
   decision, which is why it is not taken here.
   - A migration shim (*use the legacy `$HOME` file when it exists, otherwise the
     new location*) would avoid the re-index, but keeps two resolution paths
     indefinitely and leaves the collision alive for every project that predates
     it. Worth naming; not obviously worth taking.
3. **`#[cfg(test)]` fallback to a temp dir.** ~~Would have cut the measured
   144-per-run figure~~ — amended 2026-08-16: it would cover roughly **97%** of
   the test leak, not the ~49% implied by the retracted 296 figure. Still
   rejected, on two grounds that do not depend on the arithmetic: `cfg(test)`
   does not reach the integration targets in `tests/`, so those keep leaking
   silently behind a green suite — the shape that caused `4eabe442` and
   `4c9c23b8` — and it does nothing at all for the unbounded production growth
   or the basename collision, both of which exist with no tests running.

Option 2 is the only one that addresses the collision, and the collision is the
part of this bug that can corrupt a user's search results rather than merely
fill a disk.
## Tests added

Nine, in `src/retrieval/config.rs` (`sqlite_dir_tests`) and `src/sqlite_vec_ext.rs`
(`self_ignore_tests`). None mutates process env — the resolver is pure over
`(Option<String>, Option<&Path>)`, which is the whole point of moving it to the
edge.

- `an_explicit_value_wins` — the operator override beats the project root.
- `an_empty_value_is_treated_as_unset` — `CODESCOUT_SQLITE_DIR=` is a shell idiom
  for clearing a variable; taken literally it would resolve the store to the
  process CWD, silently and differently per invocation.
- `the_default_is_under_the_project_root`.
- `the_fallback_is_the_users_home_directory` — rootless callers only, now the sole
  surviving path to `$HOME`.
- `two_projects_with_the_same_basename_get_different_stores` — the collision
  regression. It asserts the shared-basename precondition first, so a later edit
  to the fixture cannot make it vacuously pass.
- `the_sqlite_dir_env_var_is_read_in_exactly_one_place` — DRY gate, needle
  assembled character-wise. Added *because* two verbatim readers already existed
  and only measurement found the second.
- Three on `write_self_ignore`: a fresh directory gets a bare `*`; a pre-existing
  `.gitignore` is left alone; an unwritable path does not panic, since
  housekeeping must never fail the operation the caller actually asked for.
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

N/A — fixed and verified on `experiments` (`2579e2bd`, groundwork `faae7892`),
gate green, regression tests in place, per-run delta measured at zero.

One claim in this file remains **inferred rather than reproduced**: the basename
collision under *Evidence*. The fix removes the condition that would let it be
observed, so it is now unfalsifiable in place — recorded here rather than quietly
dropped. Anyone wanting the demonstration must run it against a commit before
`2579e2bd`: two projects sharing a directory basename and having no config of
their own, indexed, then search one for a symbol that exists only in the other.
## References

- `src/retrieval/sqlite_code_store.rs:45-57` — `from_env`, the home-dir fallback
- `src/tools/config/tests.rs:1734` — `cache-sandwich-<pid>` id generator
- `src/tools/config/mod.rs:539-550` — `resolve_first_probe` (excluded by the control)
- `docs/conventions/test-env-isolation.md` — option A doctrine; option B is banned
- `docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md` — same class, lock files
- `docs/issues/2026-07-XX` sibling: `/tmp` probe rows leaking into the shared global catalog
