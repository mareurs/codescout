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

**2. A `re-derive: <command>` annotation plus a runner.** Rejected **on uptake, measured, not on
taste.** At `9045c56a`: **77** dated `measured YYYY-MM-DD` claims across **43** Rust files in
`src/` and `tests/`; the reader-facing re-derive form appears in **zero** of them. The rule
mandating it already exists in `CLAUDE.md` and has ~0 uptake on this surface. An opt-in
annotation is opted into *by the author, at write time, when the claim is true and there is no
reason to expect decay* — the exact party the class blinds. Machinery atop a step nobody takes.
*(Scope: a grep for the reader-instruction form over `src/` and `tests/` for `*.rs`. A wider grep
returns 97, but those are runtime re-derivation in code — a different population. The one correct
instance in the repo is `scripts/pre-commit-ledger-counts.py:853`, outside that scope, and is the
sibling of the comment that decayed.)*

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

The `re-derive` form reaches non-trivial voluntary uptake on its own (the 0/77 is the whole
basis for rejecting #2 — if it moves, the argument moves); or `measured-value-drift` reaches a
DC instance that shipped a **user-visible defect** rather than misleading an author.

## Confidence

**High** on stating properties and on rejecting #1 and #3 — #1 forbids a supported form and #3
is falsified against the case it was invented for. **Medium** on rejecting #2: the 0/77 is a
measurement of *today's* uptake, which is itself a snapshot — and a snapshot standing in for a
property is this ADR's own subject. The Revisit-when above is that admission made operational.
