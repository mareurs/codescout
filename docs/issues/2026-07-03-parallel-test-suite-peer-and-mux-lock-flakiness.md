---
id: '9c034ab9429ff2bf'
kind: bug
status: open
title: 'BUG: peer::server/peer::client + lsp::manager mux-lock tests intermittently fail under full parallel `cargo test --lib`'
owners: []
tags:
- testing
- flaky
- peer
- lsp-mux
- parallel
topic: null
time_scope: null
opened: '2026-07-03'
owner: marius
related: []
severity: low
---


# BUG: peer::server/peer::client + lsp::manager mux-lock tests intermittently fail under full parallel `cargo test --lib`

## Summary
Noticed incidentally while verifying an unrelated fix (Qdrant bootstrap-hang guard,
`docs/issues/2026-06-24-qdrant-hang-wedges-mcp-startup.md`). Two back-to-back full-suite
runs of `cargo test --lib --features server-stack` each failed a *different* small set of
tests, all of them resource-contention-shaped (Unix-domain-socket peer-serve handshakes,
an flock-based LSP mux lock). Every failing test passes cleanly in isolation
(`--test-threads=1`, filtered to just that test). Not caused by, or related to, the Qdrant
work — pure pre-existing test-suite flakiness under default parallelism.

## Symptom (Effect)
Run 1 (`cargo test --lib --features server-stack`, default parallelism) failed:
- `peer::client::tests::client_hello_then_tool_call` — panicked: "peer socket never came up"
- `peer::server::tests::hello_returns_capabilities_for_read_only_peer` — same panic
- `peer::server::tests::tool_call_runs_an_exposed_read_tool` — same panic
- `peer::server::tests::peer_tool_call_ignores_smuggled_workspace_override` — "could not connect to served socket"
- `peer::server::tests::end_to_end_served_read_tool_and_write_denied` — same

Run 2 (same command, no code changes in between) failed a **different** set:
- `lsp::manager::tests::claim_mux_lock_some_when_free_none_when_held` — "lock should be
  reclaimable after the holder releases"
- 5 of the same `peer::*` tests as Run 1 (not identical membership)

Both runs: 2866-2867 passed, ~5-6 failed, out of ~2881 total.

## Reproduction
```
cargo test --lib --features server-stack          # fails a handful, non-deterministic set
cargo test --lib --features server-stack peer:: -- --test-threads=1     # all 17 peer:: tests pass
cargo test --lib --features server-stack lsp::manager::tests::claim_mux_lock_some_when_free_none_when_held -- --test-threads=1   # passes
```

## Environment
codescout `experiments` branch, `server-stack` feature, Linux, `cargo test --lib` default
parallelism (test-threads = number of CPUs). Session also had several ad-hoc `cargo test`
invocations running/killed shortly before (unrelated Qdrant black-hole-listener debugging),
which may have added transient system load but the flakiness pattern (different tests
failing each run, all resource-contention-shaped) suggests this is not solely a one-off
artifact of that load.

## Root cause
Unknown — not investigated beyond isolation. Working hypothesis: tests that bind Unix
domain sockets (`peer::server`/`peer::client`) or flock-based lock files (`lsp::manager`
mux lock) under default full parallelism can race on OS-level resource availability
(ephemeral socket path reuse, flock acquisition timing) when many tests spin up child
processes / sockets concurrently. Each failing test's fixture presumably retries or waits
for a "ready" signal with a bounded budget that's occasionally too tight under parallel
system load.

## Evidence
Two `cargo test --lib --features server-stack` runs, this session, 2026-07-03:
run 1 → 5 failures (all peer::*); run 2 (unmodified code) → 6 failures (5 peer::* + 1
lsp::manager::*, non-identical set to run 1). Every individually-failing test passes when
re-run alone with `--test-threads=1`.

## Hypotheses tried
1. **Caused by the concurrent Qdrant fix under test in this session.** Test: the failing
   tests (peer sockets, LSP mux locks) share no code path with `QdrantWrap::connect` or
   `Agent::semantic_memory_store`. **Verdict: rejected** — different failing-test sets
   across runs with identical code is inconsistent with a real regression from one change.
2. **Deterministic failure regardless of parallelism.** Test: re-run each failing test
   alone. **Verdict: rejected** — all passed in isolation.

## Fix
Not started — out of scope for the session that found it. Candidate directions: raise
per-test "ready" wait budgets for `peer::server`/`peer::client` fixtures; give LSP-mux-lock
tests a unique lock-file path per test (if not already); or mark the affected tests
`#[serial]` / run this test group with reduced parallelism in CI.

## Tests added
N/A — flakiness observation, not a fix.

## Workarounds
Re-run failed tests individually or with `--test-threads=1` to confirm they're not a real
regression before investigating further.

## Resume
Start by checking whether `peer::server`/`peer::client` test fixtures already have a
documented "ready" polling budget (grep `ready` in `src/peer/server.rs` tests) and whether
it's generous enough under `nproc`-wide parallel load; check the LSP mux lock test's
lock-file path for collisions with other concurrently-running tests.

## References
- Surfaced during `docs/issues/2026-06-24-qdrant-hang-wedges-mcp-startup.md` fix verification, 2026-07-03.

