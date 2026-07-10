---
status: open
opened: 2026-07-10
closed:
severity: high
owner: marius
related: []
tags: [concurrency, lsp, async, lock-across-await, post-compact]
kind: bug
---

# BUG: LspManager::shutdown_all holds the clients mutex across every client's shutdown().await — a post_compact tool call can stall all concurrent LSP navigation

## Summary
`LspManager::shutdown_all` acquires `self.clients.lock().await` once and then, inside the loop, `.await`s `client.shutdown()` for every cached client before releasing the guard. `get`/`get_or_start` take the same `clients` lock, so while a `workspace(post_compact=true)` call drains the pool, every concurrent navigation tool call (`symbols`/`references`/`call_graph`/`symbol_at`) blocks for the cumulative shutdown time.

## Symptom (Effect)
An unrelated navigation tool call from another subagent/workspace hangs — with no error — for as long as the drain takes: up to (5 s join timeout + shutdown round-trip) × number of cached clients (pool cap 10). To the calling agent it looks like an unexplained multi-second-to-minute stall on `symbols`, with no indication a sibling `post_compact` flush is the cause.

## Reproduction
1. Warm several LSP clients (open files across languages so the pool holds N clients).
2. From one session/subagent call `workspace(post_compact=true)` (→ `ctx.lsp.shutdown_all().await`).
3. Concurrently, from another subagent/workspace, call `symbols(...)` needing `get_or_start`.
4. The `symbols` call blocks on `clients.lock().await` until the entire drain completes.

## Environment
codescout MCP server, branch `experiments`, 2026-07-10. Contention requires concurrent tool activity during a `post_compact` flush (parallel subagents / multi-workspace).

## Root cause
`src/lsp/manager.rs:1215-1225` (`shutdown_all`):
```rust
let mut clients = self.clients.lock().await;
for (key, client) in clients.drain() {
    match client.shutdown().await { ... }   // awaited while the clients guard is live
}
```
`clients` is the async `tokio::sync::Mutex<HashMap<LspKey, Arc<LspClient>>>` also taken by `get` (`manager.rs:1210`) and the `get_or_start` fast path. `LspClient::shutdown` sends an LSP `shutdown` request, waits on it, then joins the reader task with a 5 s timeout. The file's own `do_start` path deliberately drops the lock before shutting down a stale client for exactly this reason (per the agent, a comment near `manager.rs:~1040` warns that shutting down under the lock "would block all other get_or_start callers"); `shutdown_all` does the warned-against thing for every client back-to-back. Reachable live from `src/tools/config/mod.rs:248` (`post_compact=true`). The other two call sites (`src/server.rs:1364,1445`) are process-exit paths and not live-contention hazards.

## Evidence
- `shutdown_all` body + adjacent `get` (same lock) read directly this session (`manager.rs:1210-1225`).
- Live call site `ctx.lsp.shutdown_all().await` at `src/tools/config/mod.rs:248` under `post_compact` read directly.
- `client.shutdown()` join-timeout behavior + the `do_start` warning comment: reported by the finding agent, not independently re-read.
- Found by the n=2 shipped-hook re-eval probe (session 5efbda5f, concurrency direction).

## Hypotheses tried
1. **Hypothesis:** shutdown_all only runs at process exit, so no live contention. **Test:** grep call sites. **Verdict:** rejected — `config/mod.rs:248` invokes it from a live `workspace(post_compact=true)` tool call.

## Fix
Drop the `clients` guard before awaiting shutdowns: under the lock, `drain()` into a local `Vec`, release the guard, then `client.shutdown().await` each (optionally concurrently via `join_all`). Mirrors the lock-drop discipline `do_start` already uses.

## Tests added
N/A — not yet fixed. Regression is timing-dependent; at minimum an assertion that `shutdown_all` does not hold `clients` across the await (structural), or a concurrency test that a `get_or_start` completes promptly while a slow shutdown is in flight.

## Workarounds
Avoid `workspace(post_compact=true)` during heavy concurrent navigation; or serialize post_compact flushes to quiescent moments.

## Related unverified lead (same file, same probe — NOT independently confirmed)
The probe also flagged a **thundering-herd duplicate-start** race: on a start *failure*, `get_or_start` waiters each wake on `watch` `Some(false)`, and each unconditionally re-registers its own `starting` channel (`manager.rs:712-753`) and calls `do_start`, spawning N redundant LSP processes for one key; `StartingCleanup::drop` (`manager.rs:133-145`) unconditionally removes the `starting` entry, removing the barrier for late arrivals. Plausible but the watch-channel waiter logic was not re-read at the bytes here — verify before acting. Manifests as duplicate LSP spawns (SIGTERM'd on Arc drop, so transient, not a permanent leak) under concurrent first-start + transient failure.

## Resume
Fix `shutdown_all` per Fix (drain-then-release-then-await). Separately, verify the thundering-herd lead against `manager.rs:712-753` + `133-145` and file/close accordingly.

## References
- Provenance: n=2 shipped-hook re-eval, session 5efbda5f (A-17 follow-up). Hook-injected Phase 0 ran (memory + ledger) but the agent omitted the `Ledger checked:` receipt line — see the audit-log note event.
