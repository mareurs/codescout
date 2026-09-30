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

**Ruled by the operator 2026-09-30: options 1 and 2 as leaned, with 1,500 characters for record-writing arguments and head 500 + tail 1,000 for results. Implemented and committed together with this note. The fix SHA and patch-id are recorded in a follow-up commit that also flips the status, since a commit cannot cite its own SHA (status stays `open` until then).** The original options follow for the record. Was: needs an operator ruling before implementation, because it changes registered constants. Options
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

`KeptWindows` in `tests/test_measure_packet.py` (12 tests, plus one exact-edge test in `TokenEdges`), each rule in BOTH directions:

- **Arguments:** every call in the live `sampler.EDIT_TOOLS` and every `CATALOG_TOOLS` x `CATALOG_WRITE_ACTIONS` pair keeps a 1,400-character body whole; the exact limit is 1,500 (whole at 1,500, one character marked at 1,501); and the other direction, a call that is not a record write (shell, read, a catalog tool with a read action or a non-string or missing action, and a write-shaped `action` on a non-catalog tool) still cuts at 300. A context call and the unit's own ABOUT TO RUN call are both covered.
- **Results:** whole at 1,500; at 1,501 the first 500 and last 1,000 with the one dropped character marked between; a long result keeps its header line and its last line; the exit code is still read from the WHOLE text when it sits in the dropped middle.
- **Token guard, per window:** a token straddling the end of the head, or the start of the tail, refuses (each built so the other window is clean); a token wholly in the dropped middle does not refuse; a token wholly in the kept head now refuses (it was dropped, and never refused, under the tail-only cut); wholly in the kept tail refuses. Exact-edge cases for both windows are in `TokenEdges`.

Nine legacy tests that pinned the tail-only shape were rewritten to the new windows, one renamed pair among them (`test_result_tail_kept_and_args_cut` is now `test_result_head_and_tail_kept_and_args_cut`, `test_exit_code_outside_the_kept_tail_is_still_shown` is now `test_the_exit_prefix_is_shown_for_both_forms_beside_the_kept_head`). The latter is annotated as INERT for the read-from-the-whole-text property, because the code now sits in the kept head in both forms.

**Mutation run, isolated worktree (`./scripts/mutation-probe.sh`), 21 mutants, 21 KILLED, 0 survived:** head 499/501, tail 999/1001, record limit 1499/1501, non-writer limit 301/1500, the edit half and the catalog half of `_writes_record` off, the `isinstance` guard dropped, the tool-name clause dropped, the limit ignoring the writer, the head leak flag and the tail leak flag each dropped alone, a whole-text token check (the over-refusal direction), whole at `<` and at `<= keep + 1`, the dropped count, the exit code read from the tail only, and the marker label. Read off unittest's summary line, because the probe's own verdict parse is cargo-only (it printed INCONCLUSIVE each time). Gap found by reasoning and closed before the run: without a non-catalog tool carrying a write-shaped `action`, dropping the tool-name clause survived.

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
