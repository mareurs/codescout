---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- catalog
- worktree
- indexing
topic: null
time_scope: null
closed: 2026-06-14
---

# BUG: Activating a linked git worktree as a project indexes its files into the global catalog (duplicate + stale-on-merge); the librarian has no worktree awareness

## Summary
The librarian should only index the **main worktree** of a repo. But it has no
worktree awareness (`grep worktree src/librarian` → 0 matches). When a *linked*
git worktree is activated as a project, the indexer walks the worktree as its
own root and indexes every file under the worktree's `abs_path`. Those rows (a)
**duplicate** the main-tree content and (b) go **stale the moment the
worktree's branch merges** — yet they sit in the shared global catalog until a
*main-tree* reindex happens to prune them via `.gitignore`.

## Symptom (Effect)
The global catalog held **659** rows under
`/home/marius/work/claude/codescout/.worktrees/vdi-windows/…` — full duplicates
of the main-tree docs, plus 6 events on those duplicate paths. They were pruned
only as a side effect of a later `reindex(scope="project")` of the main tree.

## Reproduction
1. `git worktree add .worktrees/<name> <branch>` inside a repo whose
   `.gitignore` contains `/.worktrees/`.
2. Activate that worktree as a codescout project (project root = the worktree
   path) and let it index (e.g. via worktree activation flow).
3. Catalog gains rows under `…/.worktrees/<name>/…` — the root-anchored
   `/.worktrees/` gitignore rule does not match the worktree's own files
   relative to the worktree root, so nothing excludes them.
4. A later `reindex(scope="project")` of the **main** tree removes them
   (gitignore excludes `.worktrees/` relative to the main root).

## Environment
codescout MCP server, 2026-06-13. Global catalog
`~/.local/share/librarian/catalog.db`. `git worktree list` shows
`.worktrees/vdi-windows  eba48915 [vdi-windows]` as a linked worktree.

## Root cause
`src/librarian/indexer.rs:68` —
`WalkBuilder::new(abs_root).standard_filters(true).build()` honors `.gitignore`
**relative to `abs_root`**. The repo's `.gitignore` line `/.worktrees/` is
root-anchored, so:
- main tree as root → `.worktrees/` is excluded (correct);
- worktree as root → the worktree's own files are not under a `.worktrees/`
  child of themselves, so nothing excludes them → they are indexed.

There is no check anywhere in `src/librarian` for "is this root a linked
(non-main) worktree?" before indexing — the librarian treats a worktree path
like any other project root.

## Evidence
- `grep -nE "worktree" src/librarian` → 0 matches (no worktree handling).
- `git worktree list` (this session): `.worktrees/vdi-windows` is a registered
  linked worktree (`[vdi-windows]`), plus several stale ones under the dead
  `code-explorer/.worktrees/` (prunable) and `.claude/worktrees/peer-delegation`.
- 659 catalog rows existed under `…/.worktrees/vdi-windows/…`; 0 augmentations,
  6 events attached; all pruned by a main-tree reindex this session with no
  main-tree loss.

## Hypotheses tried
N/A — mechanism read from `indexer.rs` + `.gitignore` + `git worktree list`.

## Fix

**Shipped on `experiments` in `9d84f347`** (`fix(indexer): skip indexing linked git worktrees into the catalog`). Not yet on `master` — archive after cherry-pick, cite the master-side SHA then.

`src/librarian/current_project.rs::is_linked_worktree(root)` — filesystem-only detection (no `git` subprocess): a linked worktree's `.git` is a *file* whose `gitdir:` points through a `worktrees/` path component. A submodule's `.git` file points through `modules/`, so submodule roots are **not** skipped. `src/librarian/indexer.rs::index_repo_sync` guards on it at the top and returns an empty report (no walk, no rows) for a linked-worktree root — the single chokepoint all index paths (MCP reindex, CLI reindex, activate-time indexing) funnel through.

The guard stops NEW pollution; existing worktree rows are cleaned by a main-tree `reindex(scope=project)` (gitignore excludes `.worktrees/` relative to the main root) or the new `doctor(fix=prune_missing, root=<worktree-path>)` ([[2026-06-13-catalog-orphans-survive-repo-rename]]).
## Tests added

- `src/librarian/current_project.rs::is_linked_worktree_detects_worktree_not_submodule_or_main` — true for a `.git`-file worktree; false for a submodule (`modules/`), a main checkout (`.git` dir), and a non-git dir.
- `src/librarian/indexer.rs::index_repo_sync_skips_linked_worktree` — a worktree fixture containing a `.md` yields `report.added == 0` and zero artifact rows.

Full lib suite 2739 pass; clippy `-D warnings` clean.
## Workarounds
Do not activate/index a linked worktree as a project. A `reindex(scope="project")`
of the main tree prunes any worktree rows that slipped in (gitignore excludes
`/.worktrees/` relative to the main root). Cleaned this way on 2026-06-13.

## Resume
Add a linked-worktree guard in the indexing entry path (`src/librarian/indexer.rs`
and/or the project-activation path that triggers indexing): compute
`git rev-parse --git-dir` vs `--git-common-dir`; if they differ, skip indexing
that root. Then add the regression test.

## References
- `src/librarian/indexer.rs:68` — `WalkBuilder … standard_filters(true)` (gitignore-relative-to-root).
- `.gitignore:2` — `/.worktrees/` (root-anchored, why main-tree excludes but worktree-root does not).
- Sibling catalog-hygiene bugs:
  [[2026-06-13-delete-orphan-repos-cross-workspace-wipe]],
  [[2026-06-13-catalog-orphans-survive-repo-rename]].
- Discovered 2026-06-13: a main-tree `reindex(scope="project")` pruned 659
  `.worktrees/vdi-windows` rows; user confirmed worktrees should never be
  indexed (duplicate content; stale on branch merge).
