---
id: d2cd00fc837e53f2
kind: tracker
status: draft
title: Goal-Tracker × Audit-Doc-Refs Cross-Pollination
owners: []
tags:
- goal
- cross-pollination
- dogfood
topic: null
time_scope: null
---

## Why this goal exists

The 2026-05-17 review session uncovered ~38 findings across the
goal-tracker archetype implementation and its boundary with
`audit_doc_refs`. The fixes span code, spec, and prose. Rather than
shipping them as a single mega-PR or scattering them across forgotten
TODO comments, this goal-tracker aggregates the work so it survives
across sessions and resolves cleanly.

Eating the dogfood: this is literally the goal-tracker's textbook use
case. Every friction we hit while using it logs a Tier-3 eval data
point the spec was missing.

## Acceptance criteria (prose)

This goal closes when all four acceptance_signals are met:

1. **No open findings.** The hamsa/lion/yak audit (C-1) reaches zero
   `status: open` issues. 11 open issues at goal creation; 27 already
   marked fixed by amendment + plan landing.

2. **All ADRs decided.** The 5 ADRs from the deep-pass review (C-2)
   carry `status: decided`. This signal is met at goal creation —
   decisions landed in the same session that surfaced them.

3. **All I1 refactor tasks complete.** The plan's 14 tasks (C-3) reach
   `status: done`. T-14 (Hamsa S-1+S-2) shipped 2026-05-17. T-1 through
   T-13 are pending — they cover Phases 1-4 of the I1 refactor.

4. **Dogfood friction logged.** Manual check — the friction-log file
   captures observations during use (planned at first refresh; see
   below).

Out of scope for this goal:
- Tier 3 eval gate (open-issue #7 / C-4) — separate effort with its
  own deliverables.
- Stop hook → Rust CLI subcommand (Q4 / I9, open-issue #11) — separate
  refactor; tracked but not part of this goal's closure conditions.
- `TrackerMerger` trait extraction (ADR-1) — explicitly rejected;
  revisit-when conditions documented in C-2.

## Decomposition rationale

Three children chosen to match three rhythm patterns the work has:

- **C-1 (audit_issues)** — append-and-flip flow. Findings arrive as
  discoveries; statuses flip to `fixed` as code lands. Severity-graded.
- **C-2 (reflective)** — decision-record flow. ADRs don't change after
  archive. Each carries a revisit-when condition; the tracker exists
  to make those triggers visible.
- **C-3 (task_list)** — sequential checkpoint flow. Mirrors plan task
  status; concrete steps with completion criteria.

One mega-tracker would collapse these into noise. Three children let
each rhythm evolve at its own cadence.

The combination is also a deliberate exercise of the goal-tracker
across all three relevant child archetypes — when the refresh prompt
hits an `audit_issues` + `reflective` + `task_list` aggregation, we
exercise three of the six per-archetype reconciliation clauses in one
pass. That gives the Tier-3 eval coverage the design lacked.

## History

### 2026-05-17 — Goal created, dogfood starts

L1 umbrella created. Children C-1, C-2, C-3 linked.
Initial state:
- C-1 (audit): 11 open / 27 fixed of 38 findings.
- C-2 (ADRs): 5 ADRs, all `decided`. Already signal-met.
- C-3 (tasks): 1 done (T-14), 13 pending.
- Plan: `docs/superpowers/plans/2026-05-17-i1-refactor.md`.
- Amendment: `docs/superpowers/specs/2026-05-17-goal-tracker-amendment.md`.
- Companion-repo commit: `0b75991` (Stop hook S-1+S-2).

Next: first refresh + friction log creation.

