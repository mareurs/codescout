# Plan — Per-Request Workspace Pinning (regime 3 real fix)

**Status:** draft · **Opened:** 2026-05-30 · **Owner:** marius
**Fixes:** `docs/issues/2026-05-30-shared-server-global-active-project-race.md` (root cause)
**Supersedes mitigation:** the `concurrent_activation_warning` guard (`Agent::note_activation`) only *surfaces* the race; this plan *removes* it.

## Problem (one paragraph)

The active project is process-global: `Agent { inner: Arc<RwLock<AgentInner>> }`
(`src/agent/mod.rs:51`) holds a single focused `ActiveProject`, shared across all
per-session server clones and — critically — across all parallel subagents, which ride the
parent's one MCP `Peer` (`RequestContext` exposes no per-caller identity;
`src/server.rs:742`). Concurrent `workspace(activate, X)` calls are last-writer-wins, so a
subagent that activated worktree A silently reads worktree B's files after another subagent
activates B. Per-session keying can't help (subagents share the session); a per-actor map
can't help (no actor key). The only correct fix is to resolve the target workspace
**per request** rather than from ambient global state.

## Goal

A tool call can name its target workspace; the call resolves project state for *that*
workspace regardless of what any concurrent caller does. The global "active project" demotes
to a per-session **default** for calls that don't pin a workspace — preserving today's
single-agent ergonomics while making concurrent multi-workspace correct.

## Constraints / facts (scouted 2026-05-30)

- **Activation is expensive.** `Agent::activate` (`src/agent/mod.rs:323`) does I/O: config
  load, memory open, sub-project discovery (`spawn_blocking`), write-lock file open. You
  cannot re-activate per request — pinning must **select from a cache** of already-activated
  projects, activating+caching on first reference.
- **Project resolution is ambient and widespread:** ~100 call sites across 17 files —
  `require_project_root`, `Agent::with_project(|p| …)`, `active_project()`,
  `focused_project_root()` — concentrated in `src/agent/mod.rs`. Every one reads the single
  focused slot today.
- **Cross-process writes already serialize** via `.codescout/write.lock` flock — orthogonal
  to this change and stays.
- **ActiveProject is non-trivial state** (root, config, memory, read_only, dirty_files, LSP
  association). A registry of them has an RAM cost → needs eviction (mirror the LSP pool's
  LRU, `src/lsp/manager.rs`).

## Design

**Shape:** promote `AgentInner`'s single focused project to a **registry keyed by canonical
root** (`HashMap<PathBuf, ActiveProject>`), plus a per-session `default_root: Option<PathBuf>`
(what `activate` sets today). Request resolution order:
1. explicit `workspace`/`project_root` param on the call → that registry entry (activate+cache
   on miss);
2. else the session `default_root`;
3. else error (as today: "No active project").

**Access layer:** add root-scoped twins of the ambient accessors —
`with_project_at(root, |p| …)`, `require_project_root_for(selector)` — and route the existing
ambient ones through `default_root`. Tools obtain their selector from `ToolContext` (new
optional `workspace_override`, populated in `build_context` from the request input).

**Concurrency:** per-entry locking (e.g. `HashMap<PathBuf, Arc<RwLock<ActiveProject>>>`) so two
calls on *different* roots never serialize on one global write lock; the cross-process
`write.lock` still serializes same-root writes.

## Phases

- **Phase 0 — Inventory & classify.** Enumerate the ~100 resolution call sites; tag each
  read-only vs mutating and "needs pinning" vs "fine on default". Output: a checklist table in
  this plan. (Use `grep` + `references` on the four accessor symbols.)
- **Phase 1 — Registry, no behavior change.** Introduce `projects: HashMap<PathBuf, …>` +
  `default_root` in `AgentInner`; `activate` populates the registry and sets `default_root`;
  all existing accessors resolve via `default_root`. Full suite green = no regression.
- **Phase 2 — Per-call selector plumbing.** Add `workspace_override` to `ToolContext`; populate
  in `build_context`/`call_tool_inner` from an optional `workspace` input field. No tool reads
  it yet.
- **Phase 3 — Migrate read tools first.** `symbols`, `references`, `grep`, `semantic_search`,
  `read_file`, `tree` resolve via the selector (`with_project_at`). Add the concurrent-pinning
  regression: the 5-subagent scenario from the bug file, asserting each pinned call reads its
  own root.
- **Phase 4 — Migrate write tools.** `edit_code`, `edit_file`, `create_file`, `edit_markdown`,
  `memory` writes — pin + keep `write.lock` semantics. Per-entry write guard.
- **Phase 5 — Eviction + docs.** LRU-evict idle registry entries (cap + RAM ceiling). Update
  the three prompt surfaces (`src/prompts/source.md` server_instructions + onboarding,
  `builders.rs`) to document the `workspace` param; **bump `ONBOARDING_VERSION`** (param
  semantics reach the onboarding surface). Remove/relax the `concurrent_activation_warning`
  guard once pinning is the default for multi-workspace.

## Testing

- Port the bug file's 5-concurrent-activation scenario into an integration test that pins each
  call and asserts no cross-worktree read (the regression that proves regime 3 fixed).
- Per-entry concurrency: two pinned calls on different roots run without serializing.
- Default-path back-compat: unpinned calls behave exactly as today (the Phase-1 green suite).

## Risks / open questions

- **RAM:** N cached `ActiveProject` + their LSP clients. Eviction policy + ceiling needed
  (Phase 5). Tie to the existing LSP LRU.
- **`write.lock` interaction** under per-entry locks — confirm no deadlock between the
  in-process per-entry guard and the cross-process flock.
- **Param surface creep:** every pinnable tool grows a `workspace` field. Keep it optional and
  documented once in server_instructions, not per-tool prose.
- **Scope:** Phases 3–4 touch ~100 call sites. Land phase-by-phase behind green suites; never
  leave the tree half-migrated across a session boundary.

## Why not the cheaper options (decided 2026-05-30)

- Per-session keying: subagents share the session → no isolation.
- Per-actor map: MCP `RequestContext` has no per-subagent key → impossible.
- Mitigation only (shipped): the `concurrent_activation_warning` makes the race *visible* but a
  clobbered subagent still can't proceed correctly — it can only bail or serialize.
