---
status: fixed
opened: 2026-05-30
closed: 2026-06-05
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

**Confirmed 2026-05-30 (code trace).** The slot is `Agent { inner: Arc<RwLock<AgentInner>> }` (`src/agent/mod.rs:51`); `CodeScoutServer` is `#[derive(Clone)]` holding `agent: Agent` by value, so per-session clones share the same `Arc` → one global `AgentInner`. The MCP `RequestContext<RoleServer>` passed to `call_tool` (`src/server.rs:742`) exposes only `peer` (the single shared connection), `id` (a fresh per-*request* id, not a stable caller id), and `ct` (cancellation). **There is no per-subagent identity** — subagents ride the parent's one `Peer`, so the server cannot distinguish callers. This rules out a per-actor active-project map (no actor key to map on).
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

Scouted 2026-05-30. Options, re-ranked after the code trace:

- **~~Per-actor active-project map~~ — INFEASIBLE.** Needs a stable per-caller key; MCP `RequestContext` provides none (peer shared, request id per-call). Dead end — do not attempt.
- **~~Per-session keying~~ — does not fix the reported case.** Subagents share the parent's session/connection, so a `session_id`-keyed slot wouldn't isolate them (would only help true multi-client HTTP).
- **Per-request workspace pinning — the only fully-correct fix for concurrent subagents.** Each tool call optionally names its target workspace; tools resolve the project per-call from the request instead of the ambient global slot. **Large:** project resolution is ambient across ~17 files / 100+ call sites (`require_project_root`, `Agent::with_project`, `active_project()`), concentrated in `src/agent/mod.rs`. Needs its own plan + staged refactor. Not a single-session change.
- **Mitigation — make drift visible:** `activate`/`status` report the *true current* global active path (not the echoed request); warn when `activate` switches away from a path touched seconds ago. Cheap, ships standalone. Converts silent contamination into a detectable signal — but a subagent that sees the warning still can't pin, so it can only bail/serialize, not proceed correctly.
- **Mitigation — reject concurrent foreign activate:** error on `activate` to a different path while another is 'recent'. Forces serialization; needs an 'in-flight' heuristic.

No fix implemented yet — awaiting direction (per-request pinning is a planned effort vs. mitigation-now).
**Shipped 2026-05-30 (mitigation only — root cause NOT addressed; status `mitigated`):**
an activation drift-visibility guard. `Agent::note_activation` (`src/agent/mod.rs`) records the last activation `(root, Instant)`; on a `workspace(activate)` that switches to a *different* root within 5s of the prior one, the response carries a `concurrent_activation_warning` (wired in `ActivateProject::call`, `src/tools/config/mod.rs`). Pure decision in `Agent::concurrent_switch_warning`; regression test `concurrent_switch_warning_flags_rapid_foreign_switch` (`src/agent/mod.rs` tests). This converts the *silent* contamination into a visible signal — it does NOT remove the race (a clobbered subagent still can't pin its workspace).

**Root-cause fix** is planned separately: `docs/plans/2026-05-30-per-request-workspace-pinning.md` (per-request workspace pinning — the only correct fix for concurrent subagents; ~100 call sites, phased).

**Root-cause fix IN PROGRESS (2026-05-30, branch `feat/per-request-workspace-pinning`):** Phases 0–3 complete — the **READ surface (13 tools)** now resolves per-request via `ToolContext.workspace_override` → `with_project_at` / `project_root_for` / `require_project_root_for` / `security_config_for` accessors → a multi-resident `Workspace` registry (`ensure_resident`). **Regime-3 is FIXED for all reads**, proven by `read_file_concurrent_pins_no_cross_workspace_bleed` (5-task multi-thread, shared Agent, zero bleed). Writes + per-`Workspace` `Arc<RwLock>` locking + eviction remain (**Phase 4**, behind the lock-ordering gate). Status stays `mitigated` until the full fix (incl. writes) ships to `master`. Commit ledger + exact Phase-4 resume steps: the plan's "## Progress & Resume" section.
**✅ FIXED 2026-06-05 — regime-3 correctness fully closed and on `master`.** Verified all phases are ancestors of `origin/master`: reads (`ae596995`, `1b1fcc0c`, `898853a7`), write surface (`06990af7`, `070a9edd`, `b43995c2`, `6656c09a` — "regime-3 write surface complete"), read-tool `security_config` gap (`11358e82`), lock-ordering proof (`69c91896`), Phase 5 keystone + polish (`1a65bff2`, `b13c8c66`). Every per-request tool now resolves its project through `ctx.workspace_override`; concurrent reads AND writes to different pinned workspaces are proven bleed-free. Status flipped `mitigated`→`fixed` per the plan's criterion ("flip once 4a reaches master") during the 2026-06-05 verify-open reconciliation pass.

**Phase 4b (performance only — parallelize different-root writes via per-`Workspace` `Arc<RwLock>`, the ~100-site accessor ripple) remains DEFERRED by explicit call (2026-05-31).** It is NOT a correctness gap and is tracked in the still-active `docs/plans/2026-05-30-per-request-workspace-pinning.md`, not this bug. Re-open trigger lives in the plan (profiling shows different-root write serialization is a real bottleneck).

## Tests added
None yet — bug just logged. A regression test should assert that two interleaved
`activate(path_a)` / `activate(path_b)` + `status` sequences each observe their own root
(per-session isolation), or that the second activate is rejected (Option B).

## Workarounds

**Primary (fully avoids the bug today):** for parallel multi-workspace work, use **separate Claude Code windows** (separate processes → separate active-project slots) rather than parallel subagents within one session that each activate a different workspace. Confirmed: the race is specific to concurrent callers on *one* server process; separate CC instances each own their slot.

**Within a single session:** do not have parallel subagents activate *different* workspaces. If subagents must run concurrently, keep them all in the parent's single active workspace (don't switch).

**Auditing:** check the full `project_root` path, not `workspace.name` — name is identical across worktrees of one repo and hides the swap.
## Resume

N/A — fixed (regime-3 correctness, reads + writes, on `master`). The only remaining work, Phase 4b
(different-root write parallelism — performance, not correctness), is deferred by explicit decision
(2026-05-31) and tracked in `docs/plans/2026-05-30-per-request-workspace-pinning.md` (kept active for
4b). Re-open this bug only if a NEW cross-workspace contamination is observed *despite* per-request
pinning.

## References
- Related: `docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`
- `docs/manual/src/concepts/cross-process-write-serialization.md` (per-ActiveProject mutex)
- `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (mux concurrency claims)
- `src/lsp/mux/mod.rs:14,20` (per-path mux keying — drives the LSP-churn secondary effect)
- Recon: `docs/trackers/reconnaissance-patterns.md` R-11 (doc-vs-reality gaps)
