---
id: '3bbaeeac07baaca8'
kind: bug
status: open
title: A labelling packet's fixed-position cuts drop the part of a tool call or result that the judgement needs
tags:
- cluster/truncated-window-ordered-by-the-wrong-key
closed: ''
opened: 2026-09-30
owner: marius
related:
- measure-packet-cuts-results-and-arguments-without-a-marker
severity: high
---

# BUG: a labelling packet's fixed-position cuts drop the part of a tool call or result that the judgement needs

## Summary

The packet rule cuts by position: the first 300 characters of a tool call's arguments, and the last
1,500 characters of a tool result. Where the decision-relevant content sits does not enter into it. So
the body of a call that writes a durable record (a file, a tracker entry, a memory) is cut away after its
path, and the counts, headers and first section of a result are dropped in favour of its tail. The
labeller then judges a write without seeing what was written.

## Symptom (Effect)

Reported by the sandboxed pilot walkthrough (relayed, no packet content):

```
1. Tool-call arguments cut at a fixed length hide the body of any call that writes a durable record.
   That body is often what the judgement depends on.
2. A result shown as its tail loses its start: counts, headers and the first section of multi-part output.
3. Structured search results cut mid-structure are hard to read and end in stray markers.
4. Broad searches in the context use up the context window without informing the judgement.
```

## Reproduction

Synthetic only:
`python3 -c "import packet as p; print(p._call('Write', {'file_path': '/x/y.md', 'content': 'BODY ' * 200}))"`
run from `scripts/measure/`. The output shows the path and the start of `content`, and the rest is gone.

## Environment

`experiments` at `ee313cce`; python3.

## Root cause

- Arguments: `_call` (`scripts/measure/packet.py:131`) serialises `inp` in its own key order and keeps
  the first `ARGS_CHARS = 300` (`scripts/measure/packet.py:35`). For every writing tool the path key comes
  first, so the window lands on the path and cuts the body.
- Results: `_result_line` (`scripts/measure/packet.py:151`) keeps the last `RESULT_TAIL_CHARS = 1500`
  (`scripts/measure/packet.py:34`). Many tools put their summary (counts, headers) at the head.
- Structured results (item 3) are the same tail cut landing inside a JSON document.
- Item 4: context blocks keep their full cut size whether they carry signal or not, and the
  `PACKET_CHARS = 20000` cap trims the oldest material first (spec L132). It is bounded, not selective.
- The constants are the **registered** packet rule: spec L125 ("arguments cut to 300 characters") and
  L126 ("the **last** 1,500 characters"). The fix therefore changes the spec text as well as the code.

inferred from `scripts/measure/packet.py:131,151` and confirmed on synthetic input 2026-09-30. Not
measured on real packets (the controller is blind to them).

## Evidence

The walkthrough report quoted above is the only evidence from real packets.

## Hypotheses tried

None.

## Fix

**Needs an operator ruling before implementation**, because it changes registered constants. Options
the pilot controller put to the operator on 2026-09-30, with its leaning:

1. **Arguments.** Raise the limit only for tools that write a record, for example to 1,500. The set is
   already defined as `EDIT_TOOLS` and the catalog-write rule (`CATALOG_TOOLS` × `CATALOG_WRITE_ACTIONS`)
   in `scripts/measure/sampler.py:26,30-31`. Import those sets rather than copying them. Leaning: yes.
2. **Results.** Head plus tail, for example the first 500 and the last 1,000, with a marker between.
   Leaning: yes; it also mostly answers item 3.
3. **Item 4:** accept for now. The cap bounds the damage, and a per-tool budget adds complexity.

Update spec L125-126 in the same change, or in Amendment 1, which has not been committed yet. The
marker work is in `measure-packet-cuts-results-and-arguments-without-a-marker`. Land them together.

## Tests added

None yet. Owed: a record-writing call keeps its body up to the new limit while a non-writing call keeps
300. A head-plus-tail result keeps both its first and last lines. The per-tool limit needs a mutation that
swaps the set membership, and a test that fails if the non-writing limit also grows. That last test
catches the monotone direction.

## Workarounds

None. Do not label packets rendered before the fix.

## Resume

Get the operator's ruling on options 1 and 2 (and the exact numbers). Then write the tests in
`tests/test_measure_packet.py`. Constraints (frozen file, blindness, deadline, test discipline) are in the
Resume of `measure-packet-cuts-results-and-arguments-without-a-marker`. In short: `packet.py` is hashed
by `run.py frame`, so batch the fixes and then re-freeze, re-run the preflight and re-render. Never read
a real packet.

## References

- Spec: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` L120-132.
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
- Related open bug on the judge's context window: `2026-09-29-judge-audit-context-counts-text-less-tool-rows-toward-its-turn-cap`.
