# Plan — Per-Request Workspace Pinning (regime 3 real fix)

**Status:** draft · **Opened:** 2026-05-30 · **Owner:** marius
**Fixes:** `docs/issues/2026-05-30-shared-server-global-active-project-race.md` (root cause)
**Supersedes mitigation:** the `concurrent_activation_warning` guard (`Agent::note_activation`) only *surfaces* the race; this plan *removes* it.

## Decision (ADR)

*Revised 2026-05-30 after architecture-snow-lion review.*

**Decision:** Resolve the target workspace **per request** — an explicit optional `workspace`
param plus a registry of activated projects keyed by canonical root — demoting the single
global active project to a per-session **default** for unpinned calls. This makes codescout a
**multi-project-resident** server, not a single-project server that switches.

**Context:** The active project is one `Arc<RwLock<AgentInner>>` (`src/agent/mod.rs:51`) shared
across every per-session server clone and every parallel subagent (subagents share the parent's
one MCP `Peer`; `RequestContext` exposes no per-caller identity, `src/server.rs:742`). ~100
ambient resolution sites read that single slot.

**Alternatives considered:**
- *Per-session keying* — rejected: subagents share the session; isolates nothing.
- *Per-actor map* — rejected: no actor key exists in `RequestContext`.
- *Mitigation only* (the shipped `concurrent_activation_warning`) — insufficient: surfaces the
  race, cannot remove it.

**Consequences:**
- *now easier:* N concurrent subagents each operate in a different workspace, correctly.
- *now harder:* ~100 call sites migrate; **codescout holds N live `ActiveProject`s at once** —
  importing a class of lifetime decisions it has never had to make (eviction, RAM ceiling, the
  meaning of "home" when N projects are live).

**Change scenarios absorbed:** "N concurrent subagents / sessions each operate in a different
workspace" — concrete, named, and the user's actual routine workflow.

**Revisit-when:** the registry's RAM ceiling forces evicting a project mid-write; or a second
non-subagent consumer needs pinning (HTTP multi-client). Either reopens the
default-vs-explicit-workspace contract.

**Confidence:** medium-high on direction; the load-bearing risk is the registry **lifetime
contract** (see Design), not the call-site migration.
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
root** (`HashMap<PathBuf, Arc<RwLock<ActiveProject>>>`) plus a per-session
`default_root: Option<PathBuf>` (what `activate` sets today). Request resolution order:
1. explicit `workspace`/`project_root` param → that registry entry (activate+cache on miss);
2. else `default_root`;
3. else error ("No active project").

**This is a shift in what codescout is** — from single-project-with-switch to
**multi-project-resident**. Name it plainly: the registry means N `ActiveProject`s are live at
once, each holding an LSP association and a `write.lock` handle. The registry is not an
implementation detail of the fix; it *is* the fix's identity.

**Lifetime contract (load-bearing — settle in Phase 1, NOT a Phase-5 decoration).**
A registry without a lifetime rule is an unbounded leak or a thrash, and you won't learn which
until a long-lived server has activated dozens of worktrees. Before the registry exists, answer:
- *When does a project leave the registry?* (idle TTL? LRU at a cap? explicit close?)
- *What happens to its `write.lock` handle on eviction?* — must release cleanly; **never evict an
  entry with an in-flight write.** Evict only quiescent entries.
- *What happens to its mux/LSP connection?* — drop the client; the mux idle-times-out on its own.
- *What is the cap / RAM ceiling?* — tie it to the existing LSP-pool LRU (`src/lsp/manager.rs`)
  so the two ceilings are **one** policy, not two competing ones.

**Access layer:** root-scoped twins of the ambient accessors — `with_project_at(root, |p| …)`,
`require_project_root_for(selector)` — with the existing ambient ones routed through
`default_root`. Tools obtain their selector from `ToolContext` (new optional
`workspace_override`, populated in `build_context`).

**Concurrency:** per-entry `RwLock` so calls on *different* roots never serialize; the
cross-process `write.lock` still serializes same-root writes.
## Phases

