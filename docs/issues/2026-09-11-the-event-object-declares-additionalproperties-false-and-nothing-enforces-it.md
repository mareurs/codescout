---
id: ea1dff07d9ed936c
kind: bug
status: open
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

Add `#[serde(deny_unknown_fields)]` to `event_create::Args`, and a regression test asserting an
unknown nested key is refused rather than dropped.

Check the same question for every other flattened sub-object as part of the same change rather
than one at a time: the two known folds are `event` and `augment`, and the population is
whatever `artifact.rs` reaches through a `flatten_*_args` helper.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet.

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
