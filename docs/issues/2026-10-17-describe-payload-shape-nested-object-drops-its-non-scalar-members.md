---
id: c66b40d7c32404b1
kind: bug
status: fixed
title: describe_payload_shape's one-level descent names nothing for a nested object's arrays, objects and nulls
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
closed: 2026-10-17
opened: 2026-10-17
severity: low
unverified: full gate not run; verified by targeted tests only; fix SHA and patch-id not yet recorded
---

## Summary

When `describe_payload_shape` descends into an object-valued key (`flat_scalars`), it renders that object's string/number/bool members and discards every other member (array, nested object, null) without naming or counting it. At the top level those members are at least named in the `keys:` line and, for arrays, sized in `arrays:`; inside the descended object nothing names them. This predates the 2026-10-17 fix for `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`, which counts over-length and surplus entries but deliberately not these.

## Class fit

A result narrowed by its shape presented as the object's contents: `hints: {a=1}` reads as the whole of `hints` when `hints` also carried an array, an object and a null. It differs from the scalar-line members in that the discard is by KIND, not by a size bound, so no constant is involved and no `cap-class` annotation or `PROBE_ROWS` row can carry it.

## Symptom (Effect)

`{"hints": {"a": 1, "list": [1,2,3], "deep": {"x": 1}, "n": null}}` prints `1 keys: hints` and `hints: {a=1}`; `list`, `deep` and `n` appear nowhere. Verified by running it on 2026-10-17 (a throwaway test printing the shape).

## Reproduction

Reproduced at HEAD `64e71347` by running it before the fix: it printed `hints: {a=1}`.

`describe_payload_shape(&json!({"hints": {"a":1,"list":[1,2,3],"deep":{"x":1},"n":null}}))`. Code: `flat_scalars` and the `ScalarCell::NotScalar` arm in `src/tools/format.rs`.

## Root cause

The descent is scalars-only by design (the narrow fix for `docs/issues/archive/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md`), and `ScalarCell::NotScalar` is the discard. Its doc comment's old claim that the key line and `arrays:` cover it holds only at the top level.

## Fix

Landed 2026-10-17 on the worktree branch `worktree-agent-ae50bad7b60f28077` (fix SHA recorded in the commit; not yet on `experiments`).

Inside the one-level descent (`flat_scalars`) an array, object or null member is now an entry held back and is counted, in the same marker as the group's other drops: `hints: {a=1, … +3 more entries}`. Chosen: COUNT, not names, in the shared unit (`entry`/`entries`, a cut group is one entry), so the group's single marker sums over-length strings, entries past `MAX_SCALARS`, and non-scalar members. A nested object holding only non-scalars now shows `meta: {… +2 more entries}` instead of vanishing. Unchanged: the top level stays silent about non-scalars (`keys:` and `arrays:` name them there), and a group that drops nothing prints as before. Naming the members instead was not chosen because it would add a new unbounded list to a line that is bounded by design; if names are wanted that is a follow-up.

## Tests added

In `src/tools/format.rs`, driven red before the fix (`hints: {a=1}`):

- `describe_payload_shape_counts_a_nested_objects_non_scalar_members`: array + object + null beside a scalar reads `hints: {a=1, … +3 more entries}`; a group of only non-scalars reads `meta: {… +2 more entries}`; cut + over-length + array sum to 3.
- the existing `..._is_silent_when_no_scalar_was_dropped` still guards the top level and an all-scalar group.

Mutations, each observed red: not counting `NotScalar` in `flat_scalars` (1 red, the new test); dropping `|| nested_dropped > 0` from the group's push condition (2 reds: the new test's all-non-scalar case and `..._inside_a_nested_object`'s all-over-length case).

## Resume

Nothing outstanding. Verified by targeted tests only; the full gate was not run. Archive once the gate is green on `experiments`, recording the fix SHA and its patch-id.

## References

- `src/tools/format.rs`, `describe_payload_shape/flat_scalars`
- `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
