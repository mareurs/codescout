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
closed: '2026-07-05'
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

Done. If the flake ever recurs, the next suspect is the *silent* degrade in
`try_build_runtime` (`src/librarian/adapter.rs:20-24`): it logs a failed
`build_tool_context` at `info` and returns `None`, which is what made this root
cause opaque. Consider raising that to `warn` (a louder signal would have named
the cause immediately), or the deeper fix: thread the librarian config/DB path
through construction so tests don't depend on process-global HOME at all.
## References
- CI run on 88b8fb27 (Test ubuntu-latest/default job log, scratchpad
  ubuntu-default-88b8.log this session)
- docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md (same panic site,
  deterministic wine variant)
- docs/conventions/test-env-isolation.md
