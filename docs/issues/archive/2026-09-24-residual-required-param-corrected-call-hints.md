---
id: 4e554afd1a80957e
kind: bug
status: fixed
title: 'RESIDUAL: Add with_hint corrected-call suggestions to the 8 required-param failure sites that name their action but offer no corrected call'
tags:
- cluster/unclassified
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add with_hint corrected-call suggestions to the 8 required-param failure sites that name their action but offer no corrected call.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` (status `fixed`):

> Out of scope and deliberately not done: the 8 already-adequate sites name their action but carry no `with_hint` corrected call, so they satisfy clause 2 only partly. The 9 repaired sites are live-verified (2026-08-28) — see § Tests.

## Fix

Five of the eight sites the parent listed now carry a corrected call. `update` (missing `id`) escaped as a bare serde `missing field` with no action; `move`, `delete`, `graft` and `merge_worktree` named their action but built the librarian's own error type with no hint. All five now go through the shared `deser_error` helper (`src/librarian/tools/mod.rs:377`, host `RecoverableError`) with a message naming their own action and a hint that carries an `e.g.` corrected call: `src/librarian/tools/update.rs`, `mv.rs`, `delete.rs`, `graft.rs` and `merge_worktree.rs`. The messages keep the `requires '` shape that `usage::db::normalize_err_family` files under `missing_required_param`. The other three sites needed no change: `state_at` and `augment` already carried a corrected call before this commit (checked at `f909e132^`), and `workspace_state_at` has no required field, because every field of its `Args` has a serde default, so a bare `?` there is not this defect. `state_at` was added to the table test, so six sites are now pinned; `augment` is not in the table test.

## Tests added

In `src/librarian/tools/mod.rs`, the table test `every_required_param_failure_names_its_action_and_routes` (:610) gains `update`, `move`, `delete`, `graft`, `merge_worktree` and `state_at`. For every row it asserts that the failure arrives as the host `RecoverableError` (not the librarian's same-named type, not a bare serde error), that the message contains `requires` and names `doc(` or `librarian(`, and that the hint carries `e.g.` or `doc(`. For these six rows it additionally asserts that the message names THEIR OWN action (`(action="<name>")`), because a tool prefix alone is satisfied by any action's text.

## Fix provenance

- **SHA:** `f909e132` (`experiments`)
- **patch-id:** `f447c6d9967ed567f7c9ed38492cff1dbbcc51a2`

## Resume

Closed on 2026-10-06 by `f909e132` (local on `experiments`, not pushed at the time of writing). Residual follow-ups, listed and not filed: (1) the "exactly one of commit/timestamp" refusals in `src/librarian/tools/workspace_state_at.rs` and `src/librarian/tools/state_at.rs` are plain `anyhow!` errors with no corrected call, a candidate for their own bug. (2) `augment` is not in the table test, so its already-present hint is unpinned by it. (3) The parent `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` still carries its `unverified: TRACKED 36a0d5c7cb3fd4db` caveat, which the integrator decides how to retire.

## References

- `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` — parent
