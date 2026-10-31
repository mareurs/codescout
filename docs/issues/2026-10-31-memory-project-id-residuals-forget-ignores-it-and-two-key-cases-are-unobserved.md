---
id: c351f4a6969588c7
kind: bug
status: open
title: 'BUG: memory forget ignores project_id, and two project_id key cases of the write fix are unobserved'
owners:
- marius
tags:
- cluster/declared-not-wired
opened: 2026-10-31
related:
- docs/issues/archive/2026-10-29-memory-write-with-a-project-id-cross-embeds-under-the-focused-project-not-the-named-one.md
severity: low
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
None.

## Tests added
None.

## Workarounds
Focus the member, then call without `project_id`.

## Resume
1. Write a red test for each of the three cases on the in-memory store seam, and see each fail or pass on the current code.
2. Fix only the cases that fail.

## References
- Fix `4c32d4341`.
