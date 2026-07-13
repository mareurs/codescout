---
id: a656f8cec220d347
kind: bug
status: investigating
title: 'BUG: test env-access UB — non-#[serial] env writers race build_tool_context''s ~17 env reads (disjoint serial_test vs ENV_LOCK)'
owners: []
tags: []
topic: null
time_scope: null
---

# BUG: test env-access UB — non-#[serial] env writers race build_tool_context's env reads

## Status
open — latent test-hygiene issue; currently **benign** after the bug-33e4ae68 migration-atomicity fix (2026-07-13), but the underlying UB remains.

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

### ⚠️ Option 1 was attempted 2026-07-13 and is **NOT VIABLE**. Do not retry it.

The reason is worth writing down, because option 1 *reads* as the obvious mechanical fix and it is a trap.

**The env READER set is not "the `build_tool_context` test paths." It is essentially the whole suite.** Verified at the bytes:

```
Agent::new
  -> Agent::load_project_resources        (src/agent/mod.rs:529)
  -> ProjectConfig::load_or_default       (src/config/project.rs:418)
  -> GlobalConfig::load                   (src/config/global.rs)
  -> global_config_path -> global_config_dir
  -> std::env::var_os("XDG_CONFIG_HOME") / std::env::var_os("HOME")
```

Every test that constructs an `Agent` with a project root reads process env. That is *hundreds* of tests, and essentially none of them are annotated.

**`serial_test` cannot help with unannotated tests.** `#[serial]` serializes a test only against other `#[serial]` / `#[parallel]` tests. A plain, untagged test runs in parallel with a `#[serial]` one — by design; `serial_test` has no lock an unannotated test participates in. So a `#[serial]`-tagged env writer **still races** the hundreds of untagged `Agent::new` readers, and the `setenv`-reallocs-`environ` UB is untouched.

Making option 1 sound would require annotating essentially the entire test suite (`#[parallel]` on every env reader), which serializes the suite end-to-end. That is not a fix, it is a different problem.

This is the same **"serialize a subset" trap** that made the two prior `33e4ae68` mitigations insufficient — option 1 is just a bigger subset. The bug file already warned about the trap under option 3 and the warning applies to option 1 too. (Note the named example, `glob_explosion_returns_recoverable`, has since *gained* `#[serial_test::serial]` — and the UB still stands, which is itself the proof.)

### The only sound fix: stop mutating process env in tests (was "option 2")

Any `setenv` can `realloc` the `environ` array, so **any** env write races **any** concurrent env read of **any** variable — the vars don't have to match. Partial removal therefore does not partially fix it. The env writes have to go.

Scope, in dependency order:

1. **`GlobalConfig` / `global_config_dir`** — take the config dir explicitly instead of reading `HOME`/`XDG_CONFIG_HOME`. Kills the 5–9 `HOME`/`XDG` writers in `src/config/global.rs` + `src/config/project.rs`. `ProjectConfig::load_with_global_base(root, global_base)` already exists as exactly this seam for the project layer — the global layer needs its twin.
2. **`build_tool_context`** (`src/librarian/mod.rs:29`) — accept an injected config struct instead of reading `LIBRARIAN_WORKSPACE`, `LIBRARIAN_DB`, `LIBRARIAN_EMBED_{MODEL,URL,API_KEY}`, `LIBRARIAN_CWD`. Keep an env-reading thin wrapper for the real server entry point. Kills the `EnvGuard` writers in `src/server.rs::guide_hint_tests::make_server` and `src/librarian/mod.rs`.
3. **`RetrievalConfig::from_env`** / `ArtifactBackend::resolve` — same treatment (9 more reads).
4. Delete the three duplicated `EnvGuard` copies (`src/agent/mod.rs`, `src/librarian/mod.rs`, `src/server.rs`) and `ENV_LOCK`/`lock_env_for_tests` once nothing mutates env.

**Not attempted in this pass** — it is a real refactor of production config plumbing, materially larger than what was signed up for, and doing half of it would reproduce the very trap above. Left `investigating` with the analysis rather than shipping a placebo.

### Non-fix worth noting

`src/cli/mod.rs::open_ctx` also calls `set_var("LIBRARIAN_CWD")` — but in **production**, not tests, and it carries a doc comment correctly arguing that the codescout binary runs one command per process so no other threads exist yet. That one is fine as-is; it is called out here only so a future sweep does not "fix" it unnecessarily.
## References
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md — parent; the migration TOCTOU this UB triggered
- docs/conventions/test-env-isolation.md
