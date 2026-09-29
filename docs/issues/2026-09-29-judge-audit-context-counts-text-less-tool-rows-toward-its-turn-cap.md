---
id: '61f699816f5ee2fd'
kind: bug
status: open
title: The judge's audit-mode context counts text-less tool rows toward its 12-turn cap, so it shows no tool evidence
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: ''
opened: 2026-09-29
owner: marius
related: []
severity: medium
---

# BUG: the judge's audit-mode context counts text-less tool rows toward its 12-turn cap, so it shows no tool evidence

## Summary

R117 says an audit-mode judge input carries "the same session's preceding top-level turns, up to
12 turns and 20,000 chars". `judge._session_part` reads the events DB's `turns` table and counts
every row toward the 12. But `join._write_turn` writes tool-call and tool-result entries as `turns`
rows whose `text` is `None`, because `transcripts._message_text` returns `None` for them. So those
rows use up the cap while contributing nothing, and a tool's output never reaches the judge.

## Symptom (Effect)

A corpus profile run on 2026-09-29 over the frozen snapshot `2026-09-29-codescout` measured this
(count-only; the scripts are in the controller's scratchpad):

- 99.0% of the 69,979 eligible top-level packets stop at the 12-row cap, and 0.6% at the 20,000-char cap;
- the median sampled packet is 366 chars for substantive messages and 269 for routine ones, and
  spans about 4 assistant API messages;
- only about 10% of packets contain an operator prompt.

So "12 turns" is 12 transcript entries, and the judge would see a few hundred characters of narration
and no command output. Audit mode's `in-trace` detectability depends on counter-evidence being
visible before the decision, and the operator's approved example D (tests reported as passing while the visible
result shows failures) depends on a tool result. This context structurally hides both.

## Reproduction

Build an events DB for any frozen corpus (`join.build_events`) and call `judge.build_input` in audit
mode on a top-level assistant message that follows a tool call. The context lists the tool rows
with empty text, and they are counted among the 12.

## Environment

`scripts/measure/judge.py` `_session_part` (the `CONTEXT_MAX_TURNS = 12` and
`CONTEXT_MAX_CHARS = 20_000` constants); `scripts/measure/join.py` `_write_turn`;
`scripts/measure/transcripts.py` `_message_text`.

## Root cause

R117 names a unit ("turns") that the `turns` table defines differently: one row per transcript
entry, including entries with no text. The cap was written against the conversational reading and
implemented against the table's.

## Evidence

The completed Task 9b gate is not affected. Its items were document excerpts built by R120
(`DOC_CONTEXT_CHARS`), not `_session_part`, and audit mode never ran on the corpus.

## Fix

Open. The packet rule for operator-labelled cases is being redesigned (design session 2026-09-29),
and the judge must receive the same packet as the operator. So the fix belongs to that redesign:
count API messages rather than entries, and include tool calls and truncated tool results. Fix it
there rather than patching R117 alone.

## Tests added

None yet.

## References

- `docs/superpowers/specs/2026-09-26-system1-base-rate-measurement-design.md` (Amendment 8, R117)
