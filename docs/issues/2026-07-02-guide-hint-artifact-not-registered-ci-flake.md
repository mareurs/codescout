---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- flaky-test
- test-env-isolation
- librarian
topic: null
time_scope: null
opened: '2026-07-02'
owner: marius
related:
- docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md
severity: medium
---

# BUG: guide_hint test flakes on CI with "tool 'artifact' not registered"

## Summary
`server::guide_hint_tests::second_artifact_call_no_hint` failed on CI
(ubuntu-latest / default) with `tool 'artifact' not registered`
from the shared `tool_by_name` helper — the librarian `artifact` tool was absent
from the freshly constructed `CodeScoutServer`. Hit 2 of 3 runs on identical
source (runs on 88b8fb27, its rerun — which failed on the sibling heartbeat race
instead — and d936eb0f); the same source was green at 218e0a4c and passes
locally (2987/0/43, multiple runs).

## Resolution (2026-07-13) — CONFIRMED root cause + fix

**The prior env-race hypotheses were wrong.** Instrumenting `try_build_runtime` to `eprintln` the swallowed `build_tool_context` error, then looping the full `cargo test --lib` on this box, reproduced the failure on run 6/10 with the decisive message:

    [[LIBRARIAN_BUILD_FAILED]] running migrations: duplicate column name: entry_collection

The failing step is `Catalog::open_with_workspace` -> `run_migrations`, **not** a corrupted env read of `LIBRARIAN_WORKSPACE`/`LIBRARIAN_DB`.

**Root cause — TOCTOU in `run_migrations`.** Each v4+ migration block was a non-atomic check-then-act:

    if !column_exists(conn, "artifact_augmentation", "entry_collection")? {   // CHECK
        conn.execute("ALTER TABLE ... ADD COLUMN entry_collection TEXT", [])?; // ACT
    }

In autocommit mode the `SELECT` check holds no lock, so two connections sharing one catalog file can both observe the column missing and both issue the `ALTER`; the loser fails with `duplicate column name`. This is a genuine production hazard too — `open_with_workspace`'s own comment documents that separate codescout instances share one catalog file.

**Fix.** Wrap the whole migration sequence in one `BEGIN IMMEDIATE` … `COMMIT` write transaction (body extracted to `apply_migrations_in_txn`; `ROLLBACK` on error). With the connection's `busy_timeout = 5000`, the second writer blocks on `BEGIN IMMEDIATE`, then re-checks and no-ops. Migration is now atomic (add_columns + backfill all-or-nothing). File: `src/librarian/catalog/mod.rs`.

