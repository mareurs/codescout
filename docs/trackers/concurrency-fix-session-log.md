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
| _(none yet)_ | | | | | |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-30 | med | scout-existing-sync-helper-before-subprocess | would have spawned `git rev-parse` on the LSP-start hot path | validated |

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

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## W-N — title\n...")
     Also update the matching Index / Wins Index table row at the top.
     Entry templates + status vocabulary: docs/templates/session-log.md -->
