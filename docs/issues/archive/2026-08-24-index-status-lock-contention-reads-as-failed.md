---
id: 05a0548d57664984
kind: bug
status: fixed
title: 'BUG: index(action="status")''s indexing.status stays "failed" from a stale lock-contention race while real progress (chunk_count, GPU) continues'
owners:
- marius
tags:
- indexing
- misleading-status
- concurrency
- codescout-tool
closed: 2026-08-24
opened: 2026-08-24
owner: marius
related:
- docs/issues/2026-08-23-index-build-fails-embed-batch-sparse-send.md
severity: medium
---


## Summary

`index(action="status")`'s `indexing` field echoed a stale, per-agent-process
lock-contention result as `"failed"`, indistinguishable from a genuine failure,
while the real indexer (elsewhere) kept making progress. Fixed by having both
`index(action="build")` (pre-spawn, and the rare post-spawn race) and
`index(action="status")`'s `Idle` arm peek the cross-process lock and report
`"already_running_elsewhere"` / `"running_elsewhere"` with the holder's PID
instead. Live-verified: started a real rival `codescout index --force` CLI
process, called `index(action="build")` against it and got
`{"status": "already_running_elsewhere", "holder_pid": 49334}`; `index(action="status")`
reported `{"status": "running_elsewhere", "holder_pid": 49334}`; `ps -p 49334`
confirmed that PID was exactly the rival CLI process.
## Symptom (Effect)

Four consecutive `index(action="status")` calls, each showing a growing
`chunk_count` alongside an unchanging `"failed"` `indexing` block:

```
chunk_count: 41158 → 41951 → 42232 → 42781 → 43549 → 47579
indexing: {
  "status": "failed",
  "error": "another codescout index is already running for project 'codescout'
   (lock file: /run/user/1000/codescout-index-ee26de4c61f6f20e.lock — its first
   line is the holder's PID). ..."
}
```

`git_sync.behind_commits` also never moved (stuck at 943/944) across the same
polls, even as the *actual* indexing job — visibly not this agent's — kept
adding chunks.

## Reproduction

1. Have some other process (or this same MCP server's own reconnect-time
   auto-index task) already holding `codescout-index-<hash>.lock` for the
   project and actively indexing.
2. Call `index(action="build")` from a second agent/session against the same
   project. It returns `{"status": "started"}` immediately (a spawned
   `tokio::spawn` task, not a synchronous failure).
3. That spawned task calls into `sync_project`, which calls
   `index_lock::acquire` (`src/retrieval/index_lock.rs:126`), fails to get the
   OS-level exclusive lock, and the task sets
   `ctx.agent.indexing = IndexingState::Failed(e.to_string())`
   (`src/tools/semantic/index.rs:408`).
4. Every subsequent `index(action="status")` echoes that same frozen
   `Failed(...)` (`src/tools/semantic/index.rs:564-566`) until a new `build`
   call overwrites it — regardless of what the real lock holder is doing.

Observed on commit `f6263931a74b798347cce10e541146022c6c325d` on `experiments`.

## Environment

Linux laptop, `codescout start --debug` (multiple concurrent instances on this
machine — see `docs/trackers/tracker-hygiene-log.md` and this session's
cross-session coordination for context). NVIDIA GPU stack via
`docker-compose.yml` `gpu` profile (`codescout-dense-gpu`, `codescout-qdrant`,
`codescout-reranker-gpu`).

## Root cause

`ctx.agent.indexing` (`IndexingState`, defined `src/agent/mod.rs:22-38`) is a
**per-agent-process, in-memory** `Mutex`, not a live read of the shared,
cross-process file lock at `src/retrieval/index_lock.rs`. `IndexProject::call`
(`src/tools/semantic/index.rs:45-426`) spawns a background task
(`tokio::spawn`, line 303) that calls `sync_project`; on any `Err` — including
losing the `index_lock::acquire` race to a different holder — it sets
`IndexingState::Failed(e.to_string())` (line 408). `IndexStatus::call`
(`src/tools/semantic/index.rs:451-592`) then echoes whatever `IndexingState`
this agent last landed on, verbatim, as `result["indexing"]`
(lines 557-566) — with no re-check against the lock file's *current* holder,
and no distinction between "this agent's own attempt genuinely failed" (e.g.
the sparse-embedder bug, `docs/issues/2026-08-23-index-build-fails-embed-batch-
sparse-send.md`, which used this exact same code path and field shape) and
"this agent's attempt was redundant because indexing is already happening
correctly elsewhere."

