---
id: a8f384cc0052d7b9
kind: bug
status: investigating
title: 'BUG: predicate probe counts next-call buffer syntax and repeated arguments as retrieval and redundancy'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-20
owner: marius
severity: medium
---

# BUG: predicate probe observations are broader in prose than in the instrument

## Summary

The observer-phase research and overflow handoff promote next-call reference syntax into eventual retrieval of a particular overflow, and identical read arguments into redundant reads. The implementation observes neither eventual target retrieval nor unchanged source content.

## Symptom (Effect)

The research Q4 correctly labels its table as next-call classification, but its implication and the handoff describe results as never retrieved. Q2 calls identical-argument reads unambiguously redundant. These conclusions exceed the predicates and could select the wrong intervention.

## Reproduction

At HEAD `934126f8`, import `scripts/probe-predicate-candidates.py` with importlib and call its functions with synthetic rows. Each row supplies session_id, called_at, tool, input_json and overflowed.

1. Overflowing read, then grep, then read of its buffer: q4 reports `switched_tool`, even though the later read is present.
2. Overflowing read, then read of an unrelated buffer: classify_next reports `queried_the_buffer`.
3. Read a path, edit the same path, repeat the read arguments: q2 reports one `repeat_with_identical_args`.

Observed in a direct Python execution on 2026-09-20; exit code 0. These are counterexamples to the interpretation, not a remeasurement of the historical corpus.

## Environment

Shared codescout checkout, HEAD `934126f8`; direct calls to the committed Python probe. No usage payloads copied or exported.

## Root cause

q4 examines only seq[i + 1]. classify_next looks for any REF_PREFIXES string in the next input and never reads the originating response to match its handle. q2 retains normalized argument identity by session/path without checking intervening edits or content versions. Argument equality does not imply result equality.

## Evidence

- `scripts/probe-predicate-candidates.py`: q4, classify_next, q2, opened through symbols and exercised directly.
- `docs/research/2026-09-20-predicate-candidates-for-the-observer-phase.md`: Q2, Q4 and implications.
- `docs/trackers/2026-09-20-handoff-overflow-retrieval-discriminator.md`: measured versus unmeasured section.

## Hypotheses tried

The instrument might track a result's exact handle across subsequent calls. Rejected by the function bodies and the delayed/unrelated-buffer counterexamples.

Repeated arguments might by construction mean redundant reads. Rejected by the read/edit/read counterexample; historical content equality remains unmeasured.

## Fix

Not implemented. First narrow the prose to the literal observables: next recorded call contains buffer-reference syntax; read arguments repeat within a session. Any claim about eventual retrieval requires matching the originating handle over a declared observation horizon. Any redundancy verdict requires source-state/principal evidence. Whether unread content was needed remains a separate outcome question even after correcting linkage.

## Tests added

No persistent tests. Three direct synthetic probes observed the behaviors above.

## Workarounds

Use these figures for case discovery only. Do not report 32.1% as an eventual retrieval rate or its complement as never retrieved. Do not automatically suppress repeated reads.

## Resume

Reconcile the research/handoff wording with the instrument before selecting a predicate or intervention. No implementation or historical rerun was performed in this session.

**2026-09-21 — prose narrowed to the literal observables; the instrument is UNCHANGED and this defect is live.** Four surfaces reconciled, in the same commit as this note:

- `docs/trackers/2026-09-20-handoff-overflow-retrieval-discriminator.md` (`ff49e054ef02e6c3`) — retitled off "unretrieved"; the question and the measured/not-measured sections now state the next-call classification and both linkage defects; the linkage repair is named as part of the handed-off work rather than as a precondition someone already met.
- `docs/research/2026-09-20-predicate-candidates-for-the-observer-phase.md` (`555135b94c321741`) — Q2's "the same bytes fetched twice", Q4's "moved on without retrieving it", and both implication bullets. **The Q4 table's labels were already correct and were left untouched.** That is the lesson worth keeping: the instrument and its immediate labelling were honest, and the promotion into an eventual-retrieval rate happened in prose one section downstream.
- `scripts/probe-predicate-candidates.py` — `(redundant)` removed from `q2`'s docstring; `classify_next` given a docstring naming its horizon and linkage limits. Comment-only, no logic touched, `py_compile` clean, so the committed figures stay reproducible.
- `docs/PROBES.md` (`fe0cdbd8eecdebd9`) — the Q4 blind spot was absent from the column whose entire purpose is blind spots.

Deliberately NOT done, and the reason this stays open rather than archived: `classify_next` still cannot match a handle to its origin, so **no eventual-retrieval figure exists**. Producing one needs handle matching over a **declared observation horizon** — chosen and stated, since "the next call" is itself a horizon and is the one that produced the overclaim. A redundancy verdict likewise still needs source-state evidence `q2` does not collect. And whether unread content was *needed* remains a separate outcome question even after the linkage is repaired.

No figure was recomputed and no historical rerun was performed, so the magnitudes of both errors remain unmeasured; 32.1% was left in place throughout as a correct measurement of a narrower thing.

## References

- `docs/research/2026-09-20-predicate-candidates-for-the-observer-phase.md`
- `docs/trackers/2026-09-20-handoff-overflow-retrieval-discriminator.md`
