# Session Log — Concurrency Fixes (multi-instance / multi-worktree)

> Work stream: the codescout concurrency defects found 2026-05-30 while probing
> multi-instance / multi-worktree usage on backend-kotlin. Two bug files own the
> defect lifecycle:
> - `docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md` (regime 2 — FIXED)
> - `docs/issues/2026-05-30-shared-server-global-active-project-race.md` (regime 3 — open, design fork)
>
> This log captures frictions (F-N) and wins (W-N) across the sessions that work
> the fixes. Append via `edit_markdown(action="insert_before", heading="##
> Template for new entries", ...)` and add an Index / Wins Index row. Full
> conventions + entry templates: `docs/templates/session-log.md`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-30 | high | architectural | fixed-verified | Plan targets `ActiveProject` granularity; reality is a `Workspace` registry that already exists |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-30 | med | scout-existing-sync-helper-before-subprocess | would have spawned `git rev-parse` on the LSP-start hot path | validated |
| W-2 | 2026-05-30 | high | correctness-vs-performance phase split | R1 plan would have landed deadlock-prone locking before any test could exercise it | validated |

---

## W-1 — `detect_worktree_info().main_repo` is the per-repo Gradle key (no subprocess needed)

**Observed:** 2026-05-30, implementing the regime-2 fix (kotlin cross-worktree `--system-path` / `GRADLE_USER_HOME` isolation).

**Pattern:** Before writing the fix, scouted `detect_worktree_info` (`src/prompts/mod.rs:194`) and confirmed it is filesystem-only (no `git` subprocess) and returns `main_repo: Option<PathBuf>` — the shared main-checkout path across all worktrees of a repo. That is exactly the identity needed to key `GRADLE_USER_HOME` per-repo, and `default_config` is sync, so it composes directly with no async/subprocess plumbing.

**Counterfactual:** Without the scout, the natural implementation would have reached for a `git rev-parse --git-common-dir` subprocess inside `default_config` — a sync, hot, per-LSP-start config builder. That adds process-spawn latency to every LSP start, a new failure mode (git missing / not a repo), and async/sync friction (subprocess in a sync fn). The scout replaced all of that with one existing sync fn call. Cost avoided: 1 subprocess per LSP start + a fallback/error branch + its test.

**Confirming data points:**
1. `detect_worktree_info` doc comment explicitly states "Filesystem-only — no `git` subprocess" (`src/prompts/mod.rs:191-193`).
2. `workspace_hash` (`src/lsp/mux/mod.rs:14`) is `DefaultHasher` fixed-seed → deterministic across processes, so two codescout processes on one worktree compute the same suffix and correctly share — pinned by `kotlin_system_path_is_stable_for_same_workspace`.

**Impact:** med — kept a subprocess off the LSP-start hot path and avoided an extra error/fallback branch.

**Promote-when:** A second fix needs repo-vs-worktree identity and reaches for `detect_worktree_info` instead of a git subprocess. At 2 datapoints, note in CLAUDE.md that `detect_worktree_info` is the canonical sync repo-identity resolver.

**Status:** validated

---

## F-1 — Plan targets `ActiveProject` granularity; reality is a `Workspace` registry that already exists

**Observed:** 2026-05-30, pre-implementation review of `docs/plans/2026-05-30-per-request-workspace-pinning.md` (the regime-3 root-cause plan), before writing any Phase-0/1 code.

**When:** Reading the plan's Design + Lifetime-contract sections, about to start the call-site inventory.

**Expected (plan):** `AgentInner` holds a *single focused* `ActiveProject` slot; ~100 sites read it; the fix introduces `projects: HashMap<PathBuf, Arc<RwLock<ActiveProject>>>` + a `default_root`, and builds a per-request resolver ("explicit > default > error").