The lock-acquire error text itself acknowledges the ambiguity ("The holder may
be a CLI `codescout index` run OR an in-process background index... check the
PID") — but that guidance lives only in the error *string*, not in the
response *shape*, so a caller has to know to distrust `status: "failed"` and
go cross-check `chunk_count`/`git_sync` by hand, which is exactly what this
session had to do.

*Measured 2026-08-24: read `src/tools/semantic/index.rs` and `src/agent/mod.rs`
directly (see line citations above); corroborated live by `nvidia-smi
--query-gpu=utilization.gpu,memory.used,memory.total` showing 95% GPU
utilization with two `llama-server` processes holding VRAM while
`index(action="status")` reported `indexing.status: "failed"`, and by four
repeated `index(action="status")` calls showing `chunk_count` climbing under
the same unchanging "failed" block.*

## Evidence

Tool output, this session, four consecutive `index(action="status")` calls
(chunk_count only shown for brevity — full envelopes carry the same frozen
`indexing` block each time):

```
43549 chunks — indexing: {"status":"failed","error":"another codescout index is already running..."}
47579 chunks — indexing: {"status":"failed","error":"another codescout index is already running..."}
```

`nvidia-smi` output, same session:

```
utilization.gpu [%], memory.used [MiB], memory.total [MiB]
95 %, 745 MiB, 6144 MiB
---
pid, process_name, used_gpu_memory [MiB]
1580, /app/llama-server, 340 MiB
1597, /app/llama-server, 394 MiB
```

Source, `src/tools/semantic/index.rs:564-566`:

```rust
IndexingState::Failed(e) => {
    result["indexing"] = json!({ "status": "failed", "error": e });
}
```

Source, `src/retrieval/index_lock.rs:90-99` (the error `Failed` ends up
carrying verbatim):

```rust
file.try_lock_exclusive().with_context(|| {
    format!(
        "another codescout index is already running for project '{project_id}' \
         (lock file: {} — its first line is the holder's PID). The holder may be \
         a CLI `codescout index` run OR an in-process background index (e.g. an \
         MCP server's auto-index task) — check the PID, don't assume `pgrep -af \
         'codescout index'` will show it.",
        path.display()
    )
})?;
```

## Hypotheses tried

1. **Hypothesis:** the index build genuinely crashed and stopped.
   **Test:** re-polled `index(action="status")` four times over several
   minutes; separately checked `nvidia-smi` and `docker ps`.
   **Verdict:** rejected — `chunk_count` climbed monotonically across every
   poll and the GPU stayed busy, which is inconsistent with a stopped build.
   **Evidence link:** § Evidence, chunk_count sequence + nvidia-smi output.
2. **Hypothesis:** `indexing.status` is a live signal but simply slow to
   update. **Test:** traced the field's producer in source
   (`src/tools/semantic/index.rs`, `src/agent/mod.rs`) rather than waiting
   longer. **Verdict:** rejected — it is a `Mutex<IndexingState>` written once
   per `build` call and read back verbatim; nothing re-derives it from the
   lock file or Qdrant between calls. **Evidence link:** § Root cause.

## Fix

Implemented in `src/retrieval/index_lock.rs` and `src/tools/semantic/index.rs`:

1. `acquire_in` now returns a downcastable `LockHeldError { project_id, path,
   holder_pid }` on contention instead of an opaque `anyhow` context string.
2. Added `peek_in`/`peek` — acquire-then-immediately-release, non-blocking,
   returning `Option<Option<u32>>` (held-with-pid / free-or-unknown).
3. `IndexProject::call`: peeks the cross-process lock synchronously, before
   committing `ctx.agent.indexing` to `Running` — on contention, returns
   `{"status": "already_running_elsewhere", "holder_pid"}` and never touches
   agent state. The narrow post-spawn race (peek said free, the real acquire
   loses moments later) is classified via the same `LockHeldError` downcast
   inside the spawned task and steps back to `Idle` instead of `Failed`.
4. `IndexStatus::call`'s `Idle` arm now also peeks, so an agent that never
   called `build` gets `{"status": "running_elsewhere", "holder_pid"}` instead
   of silence when someone else is indexing.

**SHA:** `60a7e624` (branch `experiments`)
**patch-id:** `40abcc0cf2f65a1895332ba7da63925d7e9700b2`

Designed via `superpowers:brainstorming` (bounded path) before implementation;
two design forks considered and rejected: tagging the failure without changing
control flow (leaves callers needing to check a flag), and only fixing the
pre-spawn case (leaves the Idle-agent blind spot open).
## Tests added

Hermetic unit tests, TDD (red confirmed — compile failure naming the missing
symbol — before each green):

- `src/retrieval/index_lock.rs::tests::second_acquire_fails_with_a_downcastable_lock_held_error_naming_the_holder_pid`
- `src/retrieval/index_lock.rs::tests::peek_in_returns_none_when_free`
- `src/retrieval/index_lock.rs::tests::peek_in_returns_holder_pid_when_locked`
- `src/retrieval/index_lock.rs::tests::peek_in_does_not_leave_the_lock_held`
- `src/tools/semantic/tests.rs::already_running_elsewhere_response_names_the_holder_pid`
- `src/tools/semantic/tests.rs::already_running_elsewhere_response_allows_an_unknown_holder_pid`
- `src/tools/semantic/tests.rs::running_elsewhere_indexing_block_names_the_holder_pid`

**Not unit-tested:** the full tool-to-real-OS-lock wiring (`IndexProject::call`
and `IndexStatus::call` actually calling `peek()` against a genuinely-held
lock). `peek`/`acquire` go through the real per-user runtime dir
(`per_user_runtime_dir()`, which reads `$XDG_RUNTIME_DIR`), and this project's
own test convention (see `index_lock.rs::tests::scratch`'s doc comment, and
`acquire_in_does_not_touch_the_real_runtime_dir`) is that tests must not write
there — mutating it via an env var for a test would also risk the documented
UB from concurrent `getenv`/`setenv` (`docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`). Covered instead by
live reproduction (below) — the same standard the original bug report itself
used.

Full gate: `cargo fmt`, `cargo clippy --lib --tests -- -D warnings`, `cargo test --lib` — 4302 passed, 0 failed, 8 ignored (pre-existing, unrelated).
## Workarounds

No longer needed — fixed. (Historical: cross-check `chunk_count`/`file_count`
across two `status` calls; if climbing, indexing is progressing regardless of
what `indexing.status` said.)
## Resume

N/A — fixed and live-verified.
## References

- `docs/issues/2026-08-23-index-build-fails-embed-batch-sparse-send.md` — the
  real failure this bug was discovered while re-verifying the fix for; uses
  the same `IndexingState::Failed` path and field shape, which is exactly why
  the two were briefly indistinguishable.
- `src/retrieval/index_lock.rs` — the cross-process file lock.
- `src/tools/semantic/index.rs`, `src/agent/mod.rs` — the tool implementation
  and per-agent state.
