---
id: '75ac6fa82b7d4f82'
kind: bug
status: open
title: 'BUG: TAXONOMY archive cadence contradicts the amended archive policy'
owners:
- marius
tags:
- cluster/unclassified
- tracker-archiving
- documentation
closed: null
opened: 2026-10-08
owner: marius
related: []
severity: medium
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
Not implemented. Reconcile the live recipes after choosing the archiving design. No fix SHA or patch-id exists.

## Tests added
None: this session records the documentation conflict during exploration.

## Workarounds
Read the amended policy before choosing a destination; reuse the existing companion.

## Resume
Compare the taxonomy's Archive cadence section with the amended policy and reconcile their entry-level destination rules.

## References
- [Taxonomy](../TAXONOMY.md)
- [Archive cadence policy](../trackers/archive-cadence-policy.md)
