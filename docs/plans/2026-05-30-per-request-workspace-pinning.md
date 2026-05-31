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

> **Correction (2026-05-30, Phase-4 scout).** The per-phase prose below still
> assigns per-`Workspace` `Arc<RwLock<_>>` locking + the eviction sweep to
> **Phase 3**. In implementation they were deferred *again*: Phase 3 shipped
> multi-residence + read-tool pinning **under the existing single `AgentInner`
> lock** (map values are plain `Workspace`, not `Arc<RwLock<_>>`); the
> per-`Workspace` `RwLock` swap, eviction, and write-tool migration are all
> **Phase 4**. `## Design`'s `Arc<RwLock<Workspace>>` is the *target* shape, not
> yet implemented. The authoritative ledger is `## Progress & Resume`.
>
> **Phase 4 is itself split (correctness before performance):**
> - **4a — write-tool per-request migration (correctness).** Migrate write tools
>   (`edit_file`, `edit_code`, `create_file`, `edit_markdown`, `memory` writes,
>   …) to resolve their project from `ctx.workspace_override` — the exact mirror
>   of the Phase-3 read migration — **under the existing lock**. This alone
>   closes regime-3 for writes (a pinned write reaches the right project
>   regardless of a concurrent subagent's `activate`). Different-root writes
>   still serialize on the `AgentInner` write lock — correct, just not yet
>   parallel.
> - **4b — per-`Workspace` `Arc<RwLock<Workspace>>` + eviction (performance).**
>   The large ~100-site accessor ripple that lets different-root calls stop
>   serializing. Gated by `## Phase 4 — Lock-Ordering Proof`. Its own focused
>   effort, landed behind a green suite.

*Sequencing refined 2026-05-30 during Phase-1 implementation: per-`Workspace` locking and the eviction
sweep moved from Phase 1 to Phase 3, because both are unobservable and untestable until pinning creates
concurrent entries (one entry never contends; the default is eviction-exempt). A mechanism ships with the
test that proves it. The lifetime contract stays **defined** in Phase 1 (above); its **mechanism** lands
in Phase 3.*

- **Phase 0 — Inventory & classify.** *(done — see "Phase 0 — Call-site census" below.)* Enumerate the
  resolution sites (`references` on the accessor surface); tag read vs mutating, needs-pinning vs
  fine-on-default.
- **Phase 1 — Registry data structure + default, no behavior change.** Promote
  `inner.workspace: Option<Workspace>` to `workspaces: HashMap<PathBuf, Workspace>` +
  `default_workspace_root` in `AgentInner`, all accessors resolving through a new `default_workspace()`
  helper. The map holds **at most one entry** — `activate` clears and reinserts, exactly mirroring the
  previous single-slot drop-and-replace — so behavior is identical and the full suite stays green. Map
  values are plain `Workspace` (not yet `Arc<RwLock<_>>`): the single `AgentInner` lock is unchanged, so
  every accessor signature (`active_project() -> Option<&ActiveProject>`, …) is preserved and no call site
  outside the field swap changes. The lifetime contract is *defined* (above); its *mechanism* is Phase 3.
- **Phase 2 — Selector plumbing.** Add `workspace_override` to `ToolContext`; populate from an optional
  `workspace` input field in `build_context`/`call_tool_inner`. The selector is `{ workspace_root,
  project, file_hint }` — Level-2 fields feed the reused `resolve_root`. No tool reads it yet.
- **Phase 3 — Multi-residence + per-entry locks + migrate read tools.** Lift Phase-1's
  clear-on-activate so the registry holds N live workspaces; change entry values to
  `Arc<RwLock<Workspace>>` for per-entry locking (calls on different roots stop serializing); implement
  the eviction sweep per the lifetime contract. Add the selector-aware accessors (`with_project_at`,
  `require_project_root_for`, `project_root_for`) — Level-1 registry lookup → Level-2 `resolve_root` —
  and migrate the Phase-0 READ list (`symbols`, `references`, `grep`, `semantic_search`, `read_file`,
  `tree`, `ast`, `symbol_at`, `call_graph`, `list_overview`, `read_markdown`, `library`). The ~10 direct
  `inner.active_project()` grabs migrate to the closure form. **Add the concurrent-pinning regression**
  (the 5-subagent scenario from the bug file) — the test that proves both the lock model and regime-3
  fixed.
- **Phase 4 — Migrate write tools + lock-ordering validation gate.** `edit_code`, `edit_file`,
  `create_file`, `edit_markdown`, `memory` writes pin + keep `write.lock` semantics under the resolved
  project's per-`ActiveProject` write guard. **Gate (must pass before Phase 4 ships):** prove the
  per-`Workspace` `RwLock` (Phase 3), the per-project in-process `write_lock`, and the cross-process
  `write.lock` flock have a single consistent acquisition order — no inversion, no cross-entry cycle. A
  deadlock here is the classic failure mode of this design.
- **Phase 5 — Tuning + retire the mitigation.** Tune the LRU/cap from the lifetime contract. Update the
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

## Progress & Resume (2026-05-30)

**Status: Phases 0–3 COMPLETE — the entire READ surface honors per-request pinning;
regime-3 is fixed for all reads.** Work lives on branch
**`feat/per-request-workspace-pinning`** (forked from `experiments`). Nothing pushed;
`master`/`experiments` untouched. Every commit green: `check`/`clippy --all-targets`,
`fmt`, suite **2626 pass + 1 pre-existing reranker env failure** (`tests/retrieval_integration.rs:88`,
external Qdrant stack — unrelated).

### Commit ledger
- `ccee0849` — R2 plan re-grain (Workspace registry, not ActiveProject)
- `b3592b2c` — Phase 0 call-site census
- `840259cc` — Phase 1: registry data structure, no behavior change
- `05fa47a5` — Phase 2: `ToolContext.workspace_override` plumbing
- `ae596995` — Phase 3: `read_file` + regime-3 proof + the core machinery
- `1b1fcc0c` — Phase 3: read-nav tools batch + `require_project_root_for`
- `898853a7` — Phase 3: direct-grab reads (`semantic_search`, `library` list)

### Machinery built (`src/agent/mod.rs`)
- `AgentInner.workspaces: HashMap<PathBuf, Workspace>` + `default_workspace_root: Option<PathBuf>`
  (replaced `workspace: Option<Workspace>`); `default_workspace()/_mut()` helpers.
- `AgentInner::build_workspace(root, read_only, ProjectResources)` — under-lock assembly w/ lock-reuse scan.
- `Agent::load_project_resources(root)` — lock-free I/O → `ProjectResources` struct.
- `Agent::ensure_resident(root, read_only)` — load+cache WITHOUT clear/default-change (multi-residence).
- `Agent::with_project_at(workspace_override, |p| ...)` — Level-1 pin → focused `&ActiveProject`.
- `Agent::project_root_for / require_project_root_for / security_config_for` — pinned twins;
  `project_security_config(p)` extracted.
- Tools read `ctx.workspace_override` (canonicalized in `server.rs::extract_workspace_override`).

### Read tools migrated (13)
`read_file`, `symbols`, `references`, `symbol_at`, `list_overview`, `call_graph`, `symbol/query`,
`tree`, `ast`, `grep`, `read_markdown`, `semantic_search`, `library` (list).

### Proof tests (regime-3)
- `src/tools/read_file.rs`: `read_file_honors_workspace_override_pin`,
  `read_file_concurrent_pins_no_cross_workspace_bleed` (5-task multi-thread, zero bleed).
- `src/agent/mod.rs`: `require_project_root_for_resolves_pin_over_default` (accessor seam),
  `activate_registers_default_workspace_by_canonical_root` (Phase-1 invariant).

### NEXT — Phase 4 (writes + per-Workspace locking + eviction). START WITH THE LOCK-ORDERING PROOF.
> **Phase 5 update (2026-05-31): KEYSTONE DONE (`1a65bff2`).** The `workspace` pin is now
> **advertised** on every pinnable tool — central `Tool::pinnable()` (default true, except
> session/global/registry + librarian) + `inject_workspace_param` in `list_tools` + tests. With
> 4a's resolution machinery + the keystone, **the regime-3 fix is now usable** (agents can discover
> and pass `workspace`; the injected property carries inline usage guidance). **Remaining Phase 5:**
> (a) broader `server_instructions` / onboarding prose (a "when to pin" paragraph — the per-property
> description already gives inline guidance, so this is refinement); (b) `ONBOARDING_VERSION` bump
> **only if** the onboarding/`builders.rs` surface changes (server_instructions is live-on-connect, no
> bump); (c) the unpinned-concurrent open question — recommend keeping `concurrent_activation_warning`
> ONLY on the unpinned default path and retiring it for pinned flows; (d) **live `/mcp` end-to-end
> verify** (user step: `cargo build --release` + reconnect, confirm a pinned subagent hits the right
> workspace) before the ship sequence.
> **Update (2026-05-31, session 2).** 7/7 pinned write accessors built (incl.
> `invalidate_call_edges_for` + `call_edges_project_id_for`); the latent Phase-3 `call_graph`
> READ `project_id` gap is **closed** (it pinned root but resolved the cache namespace ambiently).
> **6 write tools fully migrated + audited complete:** `edit_file`, `create_file`,
> `symbol/edit_code`, `markdown/edit_markdown`, `approve_write`, `library`(register).
> **DONE (session 2 cont.):** `run_command` (@161/162) ✅, `core::guards::guard_worktree_write` (@18) ✅,
> the read-tool `security_config` gap (`tree`/`ast`/`grep`/`read_markdown`) ✅, and the **concurrent-write
> regression** (`create_file_concurrent_pins_no_cross_workspace_bleed` + `..._honors_workspace_override_pin`
> in `edit_file/tests.rs`) ✅ — regime-3 writes PROVEN for the migrated surface.
>
> **✅ 4a COMPLETE (2026-05-31) — regime-3 write correctness fully closed.** All per-request tools now
> resolve their project through `ctx.workspace_override`: `memory` (15 sites + `resolve_memory_dir`),
> `config` (7), `semantic/index` (7 — read blocks via `with_project_at`, the mut block via
> `with_project_at_mut`), on top of the 8 done earlier (`edit_file`/`create_file`/`edit_code`/
> `edit_markdown`/`approve_write`/`library`/`run_command`/`core::guards`). The read-tool `security_config`
> gap (`tree`/`ast`/`grep`/`read_markdown`) is closed. Concurrent-write regression GREEN; full lib suite
> 2571 pass. `onboarding` (16) + `usage` (@40) left ambient by design (session-level, not per-request).
>
> **Remaining = Phase 4b (PERFORMANCE, not correctness):** per-`Workspace` `Arc<RwLock<Workspace>>` +
> eviction (the ~100-site accessor ripple), gated by `## Phase 4 — Lock-Ordering Proof`. Regime-3 is
> correctness-fixed WITHOUT it; 4b only removes different-root write serialization on the single
> `AgentInner` lock. Then **Phase 5** (prompt surfaces + `ONBOARDING_VERSION` + retire
> `concurrent_activation_warning`). Bug `2026-05-30-shared-server-global-active-project-race.md`:
> flip `mitigated`→`fixed` (regime-3 correctness scope) once 4a reaches master.
> **Phase 4 progress (2026-05-30, this session):**
> - ✅ **Step 1 — lock-ordering proof** → see **`## Phase 4 — Lock-Ordering Proof (the gate)`** below.
> - ✅ **4a keystone — central write-gate pinned.** `server::acquire_write_guard_if_writing` now
>   resolves the `write_lock`/`file_lock` via `with_project_at(ctx override)` instead of ambient
>   `with_project`. Verified `cargo check` + `cargo test --lib` green (2568 pass; lone failure is the
>   known `first_artifact_call_appends_librarian_guide_body_v2` env-isolation flake — passes solo).
> - ⏳ **4a remainder — per-tool path/state pinning (NEXT).** The central gate fixes the *lock*, not the
>   *path*: each write tool's own resolution must pin too. SCOPED:
>   - **Pinned accessors — 6/7 BUILT this session (compile + `clippy --lib` clean):**
>     `session_write_roots_snapshot_for`, `add_session_write_root_for`, `mark_file_dirty_for`,
>     `dirty_files_arc_for` (all route through the *read* `with_project_at` — `Arc<Mutex>` interior
>     mutability), plus `with_project_at_mut` (the *mutating* twin: `inner.write()` + `&mut ActiveProject`,
>     for direct field assignment like `p.config = …` / `library_registry.register`) and
>     `reload_config_if_project_toml_for` (via `with_project_at_mut`).
>   - **⚠ 7th accessor STILL TO BUILD — `invalidate_call_edges_for`.** Subtlety: `invalidate_call_edges`
>     derives `project_id` via `call_edges_project_id()`, which resolves the **default** workspace's
>     `focused` id — NOT `p.project_id()`. The pinned twin must derive `project_id` from the **pinned**
>     workspace's `focused` (likely add a `call_edges_project_id_for`), and the doc says `call_graph`'s
>     edge *upsert* calls the same method — so **verify the already-migrated `call_graph` READ tool pins
>     its `project_id` too**, else reads/writes land in the wrong DB namespace under a pin. Investigate
>     before building.
>   - **Then migrate write tools — each COMPLETELY** (never half: a tool with root pinned but
>     `invalidate_call_edges`/`reload_config` ambient is inconsistent). Audit EVERY `ctx.agent.*` call;
>     no compiler net. Full per-tool maps scouted this session:
>     - `edit_file/mod.rs`: `security_config` (212,244,361,430), `require_project_root` (243,360,429),
>       `session_write_roots_snapshot` (245,362,431), `reload_config_if_project_toml` (344,468),
>       `invalidate_call_edges` (346,385,470), `mark_file_dirty` (347,386,471).
>     - `symbol/edit_code.rs`: trio (178-180) + `invalidate_call_edges`/`mark_file_dirty` ×6 (306/307,
>       465/466, 635/636, 659/660, 676/677, 751/752).
>     - `create_file.rs` (47-49 trio + 66/67), `markdown/edit_markdown.rs` (627-629 trio + 813/814),
>       `approve_write.rs` (43 root, 47 security, 59 `add_session_write_root`).
>     - `active_project_mut` writers → `with_project_at_mut`: `library.rs` (179-180, 392-393 register),
>       `semantic/index.rs` (148-149; also `dirty_files_arc` 288, `require_project_root` 187).
>     - `memory/mod.rs` writes (46/84/131/214/593/682/732 `with_project`; 295/335/427/765/795/833/905
>       `active_project`; 619/635/921 `require_project_root`) — classify read-vs-write per site.
>   - **Add the concurrent-WRITE regression** — two pinned writes to different workspaces, no cross-bleed;
>     mirror `read_file_concurrent_pins_no_cross_workspace_bleed`.
> - ⏳ **4b — per-`Workspace` `Arc<RwLock<Workspace>>` + eviction (performance).** The large ~100-site
>   ripple, gated by the proof above. After 4a closes regime-3 for writes.
>
> The numbered steps below are the original detailed plan; 4a maps to step 4, 4b to steps 2–3+5.

