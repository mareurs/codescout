# MCP Disconnect on User-Cancel of Long Tool Calls

**Status:** PARTIAL FIX — codescout side complete; remaining issue is upstream (Claude Code).
**Date opened:** 2026-04-16
**Branch:** `experiments`

---

## Problem

When a user presses Escape during a long-running codescout MCP tool call (typically `run_command` with `sleep`/`until`/training scripts), Claude Code closes the MCP connection. User has to `/mcp` restart to keep using codescout. Loses LSP state, cold-start cost, all in-memory caches.

Originally suspected the recent refactor `12707fe` (`refactor(workflow): break up Onboarding::call and run_command_inner god functions`).

## Investigation Summary

Followed `superpowers:systematic-debugging` (Phase 1: gather evidence first).

### Evidence gathered
- `.codescout/diagnostic-*.log` for every codescout instance involved
- `~/.claude/projects/-home-marius-work-mirela-backend-kotlin/<session>.jsonl` (Claude Code session log)
- `git diff 12707fe~1 12707fe -- src/tools/workflow.rs`
- `rmcp` 1.3.0 source — `service.rs::serve_loop`

### Key findings

1. **Refactor 12707fe is INNOCENT.** Diff confirms the spawn / kill / timeout block in `run_command_inner` is byte-identical pre and post. Only `AbortOnDrop`, `TmpfileGuard`, `inject_tee`, and `handle_successful_output` were extracted as helpers — semantics preserved.

