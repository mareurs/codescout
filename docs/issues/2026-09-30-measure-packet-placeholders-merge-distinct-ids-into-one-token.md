---
id: '6e9d80d2be97ff43'
kind: bug
status: open
title: A labelling packet's placeholders merge distinct ids into one token, so same-versus-different is lost
tags:
- cluster/unclassified
closed: ''
opened: 2026-09-30
owner: marius
related:
- measure-packet-blinding-patterns-miss-a-value-glued-to-a-word-character
severity: medium
---

# BUG: a labelling packet's placeholders merge distinct ids into one token, so same-versus-different is lost

## Summary

`_blind` replaces every UUID with `<uuid>`, every timestamp with `<timestamp>` and every API id with
`<id>`. Two different sessions, agents or instants therefore look identical, and a labeller cannot tell
whether the agent reused a value or used a new one. That is harmless for most cases, but it hides the
evidence in a case about identity (the wrong session or artifact) or timing (stale state).

## Symptom (Effect)

Walkthrough item 5 (relayed, no packet content):

```
Placeholders for ids and timestamps hide whether a real value was used. That is harmless in most cases
but would hurt a case about identity or timing.
```

## Reproduction

`python3 -c "import packet as p; print(p._blind('a 11111111-2222-3333-4444-555555555555 b 66666666-7777-8888-9999-000000000000'))"`
from `scripts/measure/` prints `a <uuid> b <uuid>`.

## Environment

`experiments` at `ee313cce`; python3.

## Root cause

`_blind` (`scripts/measure/packet.py:104-109`) calls `.sub("<uuid>", s)`, `.sub("<timestamp>", s)` and
`.sub("<id>", s)`, each with a constant replacement. It is also called separately for each piece of the
packet (the operator message at `scripts/measure/packet.py:336`, each context text in `_context_block`,
the arguments in `_call`). So there is no packet-wide state from which numbered placeholders could be
consistent. measured 2026-09-30 with the reproduction above.

## Evidence

Probe output: `a <uuid> b <uuid>` for two distinct UUIDs.

## Hypotheses tried

None.

## Fix

**Needs an operator ruling**, because it changes what the labeller sees. The controller's leaning
(2026-09-30) is to number the placeholders within one packet (`<uuid-1>`, `<uuid-2>`, `<timestamp-1>`)
in order of first appearance. That keeps same-versus-different visible and reveals no value.

Implementation note: the numbering must be packet-scoped, not per `_blind` call. Otherwise one value
gets different numbers in different sections, which is worse than today's single token. Thread a mapping
object through `build_packet` into every `_blind` call. Numbering must not make the placeholder longer
than what the cut windows assume (see the fixed-position-cuts sibling). Timestamps could be shown as
relative offsets instead of numbers. That is a larger change; raise it with the operator rather than
choose it.

## Tests added

None yet. Owed: the same UUID in the operator message and in a context result gets the same number; two
different UUIDs get different numbers; the numbering is deterministic for the same unit. Mutate the
mapping to be per-call and confirm the cross-section test fails.

## Workarounds

None.

## Resume

Get the operator's ruling (numbered or not; relative timestamps or not), then write the tests.
Constraints are in the Resume of `measure-packet-cuts-results-and-arguments-without-a-marker`:
`packet.py` is frozen by `run.py frame`; batch the fixes; never read a real packet.

## References

- Sibling: `measure-packet-blinding-patterns-miss-a-value-glued-to-a-word-character` (same function).
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
