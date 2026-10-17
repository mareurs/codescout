---
id: ad9522895036c10c
kind: bug
status: open
title: describe_payload_shape cuts an array's `item keys:` line at 24 keys and prints no count
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
---

## Summary

`format::describe_payload_shape`'s array branch renders `item keys:` from the first element with `.take(MAX_ITEM_KEYS)` (24) and appends nothing when the cut bites. The object branch's key line, by contrast, ends `… +N more`. Found while reviewing the fix for `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`, whose docstring had claimed every bound discloses itself. The cap used to share one constant and one `PROBE_ROWS` annotation (`format.shape_keys`) with the object key line; it now has its own, `format.shape_item_keys`, classified `NoMarker`.

## Class fit

A capped result presented as complete, on a small surface: `item keys:` reads as "the element's fields" when it is "the first 24". It differs from the load-bearing members in that the line exists to aim a `[*]` projection, so a missing key is discovered by a failed `json_path` rather than by a wrong answer, and from the sibling scalar-line defect in that nothing at all names the omitted keys anywhere in the output.

## Symptom (Effect)

An array whose first item has 30 keys prints `array of 1 items` then `item keys: k00, k01, … k23` and nothing about `k24`..`k29`. Verified by running it on 2026-10-17 (a throwaway test printing the shape): the output ended at `k23` with no count.

## Reproduction

`describe_payload_shape(&json!([{k00..k29: n}]))`. Code: the `Value::Array(items)` arm of `describe_payload_shape` in `src/tools/format.rs`, `first.keys().map(String::as_str).take(MAX_ITEM_KEYS).collect()` followed by an unconditional `out.push_str`.

## Root cause

The array arm was written without the disclosure the object arm has; `take` discards the surplus uncounted.

## Fix

Not designed. Candidate: when `first.len() > MAX_ITEM_KEYS`, append `, … +{n} more` as the object key line does, then move `format.shape_item_keys` to `Probed` citing a test that seeds more than 24 keys and asserts the text.

## Tests added

None — filing only.

## Resume

1. Add the count to the `item keys:` line.
2. Move `format.shape_item_keys` in `src/tools/core/cap_probe.rs` from `NoMarker` to `Probed`.

## References

- `src/tools/format.rs`, `describe_payload_shape`, array arm
- `src/tools/core/cap_probe.rs` row `format.shape_item_keys`
- `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
