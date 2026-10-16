---
kind: bug
status: open
title: describe_payload_shape drops over-length and surplus scalars from its shape line with no "+N more"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
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

Not designed. Candidate: count the filtered and the taken-away scalars and append `… +N more scalars` the way the key line does. A reader-facing nicety, not a correctness defect.

## Tests added

None — filing only.

## Resume

1. Add the count to the scalar line (both the top-level pass and `flat_scalars`).
2. Move `format.shape_scalar_len` and `format.shape_scalars` to `Probed` in `src/tools/core/cap_probe.rs`, citing a test that drives past each bound and asserts the text.

## References

- `src/tools/format.rs:143-230`
- `src/tools/core/cap_probe.rs` rows `format.shape_scalar_len`, `format.shape_scalars`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
