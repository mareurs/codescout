---
id: a656f8cec220d347
kind: bug
status: open
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
1. **Unify serialization** — make every env mutator AND every
   `build_tool_context` test path hold ONE lock (collapse `serial_test` +
   `ENV_LOCK`, e.g. `serial_test`'s named lock `#[serial(env)]` everywhere).
   Broad but mechanical.
2. **Inject config** into `build_tool_context` for tests instead of reading
   process env (hermetic construction). Larger, cleaner long-term — the "deeper
   fix" bug 33e4ae68 already hinted at.
3. **Minimal** — add `#[serial]`/`lock_env_for_tests()` to the known non-serial
   mutators (`glob_explosion`, etc.). Partial — the same "serialize a subset"
   trap that made the two prior 33e4ae68 mitigations insufficient. Not
   recommended as the whole fix.

## References
- docs/issues/2026-07-02-guide-hint-artifact-not-registered-ci-flake.md — parent; the migration TOCTOU this UB triggered
- docs/conventions/test-env-isolation.md

