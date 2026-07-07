---
id: '9c034ab9429ff2bf'
kind: bug
status: fixed
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
closed: '2026-07-05'
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

### Recurrence 2026-07-05 — the HOLDER step flakes too; fix covered only the reclaim step

Full `cargo test` (default parallelism, during link_scan work):

```
lsp::manager::tests::claim_mux_lock_some_when_free_none_when_held
panicked at src/lsp/manager.rs:2211:37:
called `Result::unwrap()` on an `Err` value: Os { code: 11, kind: WouldBlock, … }
```

Line 2211 is `holder.try_lock_exclusive().unwrap()` — the test's own holder fd acquiring
the lock right after `drop(guard)` released the first claim. Same mechanism as the fixed
reclaim flake (flock-release-visibility latency under load), different step: the 2026-07-05
fix added the retry loop only to the post-release *reclaim*, not to the *holder*
acquisition. Passes in isolation (verified same session).
## Hypotheses tried
1. **Caused by the concurrent Qdrant fix under test in this session.** Test: the failing
   tests (peer sockets, LSP mux locks) share no code path with `QdrantWrap::connect` or
   `Agent::semantic_memory_store`. **Verdict: rejected** — different failing-test sets
   across runs with identical code is inconsistent with a real regression from one change.
2. **Deterministic failure regardless of parallelism.** Test: re-run each failing test
   alone. **Verdict: rejected** — all passed in isolation.

## Fix

Fixed as **test-harness robustness** (no product code touched) — the failures were
real-time readiness budgets losing to CPU starvation under `nproc`-wide parallelism:
- **Peer socket readiness budgets raised to ~5s.** `src/peer/server.rs`
  `connect_with_retry` (50×20ms → 250×20ms) and its two inline connect loops
  (50×50ms → 100×50ms); `src/peer/client.rs` inline connect loop (50×20ms → 250×20ms).
  The spawned server binds correctly; under load it can just be scheduled past a tight
  real-time window, so a generous budget absorbs the delay.
- **Mux-lock reclaim made retry-tolerant.** `src/lsp/manager.rs`
  `claim_mux_lock_some_when_free_none_when_held` now retries the post-release claim
  (up to 50×20ms) instead of asserting instant reclaim, accommodating brief
  flock-release-visibility latency under load.
- **(2026-07-05 recurrence) Holder step made retry-tolerant too.** The same test's
  `holder.try_lock_exclusive()` (immediately after `drop(guard)`) got the same 50×20ms
  retry loop — the original fix had covered only the reclaim step; the holder step
  flaked under a full parallel run during link_scan work.

`#[serial]` was considered and rejected: the contention is CPU starvation from the
*other* ~2870 tests, which serializing the socket tests among themselves would not
relieve; raising the real-time budget targets the actual mechanism and keeps parallelism.
## Tests added

None — this is test-harness robustness, not a product fix. The regression signal is
the existing suite staying green under default parallelism.
## Workarounds
Re-run failed tests individually or with `--test-threads=1` to confirm they're not a real
regression before investigating further.

## Resume

Fixed. Verified: the 18 peer + mux-lock tests pass under `--features server-stack`,
and the full lib suite is green (2882 passed, 0 failed) under default `nproc`-wide
parallelism — the exact config that flaked. A single green run can't prove a rare
flake is gone, but both documented mechanisms (tight readiness budgets, instant-reclaim
assumption) are structurally removed.
## References
- Surfaced during `docs/issues/2026-06-24-qdrant-hang-wedges-mcp-startup.md` fix verification, 2026-07-03.