1. **Lock-ordering proof FIRST (the gate).** Per-`Workspace` `RwLock` + per-project `write_lock`
   (`Arc<Mutex>`) + cross-process `write.lock` flock: prove one consistent acquisition order, no
   inversion, no cross-entry cycle. Mirror `src/lsp/manager.rs:326-344` (check-then-act; never hold
   the registry-map lock while probing an entry's `write_lock`). This is the design's classic deadlock
   failure mode — write the proof/test before any write tool moves.
2. **Per-`Workspace` `Arc<RwLock<Workspace>>`** — change `workspaces` values so different-root calls
   don't serialize. Second touch of `with_project_at` + the accessors (anticipated in the R2 ADR).
3. **Eviction sweep** — see "## Lifetime contract": idle-TTL reusing the LSP pool's `idle_timeout_secs`;
   quiescence = EVERY activated project in a workspace has `write_lock.try_lock()` free AND `dirty_files`
   empty; `default_workspace_root` exempt; drop the entry (LSP client ages out on its own TTL).
4. **Migrate write tools** under the resolved project's write guard (likely need `with_project_at_mut` /
   `require_write_root_for` twins): `edit_file`, `edit_code`, `create_file`, `edit_markdown`,
   `approve_write`, `core/guards`, `run_command`, `memory` writes, `semantic/index` (reindex),
   `library` register (`active_project_mut`). Sites in Phase 0 "Needs-pinning … WRITE" (line refs may drift).
