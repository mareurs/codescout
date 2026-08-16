---
id: d4ab34c71d668020
kind: bug
status: fixed
title: 'BUG: test env-access UB — non-#[serial] env writers race build_tool_context''s ~17 env reads (disjoint serial_test vs ENV_LOCK)'
closed: 2026-07-13
opened: 2026-07-13
owner: marius
severity: medium
---

# BUG: test env-access UB — non-#[serial] env writers race build_tool_context's env reads

## Status

**fixed (2026-07-13)** — by *deleting* the env writes, not coordinating them. Branch `experiments`.

The originally-chosen approach (option 1, "unify the locks") was attempted first and proven **not viable**. The reasoning is kept below, because it is the whole lesson.
## Summary
`build_tool_context` performs ~17 `std::env::var()` reads during server
construction (`LIBRARIAN_WORKSPACE`, `LIBRARIAN_DB`, `LIBRARIAN_EMBED_*`,
`LIBRARIAN_CWD`, plus 9 more via `RetrievalConfig::from_env` in
`ArtifactBackend::resolve`). Several lib tests mutate env **without** holding a
lock shared with those readers: `serial_test`'s global lock (used by
`#[serial]`) and `lock_env_for_tests()`'s `ENV_LOCK` are **disjoint** mutexes.
So a non-`#[serial]` env writer can run concurrently with a `#[serial]` test's
env reads — and on glibc, `setenv`/`unsetenv` can `realloc`/compact `environ`
while a concurrent `getenv` reads it (undefined behavior: NULL/garbage/segfault).

## Discovered by
Root-causing bug 33e4ae68 (guide_hint CI flake). Confirmed non-`#[serial]`,
non-locked env mutator:
`src/librarian/tools/audit_doc_refs/mod.rs::tests::glob_explosion_returns_recoverable`
— `set_var("LIBRARIAN_AUDIT_MAX_FILES","1")` + `remove_var(...)`, with neither
`#[serial]` nor `lock_env_for_tests()`. Other env-touching sites to audit:
`agent/mod.rs` test `EnvGuard`, and `cli::open_ctx` (a **production** `set_var`
of `LIBRARIAN_CWD` — benign in prod because codescout runs one command per
process, but would bite if CLI dispatch ever moves into a long-running/REPL
context).

