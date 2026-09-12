---
id: 281db38f3fe2d783
kind: bug
status: fixed
title: 'BUG: doc''s `event` object declares additionalProperties:false and nothing enforces it'
tags:
- cluster/accepted-parameter-silently-dropped
---

## Summary

`doc`'s schema declares `"additionalProperties": false` on the `event` object
(`src/librarian/tools/artifact.rs`, the `"event"` property). Nothing enforces it.
`flatten_event_args` lifts the object's keys into `event_create::Args`, which carries no
`#[serde(deny_unknown_fields)]`, so an unknown nested key is **silently discarded** — the
`IC-15` shape, against a schema that promises the opposite in writing.

The sibling case is the control, not a comparison: `doc.augment` makes the same promise, its
description says so out loud (*"Unknown keys are REJECTED, not ignored — a typo here fails
loudly rather than silently dropping the field"*), and `augment::Args` **does** carry
`#[serde(deny_unknown_fields)]`. Two folds of the same shape, one honoured and one not.

## Symptom (Effect)

`doc(action="event_create", id=…, event={kind: "note", payload: {…}, athor: "x"})` — a typo for
`author` — succeeds, records the event, and drops `athor` without comment. The caller has no
signal, and the schema told them a typo would be refused.

## Reproduction

1. Add a nested key absent from `event_create::Args` to a live `event_create` call, e.g.
   `event={kind: "note", payload: {text: "x"}, athor: "me"}`.
2. Observe the call succeed and the event be written with no author.
3. Do the same against `augment` (`augment={prompt: "x", promt: "y"}`) and observe a refusal.

## Environment

`experiments` at `51dd7c15`. Both `Args` types are behind `feature = "librarian"`, so this is
default-lane-only.

## Root cause

`event_create::Args` (`src/librarian/tools/event_create.rs`) has no `deny_unknown_fields`.

`crate::tools::param_probe`'s module doc carries a blanket warning that the attribute "was tried
and broke every `doc(update)` call, because the dispatcher passes `action` down and the shared
schema holds sibling actions' keys". **That warning does not transfer here, and the reason is
worth stating because it is what makes this cheap:** `flatten_event_args`
(`src/librarian/tools/artifact.rs`) does not pass the shared blob down. It builds a fresh map
from the `event` object's own keys plus `artifact_id`, so `Args` never sees a sibling action's
key. That is exactly why `augment::Args` — reached the same way, via `flatten_augment_args` —
can already carry the attribute.

## Fix

Done. `#[serde(deny_unknown_fields)]` on `event_create::Args`, which is the filed defect.

**The population sweep this section asked for returned TWO sites, not one, and the second
needed a schema change the first did not.** Reached through the `flatten_*_args` helpers the
population is exactly `event` and `augment`, and `augment` was already correct — so that
framing was complete and the count was 1. But `event.source` deserialises through
`SourceArg`'s own impl, which ignores unknown keys however strict its parent is, so
`deny_unknown_fields` on `Args` does not reach it. It is a second guarded site by
construction.

`source` differed in the direction of its mismatch, which is why it could not be fixed the
same way: it declared **no** `additionalProperties` at all, so the code dropped a typo the
schema permitted. Adding only the attribute would have refused a call the published schema
still allowed — the same contradiction, pointing the other way. Both moved together:
`SourceArg` gains the attribute, and the `event.source` block in `artifact.rs` gains
`"additionalProperties": false`. `source_schema`'s schemars mirror in `event_create.rs`
gains it too and is annotated **INERT** — no `schema_for` call site exists in the crate, so
nothing materialises it and no test covers it; it is not to be credited as agreement.

That cost 29 chars of advertised tool surface against a budget sitting at headroom 0, so
`TOOL_SURFACE_CHAR_BUDGET` is ratcheted 55_711 -> 55_740 with the derivation in its log,
both ends measured on the tree rather than read off the constant.

Fix SHA: `6b64dc35`
Patch-id: `bab3c37387e6580058507b86af24119731bde240`

## Tests added

Both in `src/librarian/tools/artifact.rs`, and both derive their population from the schema
rather than listing it, so they close the CLASS and not just this instance.

- `every_schema_object_promising_to_refuse_unknown_keys_actually_refuses` — walks
  `Artifact.input_schema()` recursively for objects declaring `"additionalProperties":
  false`, asserts the exact set is `[augment, event, event.source]`, and for each sends a
  call that is valid except for one unknown key. Asserts the refusal **names the offending
  key**, not merely that one happened. A sub-object that starts making the promise arrives
  as an unhandled match arm carrying instructions; one that stops making it reds the set
  assertion, so a silently loosened contract cannot pass either.
- `every_schema_object_promising_refusal_still_accepts_its_declared_keys` — the
  opposite-direction partner, sending each guarded object its FULL declared key set. The
  gate above is monotone under refusing *more*, so a misspelled struct field name would
  break a documented key with that gate still perfectly green.

**Mutation-tested once per guarded site — three observed REDs, not one:**

1. `Args` without the attribute: the `event` probe fails, and the panic message shows the
   defect itself — an `event_id` returned for a call that carried `athor`.
2. `SourceArg` without the attribute: same, one level down.
3. The `event.source` schema line removed: the population assertion reds naming what moved.

Mutation 2 is worth keeping, because it falsified the probe rather than the fix. The arm
originally sent `source.kind: "k"`, which the `event_source` CHECK constraint rejects
independently — so the mutated code still *errored*, and a bare `expect_err` would have
passed it. Only the assertion that the message names `payloadd` caught it. The arm now
differs from a valid call in exactly the one unknown key, and the fixture line says why.

The forward-direction sibling `every_action_labelled_schema_key_is_honored_by_that_action`
ran green over this defect for its whole life, which is the point: it asks whether a
DECLARED key is honoured, and a surface that accepts everything satisfies it.

## Resume

Start by confirming the reproduction above at the tool surface, not by reading the struct — the
schema's promise and the code's behaviour are both easy to read and the point is which one a
caller meets. Then add the attribute and the test.

## References

- Found 2026-09-11 while fixing
  `docs/issues/archive/2026-09-02-param-probe-does-not-recurse-so-nesting-a-key-removes-it-from-guard-reach.md`,
  whose § Fix listed this as candidate fix **B** and preferred the recursion (fix A) instead. A
  was taken and verified; this is the part of B that A does not cover, and it is filed rather
  than folded in because the mechanism differs — A closes a *guard's* coverage at test time,
  this closes a *runtime* contradiction between schema and code.
- `docs/trackers/issue-clusters.md` § `IC-15` (`accepted-parameter-silently-dropped`).
