---
id: d372d3a36336b251
kind: bug
status: open
title: worktree_main_root maps a submodule worktree to the superproject and a relative pointer to a relative path
owners:
- marius
tags:
- cluster/unclassified
- worktrees
opened: 2026-11-13
severity: low
unverified: 'The wrong main_root for CurrentProject::resolve was inferred by two reviewers from the helper bodies, not shown by a test on that function. Not established: whether any real session uses a submodule worktree or relative worktree paths.'
---

# BUG: worktree_main_root maps a submodule worktree to the superproject and a relative pointer to a relative path

## Summary

`util::path_security::worktree_main_root` cuts a `gitdir:` pointer at the first `.git` component and returns that prefix. It has no `worktrees` check and does not resolve relative pointers. `CurrentProject::resolve` pairs it with `is_linked_worktree`, which only requires a `worktrees` component. Two layouts then give a wrong `main_root`.

## Class fit

cluster/unclassified: two helpers whose combined contract is narrower than either one's doc.

## Symptom (Effect)

- A linked worktree of a SUBMODULE has the pointer `gitdir: <super>/.git/modules/<sub>/worktrees/<name>`. `is_linked_worktree` is true; `worktree_main_root` returns `<super>`. `CurrentProject.main_root` then names the superproject, which also feeds umbrella lookup (`current_project.rs` ~44-46).
- A worktree created with `worktree.useRelativePaths` has a relative pointer. `worktree_main_root` returns a relative path such as `..` or `../..`, which is not a root.
- The helper's doc (`path_security.rs` ~1219-1225) says it answers only for linked worktrees, but it answers for any `gitdir:` pointer.

## Reproduction

Build a repository with a submodule and run `git worktree add` inside the submodule; or `git config worktree.useRelativePaths true` and add a worktree; then call `CurrentProject::resolve` on the worktree. Plan C1 tests (`cite_text.rs`) build the pointer files directly and show both mappings through `repository_root` before its own gate.

## Environment

Any layout with a submodule worktree or relative worktree paths. Rare.

## Root cause

`worktree_main_root` splits on the first `.git` component and never checks that the git dir sits directly in `<main>/.git/worktrees/`. Plan C1 Task 3 works around it inside `cite_text::repository_root` (parent equality, an absolute main root, a `.git` directory) and left the shared pair unchanged because `CurrentProject::resolve` uses it.

## Evidence

Task 3 fix rounds 2 and 3 of Plan C1 (commits 3555f6904 and c8d930394) and their re-reviews; the tests `a_worktree_of_a_submodule_is_not_the_superprojects_checkout` and the relative-pointer tests in `src/librarian/catalog/cite_text.rs`.

## Hypotheses tried

None.

## Fix

Not made. Direction: make `worktree_main_root` require that the pointed git dir's parent is `<main>/.git/worktrees`, return only an absolute path, and fix its doc; then drop the duplicate gate in `cite_text::repository_root`. Check `CurrentProject::resolve` and the umbrella lookup against both layouts.

## Tests added

None for the shared helper; the cite_text tests cover the workaround.

## Workarounds

`cite_text::repository_root` guards its own use.

## Resume

Plan E1 (layout C: `<git root>/.codescout/derived.db`, E1-R10) does not need a correct main root: it takes the nearest directory holding `.git` (`lookup_git_root`), so a linked worktree or a submodule has its own root and never calls `worktree_main_root`. The bug stays open for `CurrentProject::resolve` and any caller that still wants the main checkout root; fix it with whichever Plan E step first needs that root (none planned yet).

## References

- docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md