**Regression test.** `run_migrations_is_safe_under_concurrent_connections` (same file's test module): seeds the v3 baseline, then hammers `run_migrations` from 16 barrier-synced connections on one shared db. Deterministically RED before the fix (`duplicate column name: render_template`), GREEN after.

**Verified.** New test RED->GREEN; full `cargo test` = 3183 passed / 0 failed; the full-suite flake loop went from FAIL-on-run-6 to **15/15 clean passes** post-fix; `clippy -D warnings` clean.

### Secondary finding (NOT fixed here) — test env-access UB
The *trigger* that let a pinned guide_hint test land on a shared/wrong `LIBRARIAN_DB`: `build_tool_context` does ~17 `std::env::var()` reads during construction, and several env-mutating tests are neither `#[serial]` nor under `lock_env_for_tests()` (e.g. `audit_doc_refs::tests::glob_explosion_returns_recoverable` sets/removes `LIBRARIAN_AUDIT_MAX_FILES` with no lock). `serial_test`'s lock and `ENV_LOCK` are **disjoint**, so env *writes* can race the guide_hint env *reads* (glibc setenv/getenv UB). The migration fix makes this benign for the observed failure, but the broader env-isolation gap remains a latent test-hygiene issue (fix would be: unify on one lock, or inject config instead of reading process env). Left as a follow-up.
## Symptom (Effect)
```
thread 'server::guide_hint_tests::second_artifact_call_no_hint' (5233) panicked at src/server.rs:2966:32:
tool 'artifact' not registered
test result: FAILED. 2867 passed; 1 failed; 6 ignored
```
CI run on 88b8fb27, Test (ubuntu-latest / default). Same panic SITE
(src/server.rs:2966, `tool_by_name`) as the deterministic wine guide_hint
cluster (8 tests, WIN-27) — different trigger.

## Reproduction
Not yet reproducible — best lead: parallel-test env race (below). Observed once
on a GitHub-hosted runner; never locally.

## Environment
GitHub Actions ubuntu-latest, default features, cargo test (parallel). Source
identical to a green run (88b8fb27 changed only ci.yml + docs vs 218e0a4c).

## Root cause

**Confirmed (2026-07-05).** Two independent test-serialization mechanisms that
do not coordinate with each other:

1. The guide_hint tests are `#[serial]` (serial_test crate's global lock).
2. The HOME/`XDG_CONFIG_HOME`-mutating config tests
   (`src/config/global.rs::tests`, `src/config/project.rs::tests::env_vars_override_model_and_url`)
   serialize via a *separate* `ENV_LOCK` mutex (`lock_env_for_tests()`), NOT
   `#[serial]`.

`#[serial]` and `ENV_LOCK` are disjoint locks, so a config test (holding
`ENV_LOCK`, mutating HOME to a fresh tempdir) can run concurrently with a
`#[serial]` guide_hint test. During the guide_hint test's
`CodeScoutServer::new` → `try_build_runtime` → `build_tool_context`, the code
reads `default_config_path()` → `dirs::config_dir()` → `$HOME/.config` (when
`XDG_CONFIG_HOME` is unset). If HOME points at the config test's tempdir
(no seeded `~/.config/librarian/workspace.toml`), `workspace::load()`'s
`read_to_string` fails → `build_tool_context` returns `Err` → `try_build_runtime`
**silently logs at `info` and returns `None`** (`src/librarian/adapter.rs:20-24`)
→ the librarian tool surface (incl. `artifact`) is never registered →
`tool_by_name(server, "artifact")` panics. `make_server` pins `LIBRARIAN_DB`
but NOT the workspace/config path, so HOME is the unguarded global. Green when
scheduling doesn't overlap; red 2-of-3 on loaded CI.
## Evidence

### One-off vs deterministic
Same source green at 218e0a4c (ubuntu default success) and in every local full
gate this session; red at 88b8fb27, its rerun (heartbeat race instead — see
related file), and d936eb0f — whose combined diff vs 218e0a4c is CI-yaml + docs
only. 2-of-3 incidence for this test on current runner conditions: elevated,
not one-off — severity raised low→medium accordingly.

### The test IS #[serial]
src/server.rs:3055-3057 — `#[tokio::test] #[serial]`. But `#[serial]` only
serializes against other `#[serial]` tests; an UNMARKED test mutating
librarian-relevant env (`LIBRARIAN_DB`, `HOME`/`XDG_DATA_HOME` affecting
`dirs::data_local_dir()`) can still race the `EnvGuard::set("LIBRARIAN_DB", …)`
+ `Agent::new` + `CodeScoutServer::new` window in `make_server`
(src/server.rs:2950-2958).

## Hypotheses tried
1. **Hypothesis:** introduced by this branch. **Test:** diff 218e0a4c..88b8fb27
   (ci.yml + docs only) + green run at 218e0a4c. **Verdict:** rejected —
   pre-existing flake class.
2. **Hypothesis:** env race from a non-#[serial] test mutating librarian env
   during make_server. **Test:** not yet run. **Verdict:** deferred — see Resume.

## Fix

Unified the two serialization mechanisms so the HOME/`XDG_CONFIG_HOME`
mutators can no longer overlap the serial readers:

1. Added `#[serial]` to the 5 **live** HOME/XDG-mutating config tests
   (`src/config/global.rs`: `global_config_path_uses_xdg_config_home`,
   `global_config_path_falls_back_to_home_dot_config`,
   `global_config_load_returns_none_when_absent`,
   `global_config_load_parses_valid_toml`; `src/config/project.rs`:
   `env_vars_override_model_and_url`) + the `use serial_test::serial;` import in
   both test modules. They keep `lock_env_for_tests()` too (it still
   coordinates them with the preflight *reader* tests), so both locks are held;
   `#[serial]` is what newly excludes them from the serial guide_hint tests.
2. Incidental latent bug fixed on the spot: `src/config/global.rs`'s `mod tests`
   was missing the `#[cfg(test)]` gate (it compiled only because it happened to
   use crates that are also normal deps; the dev-only `serial_test` import
   exposed it). Added `#[cfg(test)]`.

The 4 dead HOME-mutating tests in `project.rs` and
`global_config_load_errors_on_malformed_toml` (all `#[allow(dead_code)]`, no
`#[test]`) don't run, so they weren't serialized — **whoever re-enables one must
add `#[serial]`** or the race returns. The deeper fix the bug hinted at
(inject the config/DB path for tests, removing the process-global HOME
dependency in `build_tool_context`) remains a larger future option, not done
here.
## Tests added

No new test — the fix is test-attribute serialization. Verified: `cargo test
--lib config::` (105/105) and `cargo test --lib server::guide_hint_tests::`
(11/11) pass; `cargo clippy --lib --tests -- -D warnings` clean. The flake was
scheduling-dependent so a single green run isn't proof, but the race window is
now structurally closed (mutators and readers are mutually exclusive under one
lock).
## Workarounds
Re-run the failed CI job (`gh run rerun <id> --failed`).

## Resume

**REOPENED 2026-07-13.** The 2026-07-02 mitigation (pin LIBRARIAN_WORKSPACE + LIBRARIAN_DB via EnvGuard in `make_server`; `#[serial]` on every guide_hint test) is INSUFFICIENT. Reliably reproduced locally: adding 2 unrelated non-serial lib tests (`tools::read_file`, commit 3af52f1e) tipped `activate_project_resets_hints` from green→red under the full parallel `cargo test` (~50–100% on this box; passes in isolation; baseline without the 2 tests was green).

Diagnosis so far:
- Failure is `try_build_runtime`/`build_tool_context` returning None during `make_server` → the `artifact` tool never registers (panic in `tool_by_name`).
- Every test that mutates LIBRARIAN_DB / LIBRARIAN_WORKSPACE / HOME / XDG_CONFIG_HOME is ALREADY `#[serial]` or holds `lock_env_for_tests()` (verified: `reindex_cli_indexes_repo` #[serial]; `config::project.rs` + `config::global.rs` use `lock_env_for_tests()`). So a named env-clobber racer is NOT the cause.
- Adding `lock_env_for_tests()` to `make_server` (to bridge serial_test's `#[serial]` and the bespoke ENV_LOCK) did NOT reliably fix it — one green run then red — so the race is not (only) HOME/XDG. Reverted.
- Leading hypothesis: a process-global resource/timing contention in `CodeScoutServer::new` / `build_tool_context` under parallel load (catalog open, Qdrant connect), or an unguarded `std::env::set_current_dir` racer affecting current-dir-derived resolution.

Next: instrument `build_tool_context` to log WHICH step fails (workspace::load vs Catalog::open vs current_dir) when the flake fires; grep tests for `set_current_dir`; consider making `artifact` registration not hinge on transient runtime build success, or making `make_server` assert+retry. **This blocks reliable full-suite gating and should be fixed before further parallel-test additions.**
## References
- CI run on 88b8fb27 (Test ubuntu-latest/default job log, scratchpad
  ubuntu-default-88b8.log this session)
- docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md (same panic site,
  deterministic wine variant)
- docs/conventions/test-env-isolation.md
