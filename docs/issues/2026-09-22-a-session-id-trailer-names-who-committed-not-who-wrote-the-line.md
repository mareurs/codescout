---
id: '95f8edf02e65851e'
kind: bug
status: open
title: 'BUG: a Session-Id trailer names who committed, not who wrote the lines the commit changes'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- git
- attribution
- provenance
- eval-corpus
---

**Valid:** dated 2026-09-22

## Summary

A `Session-Id:` commit trailer names the session that **made the commit**. It does not name the session that **wrote the lines the commit changes**. Reading it as authorship mis-attributes a large minority of corrections — silently, because the trailer resolves to a real session with a real transcript on disk.

## Symptom (Effect)

An analysis that walks *correction commit → `Session-Id` → transcript → the turn that published the corrected claim* lands in the wrong transcript whenever the correction was written by a session other than the one that made the error. Nothing fails. The wrong transcript is a valid transcript, the turn lookup succeeds, and the result is a confidently-sourced claim about a session that never made the mistake.

## Reproduction

At tree `09acd397`, branch `experiments`:

1. `git log --format='%H %(trailers:key=Session-Id,valueonly)'` — trailer coverage.
2. Select commits whose subject matches a correction-shaped pattern.
3. For each, `git blame` the **removed** side of each hunk at the parent, and read the antecedent commit's own trailer.
4. Compare the antecedent's session against the correcting commit's session.

## Environment

codescout, `experiments`. Trailer coverage is **September-2026-only** — re-derived at the current tree: 2026-02 through 2026-08 carry **zero**, 2026-09 carries **2033 of 2095** commits. (A census at `63e69d87` read 2030 of 2092; the delta is this session's own commits.) So the pipeline has no reach before September regardless of how the attribution question is resolved.

## Root cause

`Session-Id` is a **provenance trailer on the commit act**. It is written by whichever session runs `git commit`, and that is exactly what it says. Nothing about it is wrong.

The defect is in the reading. A correction commit's changed lines were, by definition, authored **earlier** — often by a different session, on a shared checkout where several sessions commit to the same branch within minutes. The trailer answers *who recorded this change*; a reader chasing a violation needs *who produced the text being changed*, and those are different questions with no marker distinguishing them.

## Evidence

Census at tree `63e69d87`, 2026-09-22, blaming the removed side of each hunk at the parent:

| antecedent of the corrected lines | commits |
|---|---|
| **same session wrote them** | **120** |
| a **different** session wrote them | **30** |
| pure addition — nothing removed, no antecedent | 27 |
| antecedent carries no trailer | 4 |
| **total examined** | **181** |

So roughly **one correction in six** attributes to the wrong session, and a further 27 attribute to nobody at all. Usable N for an authorship-keyed analysis is **120 commits across 45 sessions**, not 181.

**The 181 is itself bounded by a lexical proxy** over commit subjects (`retract|correct|falsif|withdr[ae]w|overstat|…`), which both over-counts — `correct` and `revert` are routine verbs — and under-counts corrections whose subject names the fact rather than the act. It is a selection, not a defect population, and should not be quoted as one.

**Measured, not inferred:** the trailer coverage figures and the four-way split. **Inferred:** that the same ratio holds outside the sampled pattern.

## Hypotheses tried

*"The trailer is simply unreliable."* Rejected — it is exactly reliable for what it states. Every one of the 181 resolved to a live session. The value is correct in its own frame.

*"Take the parent commit's trailer."* Insufficient — the parent is the previous commit on the branch, not the commit that wrote the line. On a shared checkout those differ constantly. The blame step is what identifies the antecedent.

## Fix

No code change proposed. The remedy is a reading rule, and it already has a caller:

> To attribute a corrected claim to its author, blame the **removed** side of the hunk and take the **antecedent commit's** trailer — never the correcting commit's.

`docs/evals/rule-injection-timing-preregistration.md` (`11f039dc91b862ec`, committed `645213ab`) applies this, and states its corpus as 120 commits across 45 sessions **because** of this defect. That is the cost made concrete: the naive reading would have inflated the corpus by 51% and seeded a replay experiment with transcripts that do not contain the violation being replayed.

Whether the rule belongs in `get_guide("tracker-conventions")`, in a `scripts/` helper, or only here is unadjudicated — one caller does not establish a convention.

## Tests added

None. There is no code path to guard; the defect is in how a trailer is read.

## Workarounds

The blame step above. It costs one `git blame` per changed hunk and is what the census used.

## Resume

Open questions: whether the 1-in-6 ratio holds outside the correction-shaped sample; whether pure additions (27 of 181) deserve a distinct treatment rather than exclusion; and whether a second caller appears, which would make this a convention rather than one analysis's private rule.

## References

- `docs/evals/rule-injection-timing-preregistration.md` (`11f039dc91b862ec`) — the live consumer, and where the 120/45 bound is recorded.
- `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md` — the class, which already carries the sibling reflog-`author` finding.
