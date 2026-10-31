---
id: c351f4a6969588c7
kind: bug
status: fixed
title: 'BUG: memory forget ignores project_id, and two project_id key cases of the write fix are unobserved'
owners:
- marius
tags:
- cluster/declared-not-wired
closed: 2026-10-31
opened: 2026-10-31
related:
- docs/issues/archive/2026-10-29-memory-write-with-a-project-id-cross-embeds-under-the-focused-project-not-the-named-one.md
severity: low
unverified: The gate format lane refused on ownership (rustfmt --check was clean); forget was not run end to end on a live sqlite-vec server. The fix, the regression test and the clippy, lean, default and Python lanes are verified.
---

# BUG: memory forget ignores project_id, and two project_id key cases of the write fix are unobserved

## Summary
Fix 4c32d4341 made `memory` write, delete, `remember` and `recall` file and read a semantic point under the project id the call names. Three cases were left unchecked or unchanged. This file holds them so the parent issue can close.

## Class fit
`project_id` is declared in the tool's schema, yet `forget` does not honor it (the `declared-not-wired` trait). The Qdrant delete that `forget` reaches ignores its `project_id` argument, as the parent issue recorded on 2026-10-30.

## Symptom (Effect)
- `forget` with `project_id=<member>`: the delete is not scoped to the named project. Observed on 2026-10-30 in the parent issue's runtime probe. Not re-read in source this session.
- A member that is dormant when a memory is filed: its key is its id. Activation later sets the same name. Not observed.
- The root project named by its workspace id while a member is focused: not observed.

## Reproduction
Not written yet. Each case needs a call with `project_id` set and the in-memory store seam (`multi_project_ctx_with_store`, `points_under` in `src/tools/memory/tests.rs`).

## Environment
Branch `experiments`, tree at `dd4737085`.

## Root cause
Not investigated for the two unobserved cases. For `forget`, see the parent issue.

## Evidence
Parent issue: `docs/issues/archive/2026-10-29-memory-write-with-a-project-id-cross-embeds-under-the-focused-project-not-the-named-one.md`, section Resume.

## Hypotheses tried
None.

## Fix

Done 2026-10-31 in 1644629e5 (branch `experiments`; patch-id fbfa65c744ba82abbdef59c362efe38868e2ea09). `forget` now takes its store key from `semantic_project_key`, as write, delete, remember and recall do.

- **SHA:** `1644629e5` (`experiments`)
- **patch-id:** `fbfa65c744ba82abbdef59c362efe38868e2ea09`

Only `forget` was broken. The other two cases passed on the existing code: a memory filed for a dormant member is found once the member is focused, and a `project_id` naming the root files under the root's own key. Qdrant was never affected, because it deletes by id.

## Tests added

- `forget_with_a_project_id_removes_the_named_projects_point` (`src/tools/memory/tests.rs`): red first, `left: 1, right: 0`.
- `a_memory_filed_for_a_dormant_member_is_found_once_the_member_is_focused` and `a_project_id_naming_the_root_files_under_the_roots_own_key`: green on first run. A mutation of each key branch in `semantic_key_for_project` turned both red; the mutation was reverted.
- `a_delete_under_another_projects_key_removes_nothing` (`src/memory/sqlite_semantic_store.rs`): a premise check on the real sqlite-vec store. It passes on existing code.
- The in-memory test store's `delete` now scopes by project like sqlite-vec, so a test can see a wrong key.

## Workarounds
Focus the member, then call without `project_id`.

## Resume

1. Re-run `./scripts/gate.sh` once the format lane can prove ownership. The run of 2026-10-31 passed clippy, LEAN, DEFAULT and the Python lane; the format lane refused (`FMT_STATE=ownership-refused`, coverage gaps in the history), and `rustfmt --check` on the changed files was clean after three hand edits.
2. Optionally run `forget` with `project_id` on a live server with the sqlite-vec backend. Not done: only the in-memory seam and the real store's delete were tested.
3. Then archive with `doc(action="move", ...)`.

## References
- Fix `4c32d4341`.