5. Add a concurrent-WRITE regression (two pinned writes to different workspaces: no serialize, no corrupt).

### Then Phase 5
Document the `workspace` param in the three prompt surfaces (`src/prompts/source.md`
server_instructions + onboarding, `builders.rs`); **bump `ONBOARDING_VERSION`** (param semantics reach
onboarding); **remove** `concurrent_activation_warning` for pinned flows; resolve the unpinned-
`default_workspace_root`-under-concurrency open question (Risks).

### Ship (when complete)
Standard Ship Sequence (CLAUDE.md); **summon Docs Lotus Frog before merging to `master`**; update F-1
SHA citations to master-side after cherry-pick. Bug: `docs/issues/2026-05-30-shared-server-global-active-project-race.md`.

## Phase 4 — Lock-Ordering Proof (the gate)

**Status: design proof — written before any write tool moves (Phase 4 step 1).
Graduates to `docs/architecture/workspace-lock-ordering.md` (cited from the new
lock code) when Phase 4 lands.** Scouted 2026-05-30 against the two reference
implementations the codebase already ships.

Phase 4 adds a per-`Workspace` `RwLock` (step 2) between the registry-map lock
and the per-project write locks, plus an eviction sweep (step 3). Both touch the
classic deadlock failure mode. This proves one consistent acquisition order with
no inversion and no cross-entry cycle.

