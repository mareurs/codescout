# Plan — Per-Request Workspace Pinning (regime 3 real fix)

**Status:** draft · **Opened:** 2026-05-30 · **Owner:** marius
**Fixes:** `docs/issues/2026-05-30-shared-server-global-active-project-race.md` (root cause)
**Supersedes mitigation:** the `concurrent_activation_warning` guard (`Agent::note_activation`) only *surfaces* the race; this plan *removes* it.

## Decision (ADR)

*Revised 2026-05-30 (R1) after architecture-snow-lion review. **Re-revised 2026-05-30 (R2)** after a
pre-implementation scout (F-1 in `docs/trackers/concurrency-fix-session-log.md`) found the registry
granularity was one abstraction layer too low.*

**Decision:** Resolve the target workspace **per request**, and make the unit of concurrent residence
the **`Workspace`** — not the bare `ActiveProject`. Promote `AgentInner.workspace: Option<Workspace>`
(`src/agent/mod.rs:83`) to a registry `workspaces: HashMap<PathBuf, Arc<RwLock<Workspace>>>` keyed by
canonical workspace root, plus a per-session `default_workspace_root: Option<PathBuf>` for unpinned
calls. Within a resolved `Workspace`, the **existing** `Workspace::resolve_root(project, file_hint)`
(`src/workspace.rs:373`) already selects the sub-project — reuse it, do not rebuild it.

**Context:** The racing slot is not an `ActiveProject`; it is a whole `Workspace`. `Workspace`
(`src/workspace.rs:316`) is already a multi-project container: `projects: Vec<Project>`, each `Dormant`
or `Activated(Box<ActiveProject>)`, plus a `focused: Option<String>` pointer. `Agent::activate`
(`src/agent/mod.rs:330`) builds a fresh `Workspace` from `discover_projects(root)` and does
`inner.workspace = Some(ws)` — replacing the slot wholesale. A `Workspace`'s projects are all sub-paths
of one `root`; it structurally cannot hold worktree A and worktree B at once, so concurrent
multi-worktree residence requires N live `Workspace`s, not N `ActiveProject`s. ~100 ambient sites read
the focused project through the four accessors (`active_project()` → `workspace.focused_active().as_active()`,
`src/agent/mod.rs:96`).

**Alternatives considered:**
- *Flat `HashMap<PathBuf, ActiveProject>` registry* (the R1 design) — rejected: wrong granularity. It
  duplicates the `Workspace` abstraction, discards the existing intra-workspace multi-project support
  and `resolve_root`, and would force a full re-grain once wired. Heuristic 1: two structures that must
  always change together are one structure with a misleading name. (F-1, this session.)
- *Per-session keying* — rejected: subagents share the session; isolates nothing.
- *Per-actor map* — rejected: no actor key in `RequestContext` (`src/server.rs:742`).
- *Mitigation only* (shipped `concurrent_activation_warning`) — insufficient: surfaces the race, cannot
  remove it.

**Consequences:**
- *now easier:* N concurrent subagents each operate in a different workspace correctly; resolution
  centralizes through one access layer (`with_project_at`), shrinking the long-term ambient-coupling
  surface.
- *now harder:* `inner.workspace` becomes a map behind **per-entry locks** — a concurrency-model change,
  not a field swap; the lifetime contract must reason about a `Workspace` holding *multiple* activated
  projects, each with its own `write_lock`/`file_lock`; ~100 call sites migrate to selector-aware
  accessors.

**Change scenarios absorbed:** "N concurrent subagents / sessions each operate in a different worktree"
— concrete, named, the user's routine workflow.

**Revisit-when:** the registry's bound forces evicting a `Workspace` with an in-flight write in any of
its projects; or a second non-subagent consumer (HTTP multi-client) needs pinning. Either reopens the
default-vs-explicit-workspace contract.

**Confidence:** high on direction — *raised* by the scout. The correction reduces scope: `resolve_root`
and the multi-project container already exist, so the fix threads a selector through an existing
structure rather than building a parallel one. Remaining risk is the concurrency-model change (per-entry
locking, Phase 1) and execution discipline across ~100 sites (Phases 3–4), not the data model.
## Problem (one paragraph)

