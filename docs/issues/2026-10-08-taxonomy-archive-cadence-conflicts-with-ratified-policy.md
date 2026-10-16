---
id: '75ac6fa82b7d4f82'
kind: bug
status: fixed
title: 'BUG: TAXONOMY archive cadence contradicts the amended archive policy'
owners:
- marius
tags:
- cluster/unclassified
- tracker-archiving
- documentation
closed: 2026-10-16
opened: 2026-10-08
owner: marius
related: []
severity: medium
unverified: 'No regression test: a documentation reconciliation has nothing executable to pin; verified by re-reading the section against the ratified policy bytes.'
---

# BUG: TAXONOMY archive cadence contradicts the amended archive policy

## Summary
The taxonomy prescribes quarterly files for closed tracker entries. The linked policy was amended to require one stable, repeatedly appended companion for entry archival, with a dated destination reserved for whole-file retirement.

## Symptom (Effect)
Two live instruction surfaces prescribe different destinations for the same operation.

## Reproduction
At checkout 21237e7f2734d8757c6b9666e2f79a9cfd6a318c:
1. Read `docs/TAXONOMY.md` under Archive cadence.
2. Read the archive-policy artifact's Ratified and Archive destination sections through `doc(get)`.
3. Compare entry-level destinations.

## Environment
codescout, Linux, experiments; observed 2026-10-08 through the connected MCP.

## Root cause
The taxonomy still contains the earlier time-partitioned recipe; the ratified amendment distinguishes entry archival from whole-file archival. This is a measured text disagreement, not a measured claim that a recent archive was misfiled.

## Evidence
Taxonomy, line 411:
```text
Closed entries graduate to docs/trackers/archive/<tracker>-<YYYY>-q<n>.md
```
Amended policy, operative destination:
```text
Entry-level (companion file, appended repeatedly)
docs/trackers/archive/<ledger>-archived-entries.md — stable, no timestamp
```

## Hypotheses tried
1. The quarterly recipe might still be the ratified policy. Rejected: the ratified amendment explicitly specifies a stable companion.

## Fix

Implemented on `experiments`-based worktree branch `worktree-agent-a920cce913249b4dc`, commit `1e5341352431276dfa8da4c7b88d68309be122b3` (branch: worktree-agent-a920cce913249b4dc; patch-id `7c6ec1842bb2fd04caa706074fe9cd225dd17ced`). The SHA is positional; the patch-id survives rebase and cherry-pick.

Which side is ratified, verified in `docs/trackers/archive-cadence-policy.md`: the 2026-08-16 "Ratified" table (surface 2 = ii + iv, surface 3 = a narrowed 2026-08-17) and the "Amendment — ratified 2026-08-17" operative-form table. The `<tracker>-<YYYY>-q<n>.md` destination came from the superseded 2026-05-24 "Decision" section, so the taxonomy had drifted; no open policy decision remained. `docs/TAXONOMY.md` § Archive cadence now carries: entry-level destination `<ledger>-archived-entries.md` (stable, no timestamp, check before creating), whole-file `<name>-<YYYY-MM-DD>.md` with the three-step repoint, the keep-the-heading rule, and the surface-2 trigger (closed, or promote-or-die). The same section also dropped the "SHA is on master" eligibility clause, which belonged to the overturned ii+iii draft.

## Tests added

None: documentation reconciliation. Grep over `*.rs`, `*.py`, `*.sh` found nothing that pins the old `-q<n>` text.

## Workarounds
Read the amended policy before choosing a destination; reuse the existing companion.

## Resume
Compare the taxonomy's Archive cadence section with the amended policy and reconcile their entry-level destination rules.

## References
- [Taxonomy](../TAXONOMY.md)
- [Archive cadence policy](../trackers/archive-cadence-policy.md)
