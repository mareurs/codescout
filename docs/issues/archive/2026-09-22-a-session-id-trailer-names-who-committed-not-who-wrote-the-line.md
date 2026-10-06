---
id: eca907bc945a1291
kind: bug
status: fixed
title: 'BUG: a Session-Id trailer names who committed, not who wrote the lines the commit changes'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- git
- attribution
- provenance
- eval-corpus
closed: 2026-10-06
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

No code change. The remedy is a reading rule, and it is now written once, in a convention page: [`docs/conventions/session-id-trailer-attributes-the-committer.md`](../conventions/session-id-trailer-attributes-the-committer.md) (added by `39960f6b`, 74 lines, one new file).

> To attribute a corrected claim to its author, blame the **removed** side of the hunk at the parent and take the **antecedent commit's** trailer, read with git's own trailer parser — never the correcting commit's.

The page records the rule, why the failure is silent, and the 181 to 120 commits mis-attribution measurement (the census in Evidence above), and lists the two callers.

The earlier "one caller, not yet a convention" was stale when this was closed: `scripts/measure/miner.py` already encodes the rule in code (module docstring `:8-12`, which cites this bug file; `session_id_of` `:89-107` reads the trailer through `%(trailers:key=Session-Id,valueonly)`; `_blame_shas` `:173` blames at the parent over the old-side range; `_antecedents_of` `:209`; the antecedent's trailer is read at `:351`), and `docs/evals/rule-injection-timing-preregistration.md:76` sizes its corpus by it (120 commits across 45 sessions, not 181). Where the rule lives is therefore adjudicated: a convention page that both point to. It was not placed in `get_guide("tracker-conventions")` or in a `scripts/` helper of its own.

Not done: the new page has no catalog `id` yet (a reindex mints one).

## Tests added

none: documentation fix. There is no code path to guard; the defect is in how a trailer is read. The rule is exercised by `scripts/measure/miner.py` in its own right, not by a test added here.

## Workarounds

The blame step above. It costs one `git blame` per changed hunk and is what the census used.

## Resume

Closed on 2026-10-06 by `39960f6b`, which writes the rule down once as a convention; the code that applies it predates this closure. Residual follow-ups (listed, not filed):

- Whether the 1-in-6 ratio holds outside the correction-shaped sample is still unmeasured.
- Whether pure additions (27 of 181) deserve a distinct treatment rather than exclusion is still open; `miner.py` handles one case (a retraction note blames the line it annotates).
- The convention page was not yet catalogued when this closed (no `id`); confirm a reindex has minted one.
- The convention page says the correcting commit's trailer is "used only to decide `self` versus `peer-session`", but `_commit_candidates_for_row` also reads it (`miner.py:330`) to resolve the candidate's own session `sid` (`:335-343`, used at `:363`) and to gate untrailered commits. That is the right frame (the session where the correction happened), but the sentence overstates how little the correcting trailer is used; worth tightening on the page.

## Fix provenance

- **SHA:** `39960f6b` (`experiments`)
- **patch-id:** `336173f957ff0b980f838fd927e697be87a8a006`

## References

- `docs/evals/rule-injection-timing-preregistration.md` (`11f039dc91b862ec`) — the live consumer, and where the 120/45 bound is recorded.
- `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md` — the class, which already carries the sibling reflog-`author` finding.
