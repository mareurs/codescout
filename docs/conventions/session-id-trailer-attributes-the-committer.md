---
kind: convention
status: active
title: A Session-Id trailer names who committed, not who wrote the line
owners: [marius]
tags:
- git
- attribution
- provenance
- eval-corpus
- conventions
topic: attributing a corrected claim to the session that made it
---

# A `Session-Id` trailer names who committed, not who wrote the line

A `Session-Id:` commit trailer names the session that **ran `git commit`**. It says nothing
about the session that **produced the lines the commit changes**. For a commit that corrects an
earlier claim, those are different sessions often enough to matter, and nothing marks which
case you are in. The bug file that measured it is
[`2026-09-22-a-session-id-trailer-names-who-committed-not-who-wrote-the-line.md`](../issues/archive/2026-09-22-a-session-id-trailer-names-who-committed-not-who-wrote-the-line.md).

## The rule

To attribute a corrected claim to the session that made it:

1. **Blame the removed side of each hunk, at the commit's parent.** `git blame --porcelain
   -L <old-start>,+<old-count> <parent> -- <path>`. The old-side line range selects the lines
   the commit replaced; blaming at the parent returns the commit that wrote them. Blaming at the
   correcting commit itself would attribute the new lines to the corrector.
2. **Read the antecedent commit's trailer**, with git's own parser
   (`--format='%(trailers:key=Session-Id,valueonly)'`), never a regex over the body.
3. **Never use the correcting commit's own trailer** as the author of the error. It is the
   right answer to a different question: who recorded the correction.

A hunk that removes nothing has no antecedent. Count it as unattributable rather than falling
back to the correcting commit's trailer. (The miner's one exception: a pure addition whose added
lines are a retraction note blames the single line before the insertion point, the sentence it
annotates, `miner.py:219-223`.)

## Why

The trailer is exactly reliable for what it states, so the failure is silent. Every trailer
resolves to a real session with a real transcript on disk; a lookup keyed on the wrong one
succeeds and yields a confidently sourced claim about a session that never made the mistake.
On a shared checkout where several sessions commit to one branch within minutes, the previous
commit on the branch is no substitute for the blame step: the parent is the last commit made,
not the commit that wrote the line.

Measured 2026-09-22 over 181 correction-shaped commits (a lexical selection on commit
subjects, not a defect population): the same session wrote the antecedent in 120, a different
session in 30, 27 were pure additions with no antecedent, 4 had an untrailered antecedent.
Roughly one in six attributes to the wrong session under the naive reading. Trailer coverage
is September 2026 only, so none of this reaches earlier commits.

## Callers

- `scripts/measure/miner.py` — encodes the rule in code. The module docstring states it
  (`:8-12`); `_blame_shas` blames at the parent over the old-side range (`:173-194`);
  `_antecedents_of` collects the antecedents (`:209-230`); the antecedent's trailer is read
  through `session_id_of` (defined `:89`, called `:351`), and the correcting commit's trailer
  is used only to decide `self` versus `peer-session` (`:358-360`), never as the origin.
- `docs/evals/rule-injection-timing-preregistration.md:76` — applies the rule to size its
  corpus: 120 correction commits across 45 sessions, not 181, because 30 were written by a
  different session and 27 had no antecedent.

## The failure it prevents

Seeding a replay experiment with transcripts that do not contain the violation being replayed.
The naive reading inflates the preregistered corpus from 120 to 181 (about 51%) and points part
of it at sessions that did not make the error.

If a third analysis needs the same attribution, call `session_id_of` and the blame helpers in
`scripts/measure/miner.py` rather than re-deriving them.