**Got (scouted reality):**
- `AgentInner.workspace: Option<Workspace>` (`src/agent/mod.rs:83`) — the racing slot is a whole `Workspace`, not a bare `ActiveProject`.
- `Workspace` (`src/workspace.rs:316`) is **already** a multi-project registry: `projects: Vec<Project>`, each `Dormant` or `Activated(Box<ActiveProject>)`, plus `focused: Option<String>`.
- `Workspace::resolve_root(project, file_hint)` (`src/workspace.rs:373`) **already** resolves "explicit id > file hint > focused" — the exact per-request order the plan proposes to build from scratch.
- `Agent::activate` (`src/agent/mod.rs:330`) does `inner.workspace = Some(ws)` — it **replaces the whole slot**. A `Workspace`'s `projects` are all sub-projects of one `root`; it structurally cannot hold worktree A and worktree B at once. So the registry unit must be `Workspace`, not `ActiveProject`.

**Probable cause:** Plan written from the regime-3 bug file's "single global active project" framing — accurate at the *symptom* layer (`active_project()` resolves one focused project) but not the *structural* layer (a `Workspace` nests N projects; only one `Workspace` is live). The plan lists `focused_project_root()` as an accessor, but its Design data structure (flat `HashMap<_, ActiveProject>`) shows the `Workspace` nesting was never modeled.

**Severity:** high — implementing Phase 1 verbatim builds a flat `ActiveProject` HashMap that duplicates and collides with the existing `Workspace` abstraction; the collision surfaces only after the structure is wired, forcing a full Phase-1 rewrite. Caught pre-implementation, the correction is a plan revision (registry over `Workspace`s; reuse `resolve_root`) — net *less* code.

**Workaround / Fix:** Revise the plan's Design + Lifetime-contract + Phases to `Workspace`-registry granularity before any code: (a) registry = `HashMap<PathBuf, Workspace>` (Arc/RwLock as needed) replacing `inner.workspace`; (b) `default_root` = default *workspace* root (focused-project-within-workspace stays via existing `focused`); (c) per-request resolution = registry lookup → existing `Workspace::resolve_root`; (d) lifetime/quiescence predicate must iterate ALL `Activated` projects in a workspace (each owns its own `write_lock`/`file_lock`/`dirty_files`), not check one `ActiveProject`.

**Status:** fixed-verified — plan revised to `Workspace`-registry granularity (R2) 2026-05-30 after architecture-snow-lion review; revised Design / Lifetime-contract / Phases read coherently against the scouted code (`src/workspace.rs:316,373`, `src/agent/mod.rs:83,330`).

**Fix idea / Pointer:** Plan Design + Lifetime-contract sections; this session.

---
## W-2 — Correctness-vs-performance phase split kept the deadlock risk out of the bug fix

**Observed:** 2026-05-30, implementing the per-request-workspace-pinning plan (Phases 1–3) on branch `feat/per-request-workspace-pinning`.

**Pattern:** architecture-snow-lion recognized that regime-3 is fixed by per-request resolution + multi-residence **alone** — per-`Workspace` locking is a separate *performance* concern, not a correctness requirement. Split the work: Phase 3 (correctness) ships under the **existing** single `AgentInner` `RwLock`; Phase 4 (per-entry `Arc<RwLock>` + eviction) lands later behind the lock-ordering gate. The whole read surface (13 tools) shipped + proven without touching the lock model.

**Counterfactual:** The R1 plan bundled per-`Workspace` locking into Phase 1. Following it would have landed a deadlock-prone lock-reordering **before any test could exercise it** (with one resident entry, per-entry locks never contend), coupling the bug fix to the riskiest part of the design. The split let the bug close on a proven lock model and quarantined the deadlock risk in Phase 4 with its own gate — every Phase-1–3 commit stayed reversible.

**Confirming data points:**
1. Phase 3 closed regime-3 for all reads (`read_file_concurrent_pins_no_cross_workspace_bleed` — 5 tasks, shared Agent, multi-thread, zero bleed) under the **unchanged** single `RwLock` — proving locking wasn't needed for correctness.
2. Pending: Phase 4 validates the per-entry lock model behind the lock-ordering gate.

**Impact:** high — separated a correctness fix from a deadlock-risk optimization; shipped the fix reversibly, per commit.

**Promote-when:** A second concurrency fix separates correctness (resolution) from performance (locking) and ships correctness first. At 2 datapoints, note in CLAUDE.md: "for concurrency fixes, land correctness under the existing lock model before changing lock granularity."

**Status:** validated — Phase 3 shipped + proven; Phase 4 pending.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## W-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.
     Entry templates + status vocabulary: docs/templates/session-log.md -->