### Lock inventory

| # | Lock | Type | Scope | Guards |
|---|------|------|-------|--------|
| **L0** | `Agent.inner` | `tokio RwLock<AgentInner>` | process | the `workspaces` map + `default_workspace_root` — the *registry-map lock* |
| **L1** | per-`Workspace` (Phase 4 step 2) | `tokio RwLock<Workspace>` (the map's values) | one workspace root | that workspace's projects + focused pointer |
| **L2** | `ActiveProject.write_lock` | `Arc<tokio Mutex<()>>` | one activated project | in-process write serialization |
| **L3** | `ActiveProject.file_lock` | `Arc<File>` flock | cross-process | `.codescout/write.lock` advisory lock |
| **Lx** | `dirty_files`, `session_write_roots` | `Arc<std Mutex<_>>` | one project | leaf — short, self-contained critical sections |

LSP-manager internal locks (`clients`, `last_used`, …) are a disjoint subsystem,
reached only after L0–L3 are released; their own internal order never interleaves
with the registry/write locks.

### The existing order (already shipped — `src/agent/write_guard.rs:47-104`)

`write_guard::acquire` defines **L2 → L3**, and crucially both are taken *after
the registry lock is released*:

- A write tool reads `active_project()` under **L0** (read), clones the
  `write_lock` and `file_lock` `Arc`s, **drops the L0 guard**, then calls
  `acquire`.
- `acquire` locks the in-process async mutex via `lock_owned()` (**L2**) — an
  *owned* guard that borrows nothing from the registry — then polls the flock on
  a `spawn_blocking` thread (**L3**).

So today the only registry-vs-write states are `{L0 alone, briefly}` and
`{L2, then L2+L3}`, and they never overlap. No cycle is possible because L0 is
dropped before L2 is ever awaited.

### The Phase 4 order (total — acquire top→bottom, release in reverse)

1. **L0** (registry map) — held only for *non-blocking* ops: `HashMap`
   get/insert/remove, `Arc::clone`, and at most `write_lock.try_lock()`
   (instantaneous).
2. **L1** (per-`Workspace`) — held to navigate to the focused `ActiveProject`
   and clone out its L2/L3/leaf `Arc`s. Also non-blocking while held.
3. — **check-then-act boundary: L0 and L1 are RELEASED here** —
4. **L2** (`write_lock`, owned) — awaited only now, with no registry lock held.
5. **L3** (flock) — after L2, per `acquire`.
6. **Lx** (leaf locks) — acquired and fully released within a step; never held
   while reaching for L0–L3.

### Deadlock-freedom invariant (THE GATE — enforce in review + test)

> **No thread ever blocks or `.await`s on L2 / L3 / Lx while holding L0 or L1.**
> Everything done under L0/L1 is non-blocking (map ops, `Arc::clone`,
> `try_lock`). Everything blocking (L2 `lock_owned().await`, the L3 flock poll)
> happens only *after* L0/L1 are dropped, reached via cloned `Arc`s.

Given the invariant:

- **No 1↔2 inversion:** you need L0 to find the `Arc<RwLock<Workspace>>`, so L0
  always precedes L1; the reverse never occurs.
- **No cross-entry cycle:** L0/L1 are *leaf-during-hold* — nothing blocking is
  awaited under them — so a registry lock can never be the middle of a wait
  cycle. A thread holding root A's L2 cannot be blocked on L0 held by a thread
  blocked on A's L2, because no one *holds* L0 while waiting on any L2.
- **Disjoint roots never contend:** different-root calls touch different L1 and
  different L2/L3 `Arc`s — full parallelism (the point of step 2).
- **Same-root writers serialize correctly:** `build_workspace`'s lock-reuse scan
  (already shipped) makes re-activation reuse the *same* `write_lock` /
  `file_lock` / `dirty_files` `Arc`s, so two writers to one root share L2.

### Eviction (step 3) — must mirror check-then-act

`evict_idle` (`src/lsp/manager.rs:932-962`) and the `get_or_start` LRU selector
(~`326-344`) are the templates: snapshot victims under the pool lock, **release**,
then act per-key with brief re-acquisition; the expensive `shutdown().await` runs
holding no pool lock.

The workspace eviction sweep does the same, and its quiescence probe is the one
place L0 meets L2 — so it MUST be non-blocking:

- Hold **L0 (write)** for the sweep so no new lookup can resurrect an entry
  mid-removal (lookups need L0-read, excluded by L0-write).
- Quiescence = for every activated project in the workspace,
  `write_lock.try_lock()` is **free** (NOT `.lock().await`) AND `dirty_files` is
  empty. `try_lock()` is instantaneous → safe under L0.
- A writer mid-write holds L2 (registry already released) → `try_lock` fails →
  not quiescent → skip. A writer about to start is blocked on L0-read until the
  sweep finishes; if its entry was evicted it re-resolves via `ensure_resident`.
- `default_workspace_root` is **exempt** (never evicted).

### What the proof obligates the code to guarantee (→ Phase 4 step 1 tests)

1. **Structural (review-enforced):** no `write_lock.lock()`, flock poll, or
   `.await` on a per-project lock appears inside a scope still holding an L0/L1
   guard. The pinned write accessors (`with_project_at_mut` /
   `require_write_root_for` twins) must hand back cloned `Arc`s, never a guard
   that borrows the registry.
2. **Runtime proxy for the no-hang claim:** a stress test — M concurrent pinned
   writes across K distinct roots (+ repeats on shared roots) — all complete
   under a bounded `tokio::time::timeout`. A hang ⇒ the invariant was violated.
   (Direct deadlock assertions hang rather than fail; the bounded-timeout stress
   test is the practical regression. `loom` is overkill given the small fixed
   lock set + the structural guarantee.)
3. **Concurrency correctness:** two different-root pinned writes do NOT serialize
   (overlap observable); two same-root pinned writes DO serialize (no interleave,
   no corrupt file).

## Phase 4b — DEFERRED (resume kit, 2026-05-31)

**Decision:** DEFERRED by explicit call (user, 2026-05-31), low probability of
being needed. Regime-3 correctness is fully fixed + **live-verified** without
4b; 4b is throughput-only for a narrow case. Preserved here so the scoping work
is not lost. **This plan stays active (do NOT archive) while 4b is open**, even
after Phase 4a+5 ship to master. At ship time, optionally promote this section
to a standalone `docs/trackers/` entry if the plan would otherwise be archived.

**What it does:** change `AgentInner.workspaces` from
`HashMap<PathBuf, Workspace>` → `HashMap<PathBuf, Arc<tokio::sync::RwLock<Workspace>>>`
so calls on *different* workspace roots stop serializing on the single
`AgentInner` lock. Today `with_project_at_mut` takes `inner.write()` (exclusive),
so all writes serialize globally; per-`Workspace` locking parallelizes
different-root writes. Reads already don't serialize (shared read lock);
same-root writes serialize on `write_lock` regardless — so the win is **only**
concurrent cross-workspace *writes*, which are rare.

**Gate (mandatory):** `## Phase 4 — Lock-Ordering Proof` above (commit `69c91896`).
Order registry-map L0 → per-`Workspace` L1 → `write_lock` L2 → flock L3; invariant
= never block/await on a per-entry lock while holding a registry lock; eviction
quiescence uses `try_lock()`, never `.lock().await`.

**Scouted blast radius (compiler-as-scout, 2026-05-31, then reverted):** flipping
the field type yields **12 first-layer `cargo check` errors** — the sync-accessor
bodies (`default_workspace`/`default_workspace_mut`), the `build_workspace` insert
sites (~478/516/574), the lock-reuse scan's `ws.projects` (~170), the
`with_project_at`/`with_project_at_mut` closures, and the `match` arms in
`call_edges_project_id_for` + `memory::resolve_memory_dir`. These **cascade to
~60 sites across 11 files** once the sync accessors change *signature*:
`default_workspace() -> Option<&Workspace>` and `active_project() ->
Option<&ActiveProject>` can no longer return references from behind `Arc<RwLock>`,
rippling to their ~50 callers (mostly `agent/mod.rs`). It is **atomic** — the
tree won't compile until all ~60 sites are migrated, so there is no safe mid-way
checkpoint; do it in one focused session.

**Re-scout recipe (reversible):**
```bash
# 1. flip the type in src/agent/mod.rs:
#    pub workspaces: HashMap<PathBuf, Arc<tokio::sync::RwLock<Workspace>>>,
# 2. enumerate the cascade (fix accessors → next layer surfaces):
cargo check
# 3. bail cleanly if not proceeding:
git checkout -- src/agent/mod.rs
```

**Accessor migration shape (per the proof):** the closure accessors clone the
`Arc<RwLock<Workspace>>` under a brief registry-map read lock, **release the map
lock**, then `.read()/.write().await` the per-`Workspace` lock and run the
closure. The sync accessors (`default_workspace`/`active_project`) must be
**removed** — callers move to the closure form (they cannot return refs from
behind the Arc).

**Eviction sweep (the other half of 4b):** see `## Lifetime contract` — idle-TTL
reusing the LSP pool's `idle_timeout_secs`; quiescence = every activated project
has `write_lock.try_lock()` free AND `dirty_files` empty; `default_workspace_root`
exempt. Without it, pinned-but-non-default workspaces stay resident until restart
(observed live 2026-05-31 with `mirela`). Currently harmless — the registry
entries are lightweight and the LSP clients self-evict on their own TTL — but it
grows unbounded over a long session that pins many distinct workspaces.

**Re-open trigger:** profiling shows different-root write serialization is a real
bottleneck (heavy concurrent multi-workspace subagent writes), OR resident-entry
memory growth from many pins becomes material. Until then: leave deferred.
