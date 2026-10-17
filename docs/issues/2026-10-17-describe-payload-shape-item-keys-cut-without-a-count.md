---
id: ad9522895036c10c
kind: bug
status: fixed
title: describe_payload_shape cuts an array's `item keys:` line at 24 keys and prints no count
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

`format::describe_payload_shape`'s array branch renders `item keys:` from the first element with `.take(MAX_ITEM_KEYS)` (24) and appends nothing when the cut bites. The object branch's key line, by contrast, ends `… +N more`. Found while reviewing the fix for `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`, whose docstring had claimed every bound discloses itself. The cap used to share one constant and one `PROBE_ROWS` annotation (`format.shape_keys`) with the object key line; it now has its own, `format.shape_item_keys`, classified `NoMarker`.

## Class fit

A capped result presented as complete, on a small surface: `item keys:` reads as "the element's fields" when it is "the first 24". It differs from the load-bearing members in that the line exists to aim a `[*]` projection, so a missing key is discovered by a failed `json_path` rather than by a wrong answer, and from the sibling scalar-line defect in that nothing at all names the omitted keys anywhere in the output.

## Symptom (Effect)

An array whose first item has 30 keys prints `array of 1 items` then `item keys: k00, k01, … k23` and nothing about `k24`..`k29`. Verified by running it on 2026-10-17 (a throwaway test printing the shape): the output ended at `k23` with no count.

## Reproduction

Reproduced at HEAD `64e71347` by running it before the fix: the shape ended at `k23` with no count.

`describe_payload_shape(&json!([{k00..k29: n}]))`. Code: the `Value::Array(items)` arm of `describe_payload_shape` in `src/tools/format.rs`, `first.keys().map(String::as_str).take(MAX_ITEM_KEYS).collect()` followed by an unconditional `out.push_str`.

## Root cause

The array arm was written without the disclosure the object arm has; `take` discards the surplus uncounted.

## Fix

Landed 2026-10-17 on the worktree branch `worktree-agent-ae50bad7b60f28077` (fix SHA recorded in the commit; not yet on `experiments`).

The array branch's `item keys:` line now ends `, … +N more entries` (`entry` at one) when the first item has more than `MAX_ITEM_KEYS` (24) keys, via the shared `more_entries` helper so it reads like the scalar line's and a nested group's marker. Silent at exactly 24 keys. `format.shape_item_keys` moved from `NoMarker` to `Probed` (`Marker::TextContains("more entries")`, `Mutation::Killed`).

## Tests added

In `src/tools/format.rs`, driven red before the fix (the line printed `item keys: k00 … k23` and stopped):

- `describe_payload_shape_counts_item_keys_past_the_cap` (cited by `format.shape_item_keys`): thirty keys, asserts the marker on the `item keys:` line itself (`, … +6 more entries`, `k23` shown, `k24` not) and that exactly 24 keys stay silent.

Mutation, observed red: disabling the marker emission (`if false && first.len() > keys.len()`) fails that test.

## Resume

Nothing outstanding for the defect. Verified by targeted tests only; the full gate was not run. Archive once the gate is green on `experiments`, recording the fix SHA and its patch-id.

Unrelated to this cap: `format.shape_keys` (the OBJECT key line's own `… +N more`) is still `Untested`; a test seeding an object with more than 24 keys would move that row to `Probed`. That note previously sat under the array cap and was wrong about which row it moves.

## References

- `src/tools/format.rs`, `describe_payload_shape`, array arm
- `src/tools/core/cap_probe.rs` row `format.shape_item_keys`
- `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