The active project is process-global: `Agent { inner: Arc<RwLock<AgentInner>> }`
(`src/agent/mod.rs:51`) holds a single `Workspace` slot — whose `focused` project is the `ActiveProject` every ambient site reads — shared across all
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

- **The racing slot is a `Workspace`, not an `ActiveProject`.** `AgentInner.workspace: Option<Workspace>`
  (`src/agent/mod.rs:83`). `Agent::activate` (`src/agent/mod.rs:330`) does `inner.workspace = Some(ws)` —
  replaces it wholesale. (Scouted R2, 2026-05-30.)
- **A multi-project registry already exists.** `Workspace` (`src/workspace.rs:316`) is
  `projects: Vec<Project>` + `focused: Option<String>`; each `Project` is `Dormant` or
  `Activated(Box<ActiveProject>)` (`src/workspace.rs:293-321`). A workspace can hold multiple activated
  projects (root + sub-projects).
- **A per-request resolver already exists.** `Workspace::resolve_root(project, file_hint)`
  (`src/workspace.rs:373`) resolves "explicit id > file hint > focused." Reuse it for Level-2 resolution;
  do not rebuild it.
- **Activation is expensive.** `Agent::activate` (`src/agent/mod.rs:330`) does I/O: config load, memory
  open, sub-project discovery (`spawn_blocking`), write-lock file open. You cannot re-activate per
  request — pinning must **select from the registry** of already-activated workspaces, activating+caching
  on first reference. (Same-root re-activation already reuses `write_lock`/`file_lock`/`dirty_files`.)
- **Project resolution is ambient and widespread:** ~100 call sites across 17 files —
  `require_project_root`, `Agent::with_project(|p| …)`, `active_project()`, `focused_project_root()` —
  concentrated in `src/agent/mod.rs`. Every one reads the single focused slot of the single live workspace.
- **Cross-process writes already serialize** via `.codescout/write.lock` flock — orthogonal to this
  change and stays.
- **`Workspace` + `ActiveProject` are light; the JVM is not.** The heavy resource (LSP client + JVM)
  lives in the separately-capped LSP pool (`src/lsp/manager.rs`), not in the registry entry. A registry
  of `Workspace`s bounds hashmap growth and releases `write.lock` fds; it is not a RAM manager.
## Design

**Shape (R2 — `Workspace` granularity).** Replace `AgentInner.workspace: Option<Workspace>` with:

```rust
workspaces: HashMap<PathBuf, Arc<RwLock<Workspace>>>,  // keyed by canonical workspace root
default_workspace_root: Option<PathBuf>,               // what activate() sets; unpinned calls land here
```

`activate(root)` inserts/updates the entry for `root` and sets `default_workspace_root = Some(root)`.
Same-root re-activation reuses the entry (preserving its projects' `write_lock`/`file_lock` — the logic
already in `activate`, now scoped to one map entry instead of a scan of the single workspace).

