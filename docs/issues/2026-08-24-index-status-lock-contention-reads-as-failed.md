---
id: aa0cf06422d3d0aa
kind: bug
status: open
title: 'BUG: index(action="status")''s indexing.status stays "failed" from a stale lock-contention race while real progress (chunk_count, GPU) continues'
owners:
- marius
tags:
- indexing
- misleading-status
- concurrency
- codescout-tool
closed: null
opened: 2026-08-24
owner: marius
related:
- docs/issues/2026-08-23-index-build-fails-embed-batch-sparse-send.md
severity: medium
---


## Summary

After fixing the sparse-embedder bug, `index(action="status")` reported
`indexing.status: "failed"` with a lock-contention error on every poll — while
`chunk_count` climbed steadily and `nvidia-smi` showed the GPU at 95% utilization
running the embedder. The `indexing` field is not a live check of whatever
process actually holds the project's index lock; it is a one-shot echo of this
agent's own last local `index(action="build")` call, which lost a benign lock
race against an indexer that was genuinely working. The field renders
identically to a real failure, so there is no way to tell "broken" from
"someone else already has this, and it's fine" from the response alone.

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

*Not yet implemented — filed on notice per the bug-capture discipline, pending
a decision on direction.* Two candidates, not mutually exclusive:

1. Distinguish "this agent's attempt failed because another (live) holder has
   it" from a genuine failure in the `indexing` field's shape — e.g. a
   `"benign": true` / `"reason": "lock_contention"` marker set specifically
   when the error came from `index_lock::acquire` losing the race, vs. any
   other `Err` from `sync_project`.
2. On a lock-contention `Err` specifically, leave `ctx.agent.indexing` at its
   prior state (or `Idle`) instead of overwriting it with `Failed(...)` —
   since this agent made no real attempt that should displace whatever was
   already known, and a genuinely redundant `build` call shouldn't clobber the
   status of the build that's actually running.

## Tests added

N/A — no fix implemented yet.

## Workarounds

Don't trust `indexing.status: "failed"` alone. Cross-check `chunk_count` /
`file_count` across two calls a few seconds apart (Qdrant-backed, live) — if
they're climbing, indexing is progressing regardless of what `indexing` says.
`docker ps` / `nvidia-smi` corroborate further if available.

## Resume

Pick one of the two `Fix` candidates (or both) and implement in
`src/tools/semantic/index.rs` / `src/agent/mod.rs`. Add a unit test that
simulates a lock-acquire failure specifically (distinct from the existing
`IndexingState::Failed` tests in `src/tools/semantic/tests.rs`) and asserts the
response is distinguishable from a genuine failure. Run `cargo test`, `cargo
clippy -- -D warnings`, `cargo fmt` before closing.

## References

- `docs/issues/2026-08-23-index-build-fails-embed-batch-sparse-send.md` — the
  real failure this bug was discovered while re-verifying the fix for; uses
  the same `IndexingState::Failed` path and field shape, which is exactly why
  the two were briefly indistinguishable.
- `src/retrieval/index_lock.rs` — the cross-process file lock.
- `src/tools/semantic/index.rs`, `src/agent/mod.rs` — the tool implementation
  and per-agent state.

