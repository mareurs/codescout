---
id: '0733738c6f04653b'
kind: bug
status: fixed
title: 'BUG: artifact_augment/artifact(update) params merge-patch does not recurse into nested objects (contradicts documented RFC 7396 semantics)'
topic: codescout artifact_augment / artifact(update) merge-patch recursive-merge bug
closed: 2026-08-24
---

## Summary

`apply_merge_patch` (`src/librarian/catalog/augmentation.rs`), the shared function both `artifact_augment(merge=true)` and `artifact(action="update", patch={params:...})` funnel through via `merge_params` → `merge_params_dry`, only replaced top-level `params` keys wholesale — a patch naming one branch of a nested object silently deleted every untouched sibling branch under that same top-level key. Both tools' descriptions document full RFC 7396 recursive merge ("preserving every field you omit"); the code's own doc comment even called the behavior "Shallow RFC 7396 merge-patch" — a contradiction, since RFC 7396 is *defined* as recursive for object-valued keys.

Originally discovered and filed downstream in changelog-reader's `docs/trackers/bug-artifact-augment-shallow-merge.md` (artifact `d8993902fc9e08a7`), where it bit the `/changelog-check` skill twice before being diagnosed and worked around by flattening the affected state to top-level `params` keys.

## Reproduction

```
before: params = {"a": {"x": 1, "y": 2}, "b": "top-level-value"}
patch:  merge_params(cat, id, {"a": {"x": 99}})
after (buggy):  {"a": {"x": 99}, "b": "top-level-value"}          # a.y silently deleted
after (fixed):  {"a": {"x": 99, "y": 2}, "b": "top-level-value"}
```

Reproduced locally as an automated regression test — see § Tests added.

## Root cause

`apply_merge_patch` had one match arm, `(Value::Object(t), Value::Object(p))`, and inside it did `t.insert(k.clone(), v.clone())` unconditionally for every non-null patch value — replacing the target's value for that key wholesale regardless of whether the existing value was itself an object. No recursion, at any nesting depth. Confirmed via systematic debugging: traced both write paths to this one function, checked the two related prior bug archives (2026-08-16 array-wipe, 2026-07-02 bare-array no-op) to confirm neither decision covered nested *objects*, then wrote a failing test before touching the function.

## Fix

Recurse when both the existing target value and the patch value at a key are JSON objects; otherwise (arrays, scalars, a missing key) replace wholesale. This is exactly the RFC 7396 algorithm, and deliberately leaves the array-wholesale-replace behavior from `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md` unchanged — RFC 7396 itself only recurses on object-to-object keys, never on arrays, so that prior decision falls out of the spec for free.

## Tests added

`merge_params_recursively_merges_nested_objects` (`src/librarian/catalog/augmentation.rs`) — reproduces the bug's exact shape (`{"a":{"x":1,"y":2},"b":"top-level-value"}` patched with `{"a":{"x":99}}`). Confirmed FAILING before the fix (`params.a.y` came back `Null`), passing after.

## Verified live

- `cargo test --lib`: 4295 passed, 0 failed (full suite)
- `cargo clippy -- -D warnings`: clean
- `cargo fmt`: clean
- `augment`/`update` tool test modules (143 tests) and the pre-existing `merge_params_adds_key` / `merge_params_null_deletes_key` / entry-array tests all still pass unchanged — no regression to flat-params or array-wholesale-replace behavior.

## Resume

**Closed 2026-08-24.** Fix SHA on `experiments`: `a03b54b0`. Patch-id: `2fed5982ac35b5e4de1d505e1cd2d29a891d419d`.

## References

- changelog-reader `docs/trackers/bug-artifact-augment-shallow-merge.md` (artifact `d8993902fc9e08a7`) — original downstream report; updated with this fix's SHA/patch-id and a `## Fix` section.
- `docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md` — prior related decision (arrays stay wholesale-replace, intentionally; unaffected by this fix).

