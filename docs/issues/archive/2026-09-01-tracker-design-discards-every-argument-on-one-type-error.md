---
id: '8275db3577856746'
kind: bug
status: fixed
title: 'BUG: `tracker_design` discarded EVERY argument when any one was ill-typed, and reported success'
tags:
- librarian
- tracker_design
- serde
- schema-drift
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-01
opened: 2026-09-01
owner: marius
severity: med
---

## Summary

`tracker_design::call` opened with:

```rust
let a: Args = serde_json::from_value(args).unwrap_or_default();
```

A type error on **one** argument silently produced `Args::default()` — discarding **every**
argument — and the call returned success with the archetype menu. `tracker_design(archetype=[])`
did not report a bad `archetype`; it reported the menu, which is what a caller sees when they
ask for no archetype at all. A well-formed `intent` sent in the same call went with it.

## Symptom (Effect)

The caller's only feedback is a successful response containing a plausible payload. There is no
observation that distinguishes *"my archetype was applied"* from *"my archetype was thrown away
and so was everything else."* That is `IC-15`'s defining property, and this is a sharper form
than the class's usual shape: the usual member drops **one** declared param, this one drops
**all** of them on any single malformed value.

## Evidence

Found by `Librarian::every_action_labelled_schema_key_is_honored_by_that_action`
(`src/librarian/tools/librarian.rs`) on its **first run**, before it had ever been green. The
probe flagged four keys; two were this defect and two were its own blindness:

| flagged | verdict |
|---|---|
| `tracker_design:intent` | real — the `unwrap_or_default()` above |
| `tracker_design:archetype` | real — same line |
| `doctor:offset` | probe blindness — read via an untyped `args.get(k)` accessor |
| `doctor:fix` | probe blindness — same |

**`Args` really does declare both fields** (`intent: Option<String>`, `archetype:
Option<String>`, both `#[serde(default)]`), which is why a source-level "is the param
deserialized?" check would have passed it. The defect is one line further down, in what happens
to the `Result`. A static scan for missing fields cannot see it; the behavioural probe can.

## Root cause

`unwrap_or_default()` on a `Result<Args, _>` conflates two different worlds: *"the caller sent
nothing"* and *"the caller sent something I could not read."* The first deserves defaults; the
second deserves a refusal. Collapsing them is `OB-6`'s shape — a third state existed and was
folded into the confident one.

## Fix

`bcf6075c` (patch-id `00b798d4b3945be2d1c8d2cf10a338f4a31c2263`, `experiments`). The
deserialisation now maps its error into a `RecoverableError` naming the tool and the valid
archetype names.

**Refuse, do not start honouring** — `IC-15`'s stated remedy, and it applies cleanly here
because nothing was being honoured incorrectly; the values were being discarded. There is no
caller who has come to rely on the old behaviour except by accident, and for them the new error
is the first true thing the tool has told them.

## Tests added

`a_malformed_argument_is_refused_rather_than_silently_dropped` in
`src/librarian/tools/tracker_design.rs`.

**It asserts a pair, deliberately.** The malformed call must error **and** the well-formed one
must still succeed. Asserting only the first passes for a version that rejects everything;
asserting only the second passes for the buggy version, which succeeded at all times. Only the
pair discriminates.

Mutation-verified: reverting the fix fails exactly that test — 17 passed, 1 failed — and no
other.

## Why the test suite could not catch this

`tracker_design` had 18 tests. Every one passed a well-formed argument or none at all, so every
one exercised the `Ok` branch of a `Result` whose `Err` branch was the defect. No assertion in
the module was monotone in the right direction to see it: they all confirm that good input
produces good output, and the bug is entirely about what bad input produces.

The general probe is what changed the question — it is the only test here that constructs an
input *designed to fail deserialisation*, which is a case no feature test has a reason to write.