## Why currently benign
The observed manifestation — a corrupted `LIBRARIAN_DB` read landing a *pinned*
guide_hint test on a shared/already-migrated catalog — is neutralized by the
33e4ae68 fix (`run_migrations` is now atomic under a `BEGIN IMMEDIATE`
transaction). But env-read UB could resurface as a *different* intermittent
failure (e.g. a corrupted `LIBRARIAN_WORKSPACE` read → wrong workspace →
some other test's assertion fails).

## Fix options (not yet done)

### ⚠️ Option 1 ("unify serialization") was attempted and is NOT VIABLE. Do not retry it.

It *reads* like the obvious mechanical fix. It is a trap, for two compounding reasons:

**1. The env READER set is not "the `build_tool_context` test paths." It is essentially the whole suite.** Verified at the bytes:

```
Agent::new
  -> Agent::load_project_resources     (src/agent/mod.rs)
  -> ProjectConfig::load_or_default    (src/config/project.rs)
  -> GlobalConfig::load                (src/config/global.rs)
  -> global_config_dir -> std::env::var_os("XDG_CONFIG_HOME") / ("HOME")
```

Every test that constructs an `Agent` with a project root reads process env — *hundreds* of them, essentially none annotated.

**2. `serial_test` cannot help with unannotated tests.** `#[serial]` serializes a test only against other `#[serial]` / `#[parallel]` tests. A plain, untagged test runs in parallel with a `#[serial]` one — by design; there is no lock an unannotated test participates in. So a `#[serial]`-tagged env writer **still races** the hundreds of untagged `Agent::new` readers, and the `setenv`-reallocs-`environ` UB is untouched.

Making option 1 sound would require annotating essentially the whole suite `#[parallel]`, serializing it end-to-end. That is not a fix, it is a different problem.

(The proof it was always a trap: the bug's own named example, `glob_explosion_returns_recoverable`, had *since gained* `#[serial_test::serial]` — and the UB still stood.)

### ✅ What was actually done: delete the writes

Any `setenv` can `realloc` the `environ` array, so **any** env write races **any** concurrent env read of **any** variable — the names need not match. Partial removal therefore does not partially fix it. The writes had to go, not get coordinated.

Config resolution now takes its inputs as *data*. Each seam is a pure function or an explicit parameter, so tests inject instead of mutating:

| Seam | Replaces reading… |
|---|---|
| `global_config_dir_from(xdg, home)` | `XDG_CONFIG_HOME` / `HOME` |
| `GlobalConfig::load_from_dir(dir)` | ↑ (via `global_config_path`) |
| `ProjectConfig::load_with_global_base(root, base)` | ↑ — **already existed**; the tests just weren't using it |
| `ProjectConfig::apply_embed_overrides(model, url)` | `CODESCOUT_EMBED_MODEL` / `_URL` |
| `plan_startup_env(explicit, default, exists)` + `startup_env_assignments(pairs, is_set)` | `CODESCOUT_ENV_FILE` + dotenv precedence |
| `LibrarianEnv { workspace, db, embed_*, cwd }` + `build_tool_context_with` | the 6 `LIBRARIAN_*` vars |
| `ServerEnv { probe, cc_session_id, librarian }` + `CodeScoutServer::from_parts_with_env` | `CODESCOUT_PROBE`, `CLAUDE_CODE_SESSION_ID`, + the above |
| `import_codescout(registry, ws)` / `reindex_cli(env, …)` | `CODESCOUT_REGISTRY`, `LIBRARIAN_*` |
| `enforce_file_cap(count, max)` | `LIBRARIAN_AUDIT_MAX_FILES` |

Each production entry point keeps a thin env-reading wrapper (`LibrarianEnv::from_env`, `ServerEnv::from_env`, `GlobalConfig::load`, …), so runtime behaviour is unchanged.

`ENV_LOCK` and `lock_env_for_tests` are **deleted**. So is every `EnvGuard` in the default build, and every `#[serial]` that existed only to guard env.

**`set_var` / `remove_var` in the default `cargo test` build: 119 → 0.**

### The result is *better isolation*, not merely less UB

The `guide_hint_tests` — the very cluster whose flake opened bug `33e4ae68` ("tool 'artifact' not registered") — all 11 now run **fully in parallel, with no `#[serial]` at all**, because their librarian workspace/db and CC session id are per-server values rather than process-global ones. They were never really "tests that need to take turns"; they were tests that mutated global state. Same for `librarian::tests` (`imports_codescout_projects`, `reindex_cli_indexes_repo`).

Four tests in `config/project.rs` (`load_or_default_*`) were `#[allow(dead_code)]` "stale test — missing `#[test]`" stubs. They had been **disabled because exercising config layering required faking `HOME`**. With the global base injected they are real, running tests again — the env problem had been silently costing coverage.

### Remaining: ONE writer, deliberately, and it cannot affect the default suite

`EnvGuard` in `src/agent/mod.rs`, used only by `semantic_memory_store_bootstrap_times_out_on_hung_qdrant`:

- It is **`server-stack`-gated, and `server-stack` is not a default feature** — so it does not compile into the default `cargo test` run and cannot corrupt it.
- The test exists precisely to exercise the **env-driven** construction path (`VectorBackend::resolve` + `RetrievalConfig::from_env`) against a black-hole Qdrant. Injecting past that path would delete the thing under test.

Closing it properly means threading a `RetrievalConfig` through `Agent`. Worth doing; not done here. The `EnvGuard` carries a comment saying exactly this, plus "do not copy this pattern into a default-feature test".

Also still present, and **sound**: `src/cli/mod.rs::open_ctx` and `config::global::load_startup_env` both `set_var` in **production**, at process startup, before any worker threads exist. Single-threaded `setenv` is fine. Called out so a future sweep does not "fix" them unnecessarily.

Out of scope (separate test binaries / crate, each its own process): `tests/retrieval_unit.rs` (5), `crates/codescout-embed/src/remote.rs` (6).

### Verification

3216 passed, 0 failed (up from 3204 — the four revived stale tests plus the new pure ones). `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo clippy --features server-stack --all-targets -- -D warnings` all clean.
## References
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md — parent; the migration TOCTOU this UB triggered
- docs/conventions/test-env-isolation.md
