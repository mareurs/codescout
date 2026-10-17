---
kind: bug
status: fixed
title: describe_payload_shape drops over-length and surplus scalars from its shape line with no "+N more"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
unverified: full gate not run; verified by targeted tests (describe_payload_shape, result_caps) only; fix SHA and patch-id not yet recorded
---

## Summary

`format::describe_payload_shape` renders a buffered payload's shape. Its key list discloses overflow (`… +N more`, in the `Value::Object` arm of `describe_payload_shape`), but its scalar line does not: a string longer than `MAX_SCALAR_LEN` (60) is filtered out and only the first `MAX_SCALARS` (8) scalars survive `.take(MAX_SCALARS)`, with nothing saying either happened. Filed from the `PROBE_ROWS` classification of `format.shape_scalar_len` and `format.shape_scalars` as `Coverage::NoMarker`.

## Class fit

A capped result presented as complete, on a small and low-stakes surface: the scalar line looks like "the object's scalar fields" when it is "the first eight short ones". It differs from the class's load-bearing members in that the key line directly above lists every key (up to its own marked cap of 24), so a reader can see that a key exists without a scalar beside it; the omission is recoverable by inspection, and no number is misreported.

## Symptom (Effect)

For an object with a 20,000-character string field and ten numeric fields, the shape line lists all keys, shows eight `k=v` scalars and does not include the long field or say two numerics were cut. (Inferred from the code below; the test `the_generic_fallback_describes_the_payload_instead_of_the_envelope` drives the 20,000-character case and asserts only that the raw value is never inlined.)

## Reproduction

`src/tools/format.rs`: `Value::String(s) if s.len() <= MAX_SCALAR_LEN => Some(...)` and `_ => None` at `:195-212`, then `.take(MAX_SCALARS).collect()` and an unconditional `out.push_str` of whatever survived. The same shape in the nested `flat_scalars` helper (`:160-170`). Compare the key branch, which appends `, … +{n} more`. Read at `experiments` f5f1c6f7 — inferred, not run.

## Root cause

The scalar pass is a `filter_map` + `take` with no counter of what either step removed; only the key pass was given a disclosure.

## Fix

Landed 2026-10-17 on the worktree branch `worktree-agent-ae50bad7b60f28077` (fix SHA recorded in the commit; not yet on `experiments`).

`describe_payload_shape` now counts what its scalar pass removes and ends the scalar line `… +N more entries` (`entry` at one), the counterpart of the key line's `… +N more`. The unit is the line's own ENTRY, not the scalar, because that is what is counted: `N` sums strings over `MAX_SCALAR_LEN` (filtered) and entries past `MAX_SCALARS` (cut), and an object-valued key cut by the cap is one entry however many scalars it held (the first revision labelled this `scalars`, which over-claimed; fixed on review). The same count is carried by the one-level descent (`flat_scalars`), printed inside that object's braces (`hints: {…, … +3 more entries}`); an object whose scalars were all over-length now shows `meta: {… +1 more entry}` instead of vanishing. Silent when nothing was dropped, and fields that are not scalars are never counted at the top level, where `keys:` and `arrays:` already name them (ADR `2026-08-27-negative-results-name-their-scope`, clause 2).

Both `PROBE_ROWS` rows moved from `NoMarker` to `Probed` (`Marker::TextContains("more entries")`, `Mutation::Killed`).

**Not every bound discloses itself, and the first revision's docstring said so wrongly.** Two silent drops were found by review, verified by running them, and filed; both are fixed on the same branch with the same `entry`/`entries` unit: an array's `item keys:` line cut at 24 with no count (`docs/issues/2026-10-17-describe-payload-shape-item-keys-cut-without-a-count.md`; its own constant and `PROBE_ROWS` row `format.shape_item_keys`, now `Probed`), and a nested object's arrays, objects and nulls named nowhere (`docs/issues/2026-10-17-describe-payload-shape-nested-object-drops-its-non-scalar-members.md`).

## Tests added

In `src/tools/format.rs`, all driven red before the fix:

- `describe_payload_shape_counts_over_length_scalars_it_drops` (cited by `format.shape_scalar_len`; 60-byte string admitted, 61-byte dropped, only the length bound can bite)
- `describe_payload_shape_counts_scalars_past_the_cap` (cited by `format.shape_scalars`; eleven short numbers, only the count cap can bite)
- `describe_payload_shape_counts_both_scalar_bounds_together` (sum, +4; the over-length strings come FIRST so a mutant that lets them occupy cap slots shows only six numbers)
- `describe_payload_shape_counts_what_it_drops_inside_a_nested_object` (nested site, and the all-over-length nested object)
- `describe_payload_shape_reports_a_line_whose_every_scalar_was_dropped` (the `|| dropped > 0` half of the top-level condition)
- `describe_payload_shape_counts_a_cut_nested_object_as_one_entry` (the unit: a cut group is one entry, not its three scalars)
- `describe_payload_shape_is_silent_when_no_scalar_was_dropped` (guards over-reporting; passes before and after by design)

Mutation sweep, one mutation in the tree at a time, each observed red: dropping the over-length term in `cap_scalars` (3 reds); dropping the cut term (3 reds); zeroing the over-length count at the top-level call (2 reds); zeroing it at the `flat_scalars` call (1 red, the nested test only, so the `format.shape_scalar_len` row's cited test does not cover that site); deleting `|| dropped > 0` (1 red, `..._whose_every_scalar_was_dropped`); letting over-length strings occupy cap slots (1 red, `..._both_scalar_bounds_together`); counting a cut group's inner scalars instead of one entry (1 red, `..._as_one_entry`).

## Resume

Nothing outstanding for the defect itself. Verified by targeted tests only; the full gate was not run. Archive once the gate is green on `experiments`, recording the fix SHA and its patch-id.

Related: the two follow-up issues are fixed on the same branch. `format.shape_keys` (the OBJECT key line's own `… +N more`) is still `Untested`; a test seeding an object with more than 24 keys would move that row to `Probed`. The array `item keys:` cap is a separate row (`format.shape_item_keys`) and is already `Probed`.

## References

- `src/tools/format.rs:143-230`
- `src/tools/core/cap_probe.rs` rows `format.shape_scalar_len`, `format.shape_scalars`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