- **Phase 0 — Inventory & classify.** Enumerate the ~100 resolution sites (`grep` +
  `references` on the four accessors); tag read vs mutating, "needs pinning" vs "fine on
  default". Output: a checklist table in this plan.
- **Phase 1 — Registry + lifetime contract, no behavior change.** Introduce
  `projects: HashMap<PathBuf, Arc<RwLock<ActiveProject>>>` + `default_root` in `AgentInner`;
  `activate` populates the registry and sets `default_root`; all existing accessors resolve via
  `default_root`. **Define and implement the lifetime contract here** (eviction trigger,
  write.lock-safe eviction, RAM ceiling tied to the LSP LRU) — this is the boundary, not a later
  decoration. Full suite green = no regression.
- **Phase 2 — Selector plumbing.** Add `workspace_override` to `ToolContext`; populate from an
  optional `workspace` input field in `build_context`/`call_tool_inner`. No tool reads it yet.
- **Phase 3 — Migrate read tools.** `symbols`, `references`, `grep`, `semantic_search`,
  `read_file`, `tree` resolve via the selector. Add the concurrent-pinning regression (the
  5-subagent scenario from the bug file, asserting each pinned call reads its own root).
- **Phase 4 — Migrate write tools + lock-ordering validation gate.** `edit_code`, `edit_file`,
  `create_file`, `edit_markdown`, `memory` writes pin + keep `write.lock` semantics under a
  per-entry write guard. **Gate (must pass before Phase 4 ships):** prove the per-entry
  in-process lock and the cross-process `write.lock` flock have a single consistent acquisition
  order — no inversion, no cross-entry cycle. A deadlock here is the classic failure mode of
  this design.
- **Phase 5 — Tuning + retire the mitigation.** Tune the LRU/cap from Phase-1's contract.
  Update the three prompt surfaces (`src/prompts/source.md` server_instructions + onboarding,
  `builders.rs`) to document the `workspace` param; **bump `ONBOARDING_VERSION`** (param
  semantics reach the onboarding surface). **Remove** the `concurrent_activation_warning` guard
  for pinned flows (a time-window heuristic next to a real boundary is two mechanisms for one
  concern); resolve the unpinned-under-concurrency open question (Risks) — either declare it
  unsupported or retain the warning *solely* on the `default_root` path.
## Testing

- Port the bug file's 5-concurrent-activation scenario into an integration test that pins each
  call and asserts no cross-worktree read (the regression that proves regime 3 fixed).
- Per-entry concurrency: two pinned calls on different roots run without serializing.
- Default-path back-compat: unpinned calls behave exactly as today (the Phase-1 green suite).

## Risks / open questions

Eviction (now Phase 1) and lock-ordering (now a Phase-4 gate) have been promoted out of this
list into the phases that own them. What remains is genuinely open:

- **RAM ceiling value:** what cap? Empirical — measure `ActiveProject` + LSP-client footprint
  and set it against the LSP-pool LRU. (The *policy slot* is Phase 1; the *number* is Phase-5
  tuning. Whether 5 s / a given cap is well-calibrated is a measurement question — performance's
  domain, not architecture's.)
- **Param surface creep:** every pinnable tool grows an optional `workspace` field. Keep it
  optional; document once in server_instructions, not per-tool prose. Treat this as a
  **higher-stakes agentic-surface edit**, not a routine internal change.
- **`default_root` under concurrency (OPEN):** a subagent that does NOT pin still races the
  default slot. Decide before Phase 5: is unpinned-concurrent simply **unsupported**
  (documented), or does the warning guard survive *only* for the unpinned `default_root` path?
  This is the one question that keeps the mitigation partly alive — resolve it explicitly,
  don't let it default.
- **Scope:** Phases 3–4 touch ~100 call sites. Land phase-by-phase behind green suites; never
  leave the tree half-migrated across a session boundary.
## Why not the cheaper options (decided 2026-05-30)

- Per-session keying: subagents share the session → no isolation.
- Per-actor map: MCP `RequestContext` has no per-subagent key → impossible.
- Mitigation only (shipped): the `concurrent_activation_warning` makes the race *visible* but a
  clobbered subagent still can't proceed correctly — it can only bail or serialize.
