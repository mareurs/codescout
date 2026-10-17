---
id: c66b40d7c32404b1
kind: bug
status: open
title: describe_payload_shape's one-level descent names nothing for a nested object's arrays, objects and nulls
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
---

## Summary

When `describe_payload_shape` descends into an object-valued key (`flat_scalars`), it renders that object's string/number/bool members and discards every other member (array, nested object, null) without naming or counting it. At the top level those members are at least named in the `keys:` line and, for arrays, sized in `arrays:`; inside the descended object nothing names them. This predates the 2026-10-17 fix for `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`, which counts over-length and surplus entries but deliberately not these.

## Class fit

A result narrowed by its shape presented as the object's contents: `hints: {a=1}` reads as the whole of `hints` when `hints` also carried an array, an object and a null. It differs from the scalar-line members in that the discard is by KIND, not by a size bound, so no constant is involved and no `cap-class` annotation or `PROBE_ROWS` row can carry it.

## Symptom (Effect)

`{"hints": {"a": 1, "list": [1,2,3], "deep": {"x": 1}, "n": null}}` prints `1 keys: hints` and `hints: {a=1}`; `list`, `deep` and `n` appear nowhere. Verified by running it on 2026-10-17 (a throwaway test printing the shape).

## Reproduction

`describe_payload_shape(&json!({"hints": {"a":1,"list":[1,2,3],"deep":{"x":1},"n":null}}))`. Code: `flat_scalars` and the `ScalarCell::NotScalar` arm in `src/tools/format.rs`.

## Root cause

The descent is scalars-only by design (the narrow fix for `docs/issues/archive/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md`), and `ScalarCell::NotScalar` is the discard. Its doc comment's old claim that the key line and `arrays:` cover it holds only at the top level.

## Fix

Not designed. Candidates: count the non-scalar members and append them to the group (`hints: {a=1, … +3 non-scalar}`), or list their names. Whichever is chosen must keep the top level silent for non-scalars, which `arrays:` and `keys:` already name.

## Tests added

None — filing only.

## Resume

1. Decide the disclosure for nested non-scalar members and implement it in `flat_scalars`.
2. Add a test asserting the group carries it, and that the top level still adds nothing.

## References

- `src/tools/format.rs`, `describe_payload_shape/flat_scalars`
- `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
