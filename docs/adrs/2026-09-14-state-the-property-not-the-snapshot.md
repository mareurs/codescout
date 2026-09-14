---
kind: adr
status: active
title: ADR-2026-09-14 — State the property, not the snapshot; and do not build a claim-checker
owners:
- marius
tags:
- verification
- claim-decay
- instruments
- design-principle
topic: tool-contracts
time_scope: invariant
---

# ADR-2026-09-14 — State the property, not the snapshot; and do not build a claim-checker

## Status

**Accepted.** Instance fixed (`tests/issue_clusters.rs:802-812`); mechanism **rejected on a
measurement**, which is the half this record exists to preserve.

Third in the family. `docs/adrs/2026-08-27-negative-results-name-their-scope.md` governs a
**zero**; `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` governs an
**instrument that answers a different question than it was asked**. This one governs the case
neither reaches: **the instrument was right, the answer was right, and the answer expired while
its credential survived.** Nothing was wrong at write time, which is why the two ADRs above do
not catch it.

## Context

`tests/issue_clusters.rs:802-808` justified a test's existence with a corpus snapshot — *zero bug
files carry a `cluster/` tag in flow style, so deleting the inline arm leaves the corpus-driven
check green* — carrying a date, a named method, and the words **"Verified by mutation, not
assumed."**

It was exact when written (`3be0088e`, 2026-09-01 20:26). The first flow-style tag landed
**38 hours later** (`07b7819c`, 2026-09-03 10:01). The claim did not merely go stale — its
conclusion **inverted**, from *"this mutation is invisible"* to *"this mutation reds the shared
gate"*. It was then copied near-verbatim into a session log and used to argue that a proposed
documentation gate would be decoration. Full instance and probe: `docs/trackers/claim-decay.md`
DC-6.

**The credential is the mechanism, not the staleness.** A bare assertion invites a check; a
stamped one forecloses it, and the stamp is the most quotable part of the sentence, hence the
part that survives copying. **Correct authoring practice is what made it dangerous** — which is
why "be careful" and "cite your measurements" are both null instruments against it.

## Decision

**A durable claim states a property, not a measurement, wherever a property will carry the
argument.** Where only a measurement will do, ship its derivation rather than its value —
already `CLAUDE.md` § *Observer Blindness* position 3.

**And codescout will not build a claim-checking mechanism.** No `re-derive: <command>`
annotation, no runner, no lint over dated claims.

## Alternatives considered

**1. Assert the premise (a test pinning the flow-style population at zero).** Rejected. It would
have redded on 2026-09-03 — and it forbids a **legitimate, deliberately supported** form:
`cluster_tags` reads both arms on purpose and `the_hook_script_agrees_on_both_yaml_tag_styles`
exists to prove it. A wall across a road the project paved. *This was the proposal that opened
the question; it is rejected by the analysis it asked for.*

**2. A `re-derive: <command>` annotation plus a runner.** Rejected on a **structural** ground,
with a measurement as corroboration — and the order matters, because an earlier draft of this
ADR had them the other way round.

*The structural ground, which is corpus-independent:* **the annotated set is biased away from
the defect population.** An opt-in annotation is opted into by the author, at write time, when
the claim is true and nothing suggests decay — so the author who annotates is the one who
already perceives their claim as decayable, and that author is not the one who ships a decayed
claim. The two populations are disjoint by construction. This holds at any uptake, which is why
it, and not the count, carries the rejection.

*The corroboration, and it is stronger than a forecast:* `CLAUDE.md` § *Observer Blindness*
position 3 **already mandates** the non-mechanised half of this proposal — ship the derivation
rather than the value. At `9045c56a` that mandate has **0** uptake across **77** dated `measured
YYYY-MM-DD` claims in **43** Rust files under `src/` and `tests/`. Predicting low uptake for a
new opt-in form would be a guess; observing zero uptake of an existing mandate is a **natural
experiment** on the same mechanism. Note precisely what it measures: a mandate carried in prose,
with **no trigger** — which is the shape `CLAUDE.md`'s rule has. *(Scope: a grep for the
reader-instruction form over `src/` and `tests/` for `*.rs`. A wider grep returns 97, but those
are runtime re-derivation in code — a different population. The one correct instance in the repo
is `scripts/pre-commit-ledger-counts.py:853`, outside that scope, and is the sibling of the
comment that decayed.)*

*What this does NOT reject, stated so the ADR cannot foreclose a thing it never evaluated:* a
**mandatory** annotation tied to a trigger that fires anyway is a different proposal. It would be
OB position 3's second-best shape rather than an opt-in, and the selection argument above does
not reach it — an unconditional annotation is not chosen per claim, so it cannot be chosen away
from the defect population. It is unevaluated here. It does inherit #4's objection: the trigger
would have to fire on all 77, many of which are fixed facts with no meaningful re-derivation
(*"CodeRankEmbed's measured 2048-token window"* cannot be cheaply re-derived), so the cost case
is not obviously winnable. That is an argument someone would have to make, not one this ADR has
made for them.

**3. Mutation-annotation parity (assert no two `Mutation that must kill this:` lines
contradict).** Rejected: **it would not have fired.** The contradiction here was between free
prose at `:805` and a structured annotation at `:1476`, not between two annotations. Recorded
because the population is real — 23 annotations across 10 files — so it reads like a good idea
until you check it against the case.

**4. A lint over bare dated values.** Rejected. Many of the 77 are legitimate fixed facts (an
API's token window). A campaign over a population whose members are mostly fine is
`OB-1`'s coverage-ratio trap.

## Consequences

- **now easier:** a reader meeting a justification comment gets a claim that cannot expire;
  a future session proposing a claim-checker finds the measurement instead of re-deriving it.
- **now harder:** nothing is enforced. Decay of this class stays caught by readers, and DC
  remains the evidence base rather than a gate. This is a deliberate, and revisitable, cost.

## Change scenarios absorbed

A corpus count is written into a justification and the corpus moves; a session proposes an
annotation-and-runner; a session proposes pinning a corpus population to protect prose.

## Revisit-when

**An instance where an author annotated a claim they did NOT perceive as decayable.** That is
the only evidence that bears on #2's rejection, because it is the only thing that would show the
selection bias failing. Or `measured-value-drift` reaches a DC instance that shipped a
**user-visible defect** rather than misleading an author.

**Explicitly NOT a trigger: the `re-derive` form gaining voluntary uptake.** An earlier version
of this section said exactly that — *"if uptake moves, the argument moves"* — and it was a
tripwire aimed at a signal that does not bear on the decision. Under the selection argument,
rising uptake would not rescue #2: the bias is in **which** claims get annotated, not how many.
Corrected 2026-09-14 on `fix-mask-keyword-fabrication`'s reading (sessionId `f0b1a4c7`), which
also supplied the natural-experiment framing above.

## Confidence

**High** throughout, after revision.

#1 forbids a supported form and #3 is falsified against the case it was invented for — both
settled against the code rather than by judgement.

**#2 was Medium until 2026-09-14 and is now High**, and the repair is this ADR's own law applied
to itself. The first draft rejected #2 on the 0/77 count — a measurement of today's uptake, i.e.
a **snapshot standing in for a property**, which is precisely the substitution this ADR exists
to reject. Leading instead with the selection argument, which is a property and cannot decay,
removes the self-contradiction and raises the confidence rather than lowering it.

**What is given up, since the argument now rests on a property rather than a count:** a
structural claim can be wrong in a way no measurement would reveal, and the clean numeric
tripwire is gone. That is the trade this ADR is about, taken deliberately on its own terms.
