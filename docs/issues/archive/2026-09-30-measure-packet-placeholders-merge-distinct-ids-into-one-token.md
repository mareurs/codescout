---
id: 64f2c7af4c2845fb
kind: bug
status: archived
title: A labelling packet's placeholders merge distinct ids into one token, so same-versus-different is lost
tags:
- cluster/unclassified
closed: 2026-09-30
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

**Ruled 2026-09-30 by the operator (relayed in session 00113c9d): numbered per packet, in order of first appearance; relative timestamps NOT chosen.** Sequenced AFTER the `3bbaeeac` wave (peer session codescout-cd is editing `_call`/`_result_line`/`build_packet` now); implementation starts when that commit lands. **Landed in `d16ab085`, patch-id `e17730c5765efae6330232d7584e8fe1cc6ebb17`** (own commit; not shared with the other packet fixes). Implemented as ruled: a `_Ids` per `build_packet`, threaded through `_context_block`, `_call`, `_result_line` and the operator and unit sites, numbering by first appearance in RENDER order (the context is blinded before the judged message, so the first cap-loop pass reuses those blocks). A uuid is the same value whatever its case. Deliberate gap: blinding runs before any cut or cap, so a value only in dropped material keeps its number. Original text:

**Needed an operator ruling**, because it changes what the labeller sees. The controller's leaning
(2026-09-30) is to number the placeholders within one packet (`<uuid-1>`, `<uuid-2>`, `<timestamp-1>`)
in order of first appearance. That keeps same-versus-different visible and reveals no value.

Implementation note: the numbering must be packet-scoped, not per `_blind` call. Otherwise one value
gets different numbers in different sections, which is worse than today's single token. Thread a mapping
object through `build_packet` into every `_blind` call. Numbering must not make the placeholder longer
than what the cut windows assume (see the fixed-position-cuts sibling). Timestamps could be shown as
relative offsets instead of numbers. That is a larger change; raise it with the operator rather than
choose it.

## Tests added

`NumberedPlaceholders` in `tests/test_measure_packet.py` (8 tests): one value keeps one number across sections and two values differ; numbers follow reading order not build order (the judged message's first-seen value takes the LAST number); a counter per kind; a repeat inside one string and inside call arguments; uuid case; the dispatch prompt shares the mapping; numbers past nine; the documented gap after the cap drops a context message. Nine legacy `Blinding` assertions moved to the numbered forms, and `test_every_place_transcript_text_enters_the_packet_is_blinded` now also pins the per-site order (1..5).

Mutations, one per site, on a scratchpad copy (14): uuid key not lower-cased; number always 1; number never remembered; one counter for all kinds; a fresh `_Ids()` at each of the six blinding call sites (`_call`, `_result_line`, `_context_block` text, operator/dispatch, unit text, and the call sites in the unit and in the context); the unit blinded before the context; the cap-loop rebuild with a fresh mapping. All KILLED, each pattern applied exactly once.

The pinned sha256s in `test_measure_run_sample.py` did NOT move (its corpus keeps ids in paths and set names, not in message text; observed, not proven). Not run: the four-command cargo gate (Python only); `cargo test --test committed_paths` passed. Left un-archived for that reason. **Still owed on the real corpus:** `run.py frame`, then `preflight`, then re-render the pilot, once, after this and `9a269f24`.

## Workarounds

None.

## Resume

Get the operator's ruling (numbered or not; relative timestamps or not), then write the tests.
Constraints are in the Resume of `measure-packet-cuts-results-and-arguments-without-a-marker`:
`packet.py` is frozen by `run.py frame`; batch the fixes; never read a real packet.

## References

- Sibling: `measure-packet-blinding-patterns-miss-a-value-glued-to-a-word-character` (same function).
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.

## Fix provenance

- **SHA:** `d16ab085c49a35d6add59cc473ea6dfc11ac5688` (`experiments`)
- **patch-id:** `e17730c5765efae6330232d7584e8fe1cc6ebb17`
