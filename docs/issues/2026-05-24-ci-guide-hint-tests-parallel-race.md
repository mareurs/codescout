---
status: open
opened: 2026-05-24
closed:
severity: low
owner: marius
related: [docs/issues/2026-05-24-ci-timeline-ulid-flaky.md]
tags: [ci, librarian, race, test-isolation, parallel-tests]
kind: bug
---

# BUG: server::guide_hint_tests::artifact_event_after_artifact_no_hint flakes on CI parallel runs

## Summary

In CI run 26357101302 (commit `c83b5544`) after seeding
`~/.config/librarian/workspace.toml`, 5 of 6 `server::guide_hint_tests`
pass — but `artifact_event_after_artifact_no_hint` reliably fails with
`tool 'artifact' not registered` at `src/server.rs:2515:32`. Same test
binary, same env, same workspace.toml seed — the differential is which
test calls `make_server` first under parallel scheduling. Hypothesis:
`build_tool_context` (called from `try_build_runtime`) interacts with
some shared filesystem state (config dir, lock file, scratch path) in
a way that makes the first-to-arrive succeed and later arrivals
race-fail. The 5 passing tests happen to hit the lucky ordering;
`artifact_event_after_artifact_no_hint` is the loser deterministically
under cargo's parallel-test default.

## Symptom (Effect)

```
thread 'server::guide_hint_tests::artifact_event_after_artifact_no_hint'
panicked at src/server.rs:2515:32:
tool 'artifact' not registered

test result: FAILED. 2467 passed; 2 failed; 7 ignored
(this test + librarian::tools::timeline::tests::returns_events_newest_first)
```

Same test binary, run with `cargo test` (default `--test-threads`),
ubuntu-latest, default features.

## Reproduction

```bash
# Locally on ubuntu (after touching $HOME/.config/librarian/workspace.toml):
cargo test --features librarian -- --test-threads=8 \
  server::guide_hint_tests 2>&1 | grep -E "FAILED|test result"
```

Should reproduce intermittently. `--test-threads=1` likely masks the flake.

## Environment

- Ubuntu 24.04 GHA runner, default features
- Parallel test execution (cargo test default)
- `$HOME/.config/librarian/workspace.toml` exists (empty file)

## Root cause (hypothesis)

`build_tool_context` (called by `try_build_runtime` in
`src/librarian/adapter.rs:20`) performs filesystem-coupled setup —
likely opening a SQLite catalog, locking a workspace dir, or rebuilding
an embedding index. Under parallel test scheduling, the first
`make_server().await` call wins the lock or completes setup; subsequent
ones may race and observe an inconsistent state, returning Err. The
caller `try_build_runtime` swallows the error and returns None:

```rust
pub async fn try_build_runtime() -> Option<Arc<LibToolContext>> {
    match crate::librarian::build_tool_context().await {
        Ok(ctx) => Some(Arc::new(ctx)),
        Err(err) => {
            tracing::info!("librarian disabled: {err:#}");
            None
        }
    }
}
```

The loser test ends up with no librarian tools registered.

## Evidence

- CI run 26357101302 Test (ubuntu-latest / default):
  - 5/6 guide_hint_tests pass
  - 1/6 (`artifact_event_after_artifact_no_hint`) fails at tool_by_name
  - Local single-threaded runs and the prior 4/4 failure point at the
    seed step having fixed the bulk of the issue, leaving a residual race

- The 5 passing tests:
  - `first_artifact_call_emits_librarian_hint`
  - `activate_project_resets_hints`
  - `run_command_with_overflow_emits_progressive_hint_once`
  - `run_command_without_overflow_no_progressive_hint`
  - `second_artifact_call_no_hint`

## Hypotheses tried

- Did the seed step work? Mostly — 5/6 pass. So the workspace.toml seed
  was load-bearing; this remaining failure is a different layer.

## Fix

Two candidate shapes:

**A. Serialize the test (cheap workaround).** Add `#[serial]` from the
`serial_test` crate to all `guide_hint_tests`. Pure band-aid — masks the
race rather than fixing it. Fine if `build_tool_context` is known to be
intentionally non-reentrant.

**B. Make `build_tool_context` truly idempotent / reentrant.** Audit
its filesystem interactions; replace shared lock/cache files with
per-call scoped state, or use a global tokio::OnceCell to share a single
context across concurrent callers. The right fix if the production
server can hit this race under multi-client load.

Read `build_tool_context` (probably in `src/librarian/runtime.rs` or
similar) to decide. The CLI-style audit-doc-refs path uses the same
code in single-threaded mode and works fine — so the race is parallelism-
specific, not a fundamental shape bug.

## Tests added

The failing test itself is the regression case. After the fix, run with
`--test-threads=8` to confirm stability across 100 invocations.

## Workarounds

`cargo test -- --test-threads=1` reliably passes everything but slows
the full test suite ~3-4x on multi-core boxes.

## Resume

1. Grep for `build_tool_context` and audit FS interactions.
2. If lock/cache file is the culprit: cache the tool context in a
   `static OnceCell<Arc<LibToolContext>>` so concurrent callers share
   one instance.
3. If easier — apply `#[serial_test::serial]` to all guide_hint_tests
   and document the parallelism constraint as a known limitation.
4. Push and verify on CI matrix.

## References

- `src/server.rs:2515` (tool_by_name unwrap site)
- `src/server.rs:2576-2597` (the failing test)
- `src/librarian/adapter.rs:20-28` (try_build_runtime — swallows Err)
- Sibling rot surfaced same CI run:
  - `docs/issues/2026-05-24-ci-timeline-ulid-flaky.md` (timeline ULID race)
  - `docs/issues/2026-05-24-ci-macos-tempdir-canonicalization.md`
