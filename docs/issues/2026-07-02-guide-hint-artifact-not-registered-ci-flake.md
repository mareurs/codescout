---
status: open
opened: 2026-07-02
closed:
severity: low
owner: marius
related: [docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md]
tags: [flaky-test, test-env-isolation, librarian]
kind: bug
---

# BUG: guide_hint test flakes on CI with "tool 'artifact' not registered"

## Summary
`server::guide_hint_tests::second_artifact_call_no_hint` failed once on CI
(ubuntu-latest / default, run on 88b8fb27) with `tool 'artifact' not registered`
from the shared `tool_by_name` helper — the librarian `artifact` tool was absent
from the freshly constructed `CodeScoutServer`. One-off flake: identical source
passed the same job at 218e0a4c and passes locally (2987/0/43, multiple runs).

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
Unknown — see Hypotheses tried.

## Evidence

### One-off vs deterministic
Same source green at 218e0a4c (ubuntu default success) and in every local full
gate this session; red once at 88b8fb27 whose diff is CI-yaml + docs only.

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
Not started.

## Tests added
N/A — this IS a test-infrastructure bug.

## Workarounds
Re-run the failed CI job (`gh run rerun <id> --failed`).

## Resume
Enumerate non-#[serial] tests that mutate `LIBRARIAN_DB` / `HOME` /
`XDG_DATA_HOME` or call `EnvGuard::set` without `#[serial]`:
`grep(pattern="EnvGuard::set", glob="src/**")` then cross-check each hit's test
attribute. If any exist, add `#[serial]` (or migrate the librarian DB path
resolution to constructor injection for tests, removing the env dependency —
the deeper fix docs/conventions/test-env-isolation.md hints at). Also check
whether CodeScoutServer::new silently skips librarian tool registration when
catalog open fails — a loud panic there would have named the real cause.

## References
- CI run on 88b8fb27 (Test ubuntu-latest/default job log, scratchpad
  ubuntu-default-88b8.log this session)
- docs/issues/2026-07-02-windows-gnu-wine-20-test-failures.md (same panic site,
  deterministic wine variant)
- docs/conventions/test-env-isolation.md