2. **Pre-existing bug:** `src/server.rs::call_tool` received `req_ctx: RequestContext<RoleServer>` but only consumed `req_ctx.peer`. The per-request `req_ctx.ct: CancellationToken` (rmcp's cancellation signal, fired when `CancelledNotification` arrives) was dropped on the floor.

3. **Pattern from session log (`b0d0b929-bf3b-42be-88c0-641f5d92964f.jsonl`):**
   - `[10:05:13Z] assistant: "codescout disconnected. Using native Bash:"`
   - `[10:05:24Z] user: "codescout reconnected"`
   - `[10:10:56Z] assistant: "Hook blocks Bash but codescout tools unavailable..."`
   - Both disconnects happened immediately after a `user-cancel` for a long `run_command`.

## What We Implemented (commits not yet made)

All on `experiments` branch, uncommitted as of writing.

### 1. `Cargo.toml`
Made `tokio-util` non-optional (was gated behind `http` feature). `CancellationToken` now always available.

### 2. `src/server.rs`
- `call_tool_inner` signature gained `cancel_token: tokio_util::sync::CancellationToken` parameter.
- Body wraps tool execution in `tokio::select!`:
  ```rust
  tokio::select! {
      biased;
      _ = cancel_token.cancelled() => Err(anyhow::anyhow!("request cancelled by client")),
      res = tokio::time::timeout(secs, tool_call_fut) => res.unwrap_or_else(...),
  }
  ```
- `call_tool` (the rmcp trait method) now passes `req_ctx.ct.clone()` to `call_tool_inner`.
- Test caller updated to pass `tokio_util::sync::CancellationToken::new()` (never fires).

### 3. `src/tools/workflow.rs::run_command_inner`
- Added `.kill_on_drop(true)` to the `tokio::process::Command` for the foreground shell child (Unix and Windows paths).
- Added a `PgidKillGuard` inside the `wait_with_output` async block that calls `libc::killpg(pgid, SIGKILL)` on `Drop` — kills the entire process group (sh + curl + grep + tee + ...), not just the immediate shell. Disarmed via `mem::forget` on successful completion.

### 4. New regression test
`src/server.rs::tests::call_tool_cancellation_kills_long_running_run_command` — runs `sleep 5 && touch <marker>` with `timeout_secs=30`, cancels after 200ms, asserts:
- Returns within 1s of cancel
- Result indicates cancellation
- Marker file does NOT exist after waiting past the original sleep window (proves child was reaped, not abandoned)

### Verification
- `cargo build` ✓
- `cargo clippy --all-targets -- -D warnings` ✓
- `cargo fmt --check` ✓
- `cargo test` ✓ — 1636 lib tests pass

## End-to-End Test Result

User performed a manual test on the live MCP server (instance `a993`):
- Issued `run_command(sleep 60 && touch /tmp/codescout-cancel-live-test, timeout_secs=90)`
- Pressed Escape ~41s into the sleep

**Diagnostic log evidence (`.codescout/diagnostic-a993.log`):**
```
11:26:33.718: tool_call run_command timeout_secs
11:27:14.935106: received CancelledNotification id=10 user-cancel
11:27:14.935120: cancelled id=10 reason="user-cancel"
11:27:14.935231: tool_done duration_ms=41216 ok=false   ← 125µs after cancel
11:27:14.946833: input stream terminated                 ← Claude Code closes 11ms later
11:27:14.946892: serve finished quit_reason=Closed
11:27:14.947016: service_exit instance=a993 reason=Closed
```

**Verified post-cancel:**
- `/tmp/codescout-cancel-live-test` does **not** exist → `touch` never ran → child was killed
- No orphan `sleep 60` processes anywhere → process group reaped

**Codescout side: WORKS PERFECTLY.** Cancel arm fires in 125µs, child tree dies, no orphans, no leaked resources.

## Remaining Issue

**Claude Code closes the MCP stdio connection 11ms after receiving our cancel response.**

This is independent of how codescout responds — Claude Code's cancel-on-Escape behavior tears down the MCP transport regardless. With stdio transport, that means the codescout subprocess gets EOF on stdin and exits cleanly. User then needs `/mcp` restart.

### User's hypothesis (worth testing)
> "Maybe we shouldn't return cancellation back."

Per MCP spec (need to confirm wording), the cancelling client MUST ignore any responses for the cancelled request. So sending a response is technically allowed but pointless. **Claude Code might be closing the connection specifically because we sent a response for a cancelled request** — i.e., treating it as a protocol violation.

### What rmcp does on CancelledNotification
From `rmcp-1.3.0/src/service.rs:986-995`:
```rust
Ok::<CancelledNotification, _>(cancelled) => {
    if let Some(ct) = local_ct_pool.remove(&cancelled.params.request_id) {
        tracing::info!(id=..., reason=..., "cancelled");
        ct.cancel();
    }
    cancelled.into()
}
```
- rmcp removes the per-request CT from the pool
- Calls `ct.cancel()` — this is what fires our `req_ctx.ct.cancelled()`
- The spawned handler task (lines ~963) is NOT killed by rmcp; it keeps running
- Whatever the handler returns gets sent as a response (`sink.send(response).await`)

**There is no rmcp API that says "drop this request without responding".** Our handler always returns a `Result`, and rmcp always sends it.

## Next Steps (for the next session)

### Step 1 — Confirm Claude Code's behavior is response-triggered
Test hypothesis: does Claude Code close the connection only if we respond, or unconditionally on cancel?

Option A — Add a permanent "swallow" branch in our select:
```rust
tokio::select! {
    biased;
    _ = cancel_token.cancelled() => {
        // Hang forever — never produce a response.
        // The spawned rmcp task will leak until the connection drops,
        // but we want to see if the connection STAYS UP without our response.
        std::future::pending::<...>().await
    }
    res = tool_call_fut => res,
}
```
Run the same `sleep 60 && touch ...` test. Press Escape. Check whether `input stream terminated` still happens.

- If YES: Claude Code closes regardless of our response → upstream issue, file with anthropics/claude-code.
- If NO: We need a clean way to suppress the response (since `pending::<>().await` leaks the task indefinitely).

### Step 2 — If Step 1 confirms response is the trigger, find the clean suppression path
- Check rmcp 1.4+ for a "drop request" API
- If none, propose one upstream OR fork rmcp temporarily
- Alternative: panic in the cancel arm and let `catch_unwind` swallow it (very ugly)

### Step 3 — Worst case workaround: switch transport
- HTTP/SSE transport survives stdio close-on-cancel
- Already supported in codescout (`http` feature, see `src/server.rs:780`)
- Would require user to switch their `.mcp.json` config from stdio to `http://localhost:PORT/mcp`
- Document tradeoff: needs extra port, auth token, but cancels won't kill the server

### Step 4 — Once all above is settled
- Commit the codescout fix as standalone improvement (regardless of Step 1 outcome — kill_on_drop + cancel-aware select! is a big win on its own: no orphan shell processes after cancel, no resources held until full timeout)
- Update `docs/TODO-tool-misbehaviors.md` if appropriate (this WAS a misbehavior of `run_command`)
- Cherry-pick to master per CLAUDE.md "Standard Ship Sequence"

## Key References

| File | What's there |
|------|--------------|
| `src/server.rs:202-358` | `call_tool_inner` (with cancel select!) |
| `src/server.rs:432-464` | `call_tool` trait impl (passes ct) |
| `src/server.rs:1311-1378` | new regression test |
| `src/tools/workflow.rs:3132-3351` | `run_command_inner` (with kill_on_drop + PgidKillGuard) |
| `Cargo.toml` | `tokio-util = "0.7"` (non-optional) |
| `~/.cargo/registry/src/.../rmcp-1.3.0/src/service.rs:920-1010` | rmcp serve_loop — cancel handling, no drop-without-response API |
| `.codescout/diagnostic-a993.log` | the live test evidence (cancel → 125µs → response → 11ms → close) |
| `~/.claude/projects/-home-marius-work-mirela-backend-kotlin/b0d0b929-bf3b-42be-88c0-641f5d92964f.jsonl` | original session that triggered investigation |

## How To Resume

1. Read this doc.
2. Check `git status` on `experiments` — the implemented changes should still be uncommitted.
3. Either commit them as-is (codescout-side fix is valuable on its own) and proceed to Step 1, OR proceed to Step 1 first and bundle.
4. The Step 1 experiment is the highest-value next move — it determines whether we have a clean fix path or need to escalate upstream.
