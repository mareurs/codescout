---
kind: convention
status: active
title: Test environment isolation
owners: []
tags:
  - testing
  - concurrency
  - librarian
---

# Test environment isolation

Any test helper that constructs an object resolving configuration from
process-global env vars MUST isolate that state from concurrent tests.
"Object" here primarily means the librarian `Agent` (reads
`LIBRARIAN_DB`, `LIBRARIAN_WORKSPACE`, `LIBRARIAN_CWD`,
`LIBRARIAN_EMBED_*`), but the convention applies to any future builder
with the same shape.

Promoted from U-20 in `docs/trackers/codescout-usage-frictions.md`
after the #68 stability work surfaced a months-long latent SQLite race:
`make_server()` in `src/server.rs::guide_hint_tests` returned
`(TempDir, CodeScoutServer)` — looked self-contained — but built an
Agent that read `LIBRARIAN_DB` from process env, falling back to a
shared default `dirs::data_local_dir().join("librarian/catalog.db")`
when unset. Every parallel test calling that helper raced on the same
DB file. POSIX advisory locks usually hid the race on Linux; Windows
mandatory locks deadlocked routinely.

## The rule

A test helper that constructs an Agent (or any object that reads
config from process-global env) MUST do at least one of:

A. **Accept env values as explicit arguments.** The helper's signature
   names what it depends on; tests pass per-test values; no env
   mutation. Strongest isolation, often impractical when the helper
   wraps existing constructors that hard-code env reads.

B. **Return an `EnvGuard` (RAII) that the test holds for its full
   lifetime.** The helper sets env per-test, restores on drop. Combine
   with `#[serial_test::serial]` on every test that calls the helper —
   the EnvGuard alone is not sufficient under concurrent execution
   because `std::env::set_var` is process-global, so two parallel
   tests both setting and unsetting LIBRARIAN_DB will race regardless
   of guard discipline. The `#[serial]` lock pins one test's
   set→use→drop cycle to complete before the next starts.

C. **Document a `#[serial]` requirement on the helper's docstring.**
   Acceptable when the helper does not itself set env (so it inherits
   whatever the caller / suite set up), but the resulting object's
   behavior depends on env. Caller-driven; weaker than B.

## Established exemplars

Both modules in this repo carry their own local `EnvGuard` struct.
The shape is identical; the duplication is intentional (test-only
helpers stay local to each module — promoting to a shared helper
crate would tangle the dependency graph for marginal LOC savings):

| Helper | Location | Pattern |
|---|---|---|
| `EnvGuard` | `src/librarian/mod.rs::tests` | RAII set→restore; used with `#[serial]` |
| `EnvGuard` | `src/server.rs::guide_hint_tests` | Same shape; same use |

When a future test module hits the same shape, copy the EnvGuard pattern
locally rather than building a shared crate. The cost of two more lines
of duplication is lower than the cost of a generic test-utilities crate
that everyone has to learn.

## Diagnostic shape

The race is detectable in production CI as one of:

- Intermittent `"tool '<name>' not registered"` panics in tests that
  call `tool_by_name(...).unwrap()` — the librarian feature failed to
  register because catalog open / Agent init lost its env race.
- Intermittent `LIBRARIAN_DB` resolution to a path that was a previous
  test's tempdir (now dropped) — manifests as "no such file or
  directory" on first catalog op.
- On Windows: deadlocks instead of failures, because mandatory locks
  on the shared default DB file cause both readers to block forever
  rather than fail.

If you see this shape, suspect missing isolation in the test's helper
chain. The fix is option B above; document it locally and link back to
this convention from the helper's docstring.

## Known gaps (open)

The `#[serial]` + `EnvGuard` discipline established by U-20's fix is
robust **within** a single test module but does NOT coordinate
**across** modules. A non-`#[serial]` test in module X that touches
LIBRARIAN_DB or LIBRARIAN_CWD without an EnvGuard can race with an
in-flight Agent construction in module Y's `#[serial]` block. Observed
once on Linux during the U-23 verification session
(`server::guide_hint_tests::artifact_event_after_artifact_no_hint`
flaked under full `cargo test --lib`, passed cleanly on isolated
retry). Class-level fix deferred — likely options:

1. Annotate every env-mutating test in the codebase with `#[serial]`
   (find them via `grep "set_var(.*LIBRARIAN" tests/ src/`), even when
   they don't construct an Agent. Costs CI time (more serialization)
   for diagnostic stability.
2. Move the librarian DB / workspace resolution off of process-global
   env and onto explicit arguments threaded through `Agent::new`.
   Larger refactor, removes the foot-gun at the source.

The U-N tracker entry for U-20 carries the deferred status.
