---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: medium
unverified: 'No regression test. The assertion would have to detect a PLACEHOLDER prescribing a policy — `n=<count>` — which means teaching the gate a second grammar (template syntax) on top of the one it has, in the same file whose two-namespace parsing problems are already an IC-6 member. Mitigated instead: the template line now names the check and says why it cannot enforce itself there, so the next author reads the limitation at the point of copying.'
---

# `no_class_field_states_a_bare_n` cannot fire on the template, which is the one surface guaranteed to mint violations

## Symptom

`docs/trackers/issue-clusters.md` § *Template for new entries* — the block every new class
is copied from — prescribed the retired policy:

```
**Members:** `filter={"tags": {"contains": "cluster/<slug>"}}` — n=<count>, <YYYY-MM-DD>
```

The gate that refuses exactly this passed on it, every run, since the 2026-09-02 count-policy
inversion.

## Why it cannot fire

`bare_n_values(rest: str) -> list[int]` parses **integers**. `<count>` is a placeholder, so
there is no integer, so there is nothing to return. The check is not weakly covering the
template — it is structurally incapable of reaching it.

That makes this a different defect from the backtick escape already filed as
`the-no-stored-count-gate-is-defeated-by-house-style`. That one is about a real number hidden
by a quotation marker. This one is about **no number at all**: an instruction to write a count
later, which instantiates into a violation the gate then refuses — with the document that
issued the instruction unnamed in the refusal.

## Why the template is the worst surface for it

Every other stale surface misleads whoever reads it. The template is **copied**, so it mints
violations at the rate new classes are created, and each one is refused at commit time by a
gate whose message points at the field rather than at the template that specified it. The
author's correct move — follow the ledger's own template — is the move that reddens the gate.

## Class

`cluster/selector-narrower-than-its-population` (`IC-18`): *"A selector … is narrower than the
population its name or its caller's intent implies. It runs to completion over a subset and
returns a well-formed answer."* The gate's intent is *no field prescribes or stores a bare
count*; its selector is *an integer after an unbackticked `n=`*. The template sits in the gap.

**Not `IC-6`.** Checked against the claim rather than by adjacency — the two neighbouring
findings in this file are both `IC-6`, and classifying by proximity to them is the error a
peer and I adjudicated an hour earlier (`IC-6` binds the *wrong* target; `IC-18` matches too
*little*). Nothing here is unrepresentable; the population is simply not reached.

## How it was found

Grepping for remaining occurrences of the retired prescription after fixing a *different*
surface. It is the sixth surface of one policy change: three updated by the original
inversion, a fourth by `4647762b` (a peer), a fifth by `707bff08`, and this one. Each repair
was made by an author reading one section and discharged there.

## The surface count, with its unit — because it has two defensible values

This file calls itself "a sixth surface". That number is only meaningful with the unit
attached, and a peer session derived the alternative rather than accepting mine:

| unit | count | members |
|---|---|---|
| surfaces that **instruct an author** | **6** | 3 from the original inversion, `4647762b`, `707bff08`, this one |
| surfaces that **state what the field carries** | **8** | the six, plus two internal doc comments |

The two extra are present-tense rationale inside the enforcement layer itself, both verified
verbatim:

- `scripts/pre-commit-ledger-counts.py:272` — *"Both fields are read: `**Members:**` states the
  count, `**Promotes to:**` reasons from it…"*
- `tests/issue_clusters.rs:551` — *"Both fields are read. `**Members:**` states the count;
  `**Promotes to:**` *reasons* from it…"*

Under current policy a `**Members:**` field must **not** state a count — which is what the
function each sentence documents exists to refuse. So each is a sentence describing the retired
policy, sitting in the code that enforces its replacement.

**Deliberately not filed and not fixed.** The operative half of both — *both fields are scanned,
and `**Promotes to:**` is where the 2026-09-01 drifts did their damage* — is still true, the
audience is whoever maintains the gate rather than an author writing a field, and a tense repair
in two internal rationales is not worth a gate run on a nine-session tree. Filing them would be
padding a defect population inside a record about how populations get padded.

**Why this section exists at all.** "Seven surfaces" was published in a peer message and was
wrong twice over: the enumeration behind it listed six, and the unit was never stated, so even
the corrected 6 is the right answer to only one of two reasonable questions. CLAUDE.md:
*"A count of a defect population must arrive with its unit or not at all — one population
yielded four defensible numbers inside an hour, each the right answer to a different
question."* That law, breaking inside the ledger whose Index blockquote retired a stored count
for the same reason, is worth more on the record than a tidy figure would have been.

## Fix

`docs/trackers/issue-clusters.md` § *Template for new entries* — the `**Members:**` line now
describes a per-member derivation, names `scripts/probe-cluster-census.py` as the way to get a
figure, and states in the template itself that the gate cannot enforce this line. A limitation
declared at the point of copying, since it cannot be gated.

**SHA:** recorded in the commit that carries this file.

## References

- `scripts/pre-commit-ledger-counts.py:224` — `bare_n_values`, and its `list[int]` return
- `docs/issues/2026-09-02-the-no-stored-count-gate-is-defeated-by-house-style.md` — the
  backtick escape; adjacent, different mechanism
- `4647762b`, `707bff08` — the fourth and fifth surfaces of the same policy change
