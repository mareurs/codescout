---
status: open
opened: 2026-05-30
closed:
severity: high
owner: marius
related: [2026-05-30-cross-worktree-kotlin-jvm-shared-system-path]
tags: [concurrency, workspace, lsp, multi-agent]
kind: bug
---

# BUG: shared codescout server has one process-global active project — concurrent activations silently cross-contaminate reads

## Summary
A single codescout MCP server holds **one** active project as process-global state.
When multiple concurrent callers (e.g. parallel subagents sharing the one server)
call `workspace(action="activate")` against different paths, it is **last-writer-wins**:
each caller's activate *response* echoes its own path (looks successful), but the shared
server state is immediately clobbered by the next activator. Subsequent reads resolve
against whatever path won the race — a different worktree than the caller activated.
Because sibling worktrees of one repo share the same project **name**, the drift is
invisible unless you inspect `project_root`. Concurrent activation also churns the LSP
layer, producing a disconnect storm.

## Symptom (Effect)
5 subagents, each assigned a distinct backend-kotlin worktree, sharing one codescout
server. Each: `activate(own path, read_only=true)` → its response echoed its own path →
then 4× `workspace(status)`. Observed active `project_root` per agent:

```
agent assigned=backend-kotlin (root)      status: own, own, own, cc-exp-1   symbols: 1 ✓
agent assigned=weekly-pattern             status: cc-exp-1 ×3, cc-exp-3     symbols: LSP server disconnected
agent assigned=cc-exp-1                   status: cc-exp-3 ×4               symbols: LSP server disconnected
agent assigned=cc-exp-2                   status: cc-exp-3 ×4               symbols: LSP server disconnected
agent assigned=cc-exp-3                   status: cc-exp-3 ×4 (won race)    symbols: LSP server disconnected
```

3 of 5 agents read a `project_root` they never activated. `workspace.name` stayed
`backend-kotlin` for all five (same repo) — the contamination is path-only, name-invisible.
4 of 5 `symbols` calls failed with the verbatim error `LSP server disconnected`.

## Reproduction
1. One codescout MCP server, activated on any project.
2. Create ≥3 git worktrees of a second repo (distinct paths, same repo → same project name).
3. Dispatch N parallel agents on the shared server; each `activate(worktreeN, read_only=true)`
   then `workspace(status)` repeatedly and a `symbols(path=...)`.
4. Observe: `status.project_root` converges to the last activator for most agents;
   `symbols` intermittently returns `LSP server disconnected`.

Commit: `5436d06e` (experiments). Invoke: live `/mcp` server, parallel Agent dispatch.

## Environment
Linux, codescout 0.14.0, MCP stdio transport, server run with `--debug`.
Probe repo: `/home/marius/work/mirela/backend-kotlin` (Kotlin) + 4 worktrees.
Controller: Claude Code (Opus), branch `experiments`.

## Root cause
The active project is **process-global** server state, not per-MCP-session / per-connection.
All subagents share the parent session's single MCP server, so all `activate` calls mutate
one shared slot — last-writer-wins. The `activate` response is built from the caller's own
argument, so it reports success even though durable state is already someone else's.

Confirmed mechanism (behavioral + log). Exact owning field not yet pinned in src — best lead:
the per-`ActiveProject` `tokio::sync::Mutex<()>` referenced in
`docs/manual/src/concepts/cross-process-write-serialization.md` (§ "How It Works") implies a
single shared `ActiveProject` per process; `src/server.rs` holds the store. Needs a code trace
to cite `path:line` for the slot itself.

Secondary effect — LSP churn: the kotlin mux socket is keyed per workspace **path**
(`src/lsp/mux/mod.rs:14` `workspace_hash`, `:20` `socket_path_for_workspace`). As the global
active root thrashed, the server tried to stand up a *second* kotlin mux for a different
worktree path and it failed, falling back to direct LSP and disconnecting (see Evidence).

## Evidence

### Server diagnostic log — activation thrash + mux fragmentation
`/home/marius/work/claude/codescout/.codescout/diagnostic-7d74.log`:
```
INFO ... tool=workspace arg_keys=["action", "path", "read_only"]
INFO ... mux already running for kotlin, connecting to ".../codescout-kotlin-mux-26a9e85d58931839.sock"
WARN ... tool=symbols ... codescout::fs: LSP mux disconnect, retrying once: Mux connection lost
INFO ... tool=symbols ... mux already running for kotlin, connecting to ".../codescout-kotlin-mux-2a70f388bd6f77fa.sock"
WARN ... tool=symbols ... Mux startup failed for kotlin, falling back to direct LSP: Failed to connect to mux socket: ".../codescout-kotlin-mux-2a70f388bd6f77fa.sock"
```
Two distinct kotlin mux socket hashes (`26a9e85d…` = backend-kotlin root, `2a70f388…` = cc-exp-1)
appear in one interleaved burst — the active root moved between worktrees mid-sequence.

### Subagent status reports
See Symptom table — 3/5 agents observed a `project_root` they never activated; the activate
response had echoed each agent's own path regardless.

## Hypotheses tried
1. **Hypothesis:** active project is per-session, so concurrent agents are isolated.
   **Test:** 5 parallel agents, each activate distinct path + poll status.
   **Verdict:** rejected — status converged to the last activator across agents.
   **Evidence:** Symptom table.
2. **Hypothesis:** the LSP disconnects are unrelated to activation churn.
   **Test:** correlate `symbols` failures with the diagnostic log around the burst.
   **Verdict:** rejected — disconnects coincide with the per-path mux fragmentation
   (`Mux startup failed … falling back to direct LSP`).
   **Evidence:** server diagnostic log subsection.

## Fix
Plan (not yet implemented — design decision for the owner):
- **Option A (preferred):** scope the active project **per-MCP-session / per-connection**
  instead of process-global, so concurrent callers don't share one slot.
- **Option B:** if global state must stay, make `workspace(activate)` under a foreign
  active project return a `RecoverableError` ("another caller holds a different active
  project") rather than silently winning — surfaces the race instead of hiding it.
- Independent of A/B: the activate response should reflect *durable post-call* state, not
  echo the caller's argument, so "success" can't lie.

## Tests added
None yet — bug just logged. A regression test should assert that two interleaved
`activate(path_a)` / `activate(path_b)` + `status` sequences each observe their own root
(per-session isolation), or that the second activate is rejected (Option B).

## Workarounds
- On a single shared server, **do not activate different workspaces concurrently.**
  Serialize activations (one workspace at a time), or give each workspace its own
  codescout server process (separate Claude Code session) — at the JVM RAM cost
  documented in the related cross-worktree bug.
- When auditing which workspace is active, check `project_root`, **not** `name` — name
  is identical across worktrees of one repo and masks the drift.

## Resume
Trace the active-project slot: `symbols(name="ActiveProject", include_body=true)` and grep
`src/server.rs` for where the activate handler writes it; confirm it is a single
process-global field (not keyed by session id). Then prototype Option A (per-connection
scoping) or Option B (reject-foreign-activate) and add the interleaved-activate regression.

## References
- Related: `docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`
- `docs/manual/src/concepts/cross-process-write-serialization.md` (per-ActiveProject mutex)
- `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (mux concurrency claims)
- `src/lsp/mux/mod.rs:14,20` (per-path mux keying — drives the LSP-churn secondary effect)
- Recon: `docs/trackers/reconnaissance-patterns.md` R-11 (doc-vs-reality gaps)
