---
id: 3cf36991ec89a020
kind: bug
status: archived
title: In a labelling packet a tool-only message reads as missing content, and the judged message is easy to confuse with the context
tags:
- cluster/unclassified
closed: 2026-09-30
opened: 2026-09-30
owner: marius
related: []
severity: medium
---

# BUG: in a labelling packet a tool-only message reads as missing content, and the judged message is easy to confuse with the context

## Summary

When the judged assistant message has no prose, only tool calls, its section holds nothing but the
`ABOUT TO RUN:` block. That reads as if the text failed to render. Separately, a long earlier message in
the Context section is easy to mistake for the one being judged, because the judged message is marked
only by an ordinary `## The message` heading at the bottom.

## Symptom (Effect)

Walkthrough item 7 (relayed, no packet content):

```
"The message" can be only a tool call with no prose, which reads as missing content. A long earlier
message in the context is easy to mistake for the one being judged.
```

## Reproduction

Not reproduced on a packet. The controller is blind to real packets. The mechanism follows from the two
functions below and can be shown on the synthetic fixture: build a packet for a unit whose text is empty
and which has tool calls.

## Environment

`experiments` at `ee313cce`.

## Root cause

- `_join_body` (`scripts/measure/packet.py:209-211`) includes the text only if it is non-empty and
  falls back to `"(no text)"` only when there is neither text nor calls. A tool-only message therefore
  gets no statement that it has no prose.
- `_render` (`scripts/measure/packet.py:250`) emits `## The message` with no other emphasis, while
  context messages are `### −6` … `### −1` (`_context_block`, `scripts/measure/packet.py:189-202`).

inferred from `scripts/measure/packet.py:209-211,250`; not measured on a packet.

## Evidence

Walkthrough item 7.

## Hypotheses tried

None.

## Fix

**Landed in `1f42d84a`, patch-id `4e27ab51269db2579fe05550cfc6515b761d03e9`** (one commit shared with `f32b7d24` and `8829a4aa`). Steps 1 and 2 done: the note is `(no text; the message is only the tool call(s) below)` and the section is `## The message (the one you judge)`; the Context note was not added. `label.py` quotes only the phrase "The message" in prose and nothing parses the heading (grepped). Because the heading is 20 chars longer, the body budget is 19,886 and the packet sha256 pins moved.

This changes presentation only; the unit definition is untouched.

1. When the judged message has no text, write an explicit line before the calls, for example
   `(no text; the message is only the tool call(s) below)`.
2. Make the judged section unmistakable, for example `## The message (the one you judge)`. Consider a
   one-line note under `## Context` saying these are earlier messages, for background.
3. If the heading text changes, check `label.py` and its tests for anything that parses or quotes it.

**Out of scope, flagged for the operator:** whether a tool-only message should be a unit at all. It is a
unit by design today, because the pending action is the decision point. Changing that is a
`sampler.py` change: a new frame, new counts and a new pilot draw.

## Tests added

`ToolOnlyMessage` in `tests/test_measure_packet.py` (5 tests): a tool-only message carries the note before `ABOUT TO RUN:`; a message with text does not; a text-less CONTEXT message keeps its old shape; the judged heading appears exactly once and no context block borrows it; a message with neither text nor calls keeps `(no text)`. `UnitTrim.test_a_tool_only_message_drops_only_its_note_when_only_the_note_does_not_fit` reaches the two new `_fit` branches (a 63-call block of 19,833 chars leaves 51 spare, more than the 49-char trim marker, which is where an unguarded trim would print a trim marker over empty text).

Mutations (5, on a scratchpad copy): note dropped in `_join_body`, note dropped in `_fit`'s second branch, unguarded trim on empty text, note-yield branch removed, heading reverted: all KILLED. Not run: the cargo gate (Python only); left un-archived.

## Workarounds

None.

## Resume

Write the tests against the synthetic fixture in `tests/test_measure_packet.py`, then change
`_join_body`/`_render`. Constraints are in the Resume of
`measure-packet-cuts-results-and-arguments-without-a-marker`: `packet.py` is frozen by `run.py frame`;
batch the fixes; never read a real packet.

## References

- Spec: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` L120-132.
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.

## Fix provenance

- **SHA:** `1f42d84a7201ce99e624eac18b10adf41e0ed671` (`experiments`)
- **patch-id:** `4e27ab51269db2579fe05550cfc6515b761d03e9`