**Two-level resolution (R1 conflated these into one):**
1. *Level 1 — which `Workspace`?* explicit `workspace`/`project_root` param (canonical root) →
   `workspaces[root]` (activate+cache on miss); else `default_workspace_root`; else error ("No active
   project").
2. *Level 2 — which project inside it?* the **existing** `Workspace::resolve_root(project, file_hint)`
   (`src/workspace.rs:373`) — "explicit id > file hint > focused." Already built; reused **unchanged**.

So `default_workspace_root` (new, Level 1) and `focused` (existing, Level 2) are **orthogonal**: the
former picks the workspace, the latter picks the sub-project inside it. Neither replaces the other.

**This is what codescout becomes:** multi-**workspace**-resident (N repos/worktrees live at once), each
workspace itself already multi-project. The registry — N live `Workspace`s, each holding LSP associations
and `write.lock` handles for its activated projects — *is* the fix's identity, not an implementation
detail.

**Access layer:** root-scoped twins of the ambient accessors — `with_project_at(selector, |p| …)`,
`require_project_root_for(selector)` — where
`selector = { workspace_root: Option<PathBuf>, project: Option<String>, file_hint: Option<PathBuf> }`.
They lock the registry map briefly to **clone** the `Arc<RwLock<Workspace>>`, release the map lock, then
lock that one `Workspace` and run `resolve_root` + the caller's closure. Existing ambient accessors route
through `default_workspace_root`. Tools obtain the selector from `ToolContext` (new optional
`workspace_override`, populated in `build_context`).

**Concurrency:** per-`Workspace` `RwLock` so calls on *different* roots never serialize; the registry map
lock is held only for the `Arc` clone. The cross-process `.codescout/write.lock` flock still serializes
same-project writes across processes.
## Lifetime contract (Phase-1 detail)

*The load-bearing decision, settled here so Phase 1 implements it rather than Phase 5
discovering it. Grounded in the real `ActiveProject` shape, not a diagram.*

### What an entry owns (eviction must account for each)

*R2: an entry is a whole **`Workspace`** (`src/workspace.rs:316`), not one `ActiveProject`. Eviction
reasons about a **set** of activated projects.*

A `Workspace` owns:
- `projects: Vec<Project>` — each `Dormant` (drop is free) or `Activated(Box<ActiveProject>)`. A workspace
  may hold **multiple** activated projects (root + sub-projects), so the eviction predicate iterates them.
- `focused: Option<String>` — Level-2 pointer; pure data, drop is free.

Each **activated** project (`ActiveProject`, `src/agent/mod.rs:135`) owns:
- Pure data — `config`, `memory`, `private_memory`, `library_registry`, `head_sha`, `has_git_remote`:
  drop is free.
- `write_lock: Arc<tokio::Mutex<()>>` — in-process write serializer, **acquired FIRST** in the
  write-lock order (struct doc, `src/agent/mod.rs:122`).
- `file_lock: Arc<File>` — the fd whose `flock` *is* the cross-process `write.lock`. Releases when the
  last `Arc<File>` clone drops or its `WriteGuard` unlocks (`src/agent/write_guard.rs`).
- `dirty_files: Arc<Mutex<HashSet>>` — files written but not yet re-indexed (cleared on a successful
  `index_project`).
- `session_write_roots` — `approve_write` grants; session-scoped, re-grantable.
### Why the ceiling unifies trivially
The heavy resource (LSP client + JVM) is **not** owned by `ActiveProject` — it lives in the
separately-capped LSP pool (`src/lsp/manager.rs`: `max_clients` + idle-TTL + cost-tiered LRU,
lines 27–38 / 83–90). So the registry is **not** a RAM manager (the LSP pool already is one);
it only (a) releases the `write.lock` fd cleanly and (b) bounds hashmap growth. The registry
therefore *reuses* the LSP pool's idle-TTL and needs no independent RAM cap — one policy, by
construction, not by coordination.

### The contract

*R2: the entry is a `Workspace`; quiescence is a property of **all** its activated projects.*

1. **Eviction trigger:** idle-TTL reusing the LSP pool's `idle_timeout_secs` (default 300s); a background
   sweep on the same cadence as the LSP idle-evictor. A soft cap (≥ LSP `max_clients`) bounds the map;
   over-cap runs an LRU pass under the same quiescence rule. The unit evicted is a whole `Workspace`.
2. **Quiescence predicate (never evict a busy entry):** a `Workspace` is evictable only if **every**
   `Activated` project in it is quiescent — `write_lock.try_lock()` succeeds **and** `dirty_files` is
   empty — for *all* of them. Any one busy project skips the whole workspace this cycle (the cap is soft;
   momentary over-cap is fine, mirroring the LSP pool's re-check-between-locks, `manager.rs:344`).
3. **Write-safety proof (not just a hope):** per project, because `write_lock` is acquired *before* the
   flock, a free `write_lock` proves no `WriteGuard` exists, hence nothing holds the flock; dropping that
   project's `Arc<File>` releases it with **no truncated write**. Applied across all activated projects in
   the evicted workspace, the predicate in (2) is the safety property end to end.
4. **`default_workspace_root` is exempt.** The session's home workspace is never evicted — unpinned calls
   always resolve. Eviction only ever touches pinned, non-default entries.
5. **On evict:** drop the `Workspace` entry; its activated projects' LSP clients (one per project) are
   **not** force-killed — each ages out on its own LSP-pool idle-TTL (decoupled lifetimes, shared TTL
   value).
6. **Sweep lock ordering:** probe quiescence with `try_lock`, never a blocking lock, and never hold the
   registry-map lock while probing an entry's projects' `write_lock`s (check-then-act, re-validate after
   — `manager.rs:326-344` is the template). Same ordering the Phase-4 gate validates.
### One question this forces (decide in Phase 1, recommendation given)
If an entry is **dirty** (unindexed writes) when it would otherwise evict: (a) skip eviction
until the next `index_project` clears it, or (b) trigger a final index then evict? **Recommend
(a)** — simpler, and the stale-index risk is bounded (the next activation re-indexes). Revisit
only if dirty entries are observed pinning the map open.
## Phases

- **Phase 0 — Inventory & classify.** Enumerate the ~100 resolution sites (`grep` + `references` on the
  four accessors); tag read vs mutating, "needs pinning" vs "fine on default". Output: a checklist table
  in this plan.
- **Phase 1 — Registry + lifetime contract, no behavior change.** Promote
  `inner.workspace: Option<Workspace>` to `workspaces: HashMap<PathBuf, Arc<RwLock<Workspace>>>` +
  `default_workspace_root` in `AgentInner`; `activate` populates the map and sets the default; all existing
  accessors resolve via `default_workspace_root` → the one entry → the **existing** `Workspace::resolve_root`.
  **No new resolver is written** — `resolve_root` is reused. Move the AgentInner-wide lock to per-`Workspace`
  locks (the concurrency-model change). **Define and implement the lifetime contract here** (eviction over
  whole `Workspace`s; quiescence across all activated projects; map-bound tied to the LSP LRU) — the
  boundary, not a later decoration. Full suite green = no regression.
- **Phase 2 — Selector plumbing.** Add `workspace_override` to `ToolContext`; populate from an optional
  `workspace` input field in `build_context`/`call_tool_inner`. The selector is `{ workspace_root,
  project, file_hint }` — Level-2 fields feed the reused `resolve_root`. No tool reads it yet.
- **Phase 3 — Migrate read tools.** `symbols`, `references`, `grep`, `semantic_search`, `read_file`,
  `tree` resolve via `with_project_at(selector)` (Level-1 registry lookup → Level-2 `resolve_root`). Add
  the concurrent-pinning regression (the 5-subagent scenario from the bug file, asserting each pinned call
  reads its own root).
- **Phase 4 — Migrate write tools + lock-ordering validation gate.** `edit_code`, `edit_file`,
  `create_file`, `edit_markdown`, `memory` writes pin + keep `write.lock` semantics under the resolved
  project's per-`ActiveProject` write guard. **Gate (must pass before Phase 4 ships):** prove the
  per-`Workspace` `RwLock`, the per-project in-process `write_lock`, and the cross-process `write.lock`
  flock have a single consistent acquisition order — no inversion, no cross-entry cycle. A deadlock here is
  the classic failure mode of this design.
- **Phase 5 — Tuning + retire the mitigation.** Tune the LRU/cap from Phase-1's contract. Update the
  three prompt surfaces (`src/prompts/source.md` server_instructions + onboarding, `builders.rs`) to
  document the `workspace` param; **bump `ONBOARDING_VERSION`** (param semantics reach the onboarding
  surface). **Remove** the `concurrent_activation_warning` guard for pinned flows; resolve the
  unpinned-under-concurrency open question (Risks) — either declare it unsupported or retain the warning
  *solely* on the `default_workspace_root` path.
## Phase 0 — Call-site census (completed 2026-05-30)

*`references` (LSP-aware, not grep) on the full accessor surface. Two corrections to the Phase-0
framing above: the "four accessors" are actually **six**, and ~40% of raw refs are tests. Production
sites needing pinning ≈ **50**, validating the R2 "smaller than ~100" claim.*

### Accessor surface (6, not 4)

| Accessor | Def | Kind | Raw refs / files | Resolves |
|---|---|---|---|---|
| `AgentInner::active_project()` | `agent/mod.rs:96` | read primitive | 31 / 8 | focused project of the one live workspace |
| `AgentInner::active_project_mut()` | `agent/mod.rs:101` | **mutating** primitive | 18 / 6 (10 test) | mutable focused project |
| `Agent::require_project_root()` | `agent/mod.rs:505` | read → `Result<PathBuf>` | 44 / 21 (~15 test) | focused root or error |
| `Agent::with_project(\|p\|…)` | `agent/mod.rs:690` | read closure | 61 / 13 (~27 test) | `&ActiveProject` of focused |
| `Agent::project_root()` | `agent/mod.rs:962` | read → `Option<PathBuf>` | 18 / 11 (~3 test) | focused root or None |
| `Workspace::focused_project_root()` | `workspace.rs:339` | Level-2 primitive | 5 / 2 | (internal; called by `resolve_root`) |

`require_project_root`, `project_root`, and `with_project` all delegate to `active_project()` /
`focused_project_root()` internally — so **selector-aware resolution is added once at these primitives**
plus the closure accessor; tool handlers then change `agent.X()` → `agent.X_for(ctx.selector())`.

### Classification — production sites

| Bucket | Count (approx) | Migration |
|---|---|---|
| **Needs pinning** — per-request read/write tool handlers | ~50 | route through `with_project_for(selector)` / `require_project_root_for(selector)` / `project_root_for(selector)`; selector from `ctx.workspace_override` |
| **Fine on default** — session/server-level | ~18 | unchanged; resolve via `default_workspace_root` (server caps, heartbeat, usage recording, `mcp_resources`, prompt gen, onboarding, auto_register) |
| **Internal plumbing** — the accessors themselves | ~22 (in `agent/mod.rs`) | add `_for(selector)` variants here; not per-site edits |
| **Tests** | ~55 | unaffected (resolve via default); add new tests for pinned paths |

### Needs-pinning, by tool — READ (Phase 3)
`symbols` (`symbols.rs:225`), `references` (`references.rs:72`), `symbol_at` (`77,247`), `list_overview`
(`227,278,385`), `call_graph` (`414`), `tree` (`77,180`), `ast` (`15`), `grep` (`52`), `read_file`
(`62` + `active_project:339`), `read_markdown` (`76`), `semantic_search` (`active_project:193`),
`semantic/index` (`96,199`), `library` (`active_project:30`).

### Needs-pinning, by tool — WRITE (Phase 4; also per-entry `write_lock`)
`edit_file` (`243,360,429`), `create_file` (`47`), `edit_code` (`178`), `edit_markdown` (`627`),
`memory` writes (`mod.rs` + `active_project:427,795,833,905`), `approve_write` (`43`), `run_command`
(`161`). `active_project_mut` writers: `library` (`177`), `fs` (`387`), `semantic/index` (`149`).

### Highest-care sites (flag for Phase 3)
The ~10 **direct `inner.active_project()` grabs** (`memory/mod.rs`, `semantic_search.rs`, `read_file.rs`,
`library.rs`, `semantic/index.rs`) take their own `inner.read().await` then reach in — bypassing the
`with_project` closure. Under per-`Workspace` locking the lock acquisition changes shape, so these
**cannot swap the accessor in place**; they migrate to the closure form `with_project_for(selector,
|p| …)` or a new `active_project_for(selector)` helper that resolves the registry entry first. These are
the migration's sharp edges — not the closure callers, which retrofit mechanically.

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
- **`default_workspace_root` under concurrency (OPEN):** a subagent that does NOT pin still races the
  default slot. Decide before Phase 5: is unpinned-concurrent simply **unsupported**
  (documented), or does the warning guard survive *only* for the unpinned `default_workspace_root` path?
  This is the one question that keeps the mitigation partly alive — resolve it explicitly,
  don't let it default.
- **Scope:** Phases 3–4 touch ~100 call sites. Land phase-by-phase behind green suites; never
  leave the tree half-migrated across a session boundary.
## Why not the cheaper options (decided 2026-05-30)

- Per-session keying: subagents share the session → no isolation.
- Per-actor map: MCP `RequestContext` has no per-subagent key → impossible.
- Mitigation only (shipped): the `concurrent_activation_warning` makes the race *visible* but a
  clobbered subagent still can't proceed correctly — it can only bail or serialize.
