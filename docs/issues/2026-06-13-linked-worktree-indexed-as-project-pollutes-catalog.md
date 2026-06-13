---
status: open
opened: 2026-06-13
closed:
severity: medium
owner: marius
related: []
tags: [librarian, catalog, worktree, indexing]
kind: bug
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
Plan (not implemented). Detect a linked worktree before indexing and skip it
(index only the main worktree). Detection: a linked worktree's `.git` is a
*file* (`gitdir: …/.git/worktrees/<name>`), or equivalently
`git rev-parse --git-dir` != `git rev-parse --git-common-dir`. On a linked
worktree, either refuse to index (return a clear message) or redirect indexing
to the main worktree root. Principle: the catalog tracks one canonical
(main-worktree) copy; worktree checkouts are duplicate + stale-on-merge.

## Tests added
None yet. When fixed: a test that creating + activating a linked worktree as a
project does not add `…/.worktrees/…` rows (indexing is a no-op / refused), and
that main-tree indexing is unaffected.

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
