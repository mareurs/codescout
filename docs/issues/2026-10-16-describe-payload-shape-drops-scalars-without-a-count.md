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

`format::describe_payload_shape` renders a buffered payload's shape. Its key list discloses overflow (`… +N more`, `format.rs:178-182`), but its scalar line does not: a string longer than `MAX_SCALAR_LEN` (60) is filtered out and only the first `MAX_SCALARS` (8) scalars survive `.take(MAX_SCALARS)`, with nothing saying either happened. Filed from the `PROBE_ROWS` classification of `format.shape_scalar_len` and `format.shape_scalars` as `Coverage::NoMarker`.

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

`describe_payload_shape` now counts what its scalar pass removes and ends the scalar line `… +N more scalars`, the counterpart of the key line's `… +N more`. `N` sums the two removals: strings over `MAX_SCALAR_LEN` (filtered) and scalars past `MAX_SCALARS` (cut). The same count is carried by the one-level descent (`flat_scalars`), printed inside that object's braces (`hints: {…, … +3 more scalars}`); an object whose scalars were all over-length now shows `meta: {… +1 more scalars}` instead of vanishing. Silent when nothing was dropped, and fields that are not scalars (arrays, null, empty objects) are never counted, so the marker stays a signal (ADR `2026-08-27-negative-results-name-their-scope`, clause 2). An object-valued key holds one slot of the top-level cap, however many scalars it carries.

Both `PROBE_ROWS` rows moved from `NoMarker` to `Probed` (`Marker::TextContains("more scalars")`, `Mutation::Killed`).

## Tests added

In `src/tools/format.rs`, all driven red before the fix:

- `describe_payload_shape_counts_over_length_scalars_it_drops` (cited by `format.shape_scalar_len`; 60-byte string admitted, 61-byte dropped, only the length bound can bite)
- `describe_payload_shape_counts_scalars_past_the_cap` (cited by `format.shape_scalars`; eleven short numbers, only the count cap can bite)
- `describe_payload_shape_counts_both_scalar_bounds_together` (sum, +4)
- `describe_payload_shape_counts_what_it_drops_inside_a_nested_object` (nested site, and the all-over-length nested object)
- `describe_payload_shape_is_silent_when_no_scalar_was_dropped` (guards over-reporting; passes before and after by design)

Mutation sweep, one per guarded site, each reds the named tests: dropping the over-length term in `cap_scalars` (3 reds); dropping the cut term in `cap_scalars` (3 reds); zeroing the over-length count at the top-level call (2 reds); zeroing it at the `flat_scalars` call (1 red, the nested test).

## Resume

Nothing outstanding for the defect. Not done here: the full gate (`./scripts/gate.sh`) was not run, only `cargo test --lib describe_payload_shape` and `--test result_caps`; archive once the gate is green on `experiments`, recording the fix SHA and its patch-id. `format.shape_keys` (the key line's own `… +N more`) is still `Untested` in `PROBE_ROWS`; a test seeding more than 24 keys would move it to `Probed`.

## References

- `src/tools/format.rs:143-230`
- `src/tools/core/cap_probe.rs` rows `format.shape_scalar_len`, `format.shape_scalars`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
