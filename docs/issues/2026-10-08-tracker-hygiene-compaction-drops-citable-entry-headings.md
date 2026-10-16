---
id: d0310f7b0f499c83
kind: bug
status: open
title: 'BUG: tracker hygiene''s retirement recipe drops entry definitions required by citation resolution'
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

# BUG: tracker hygiene's retirement recipe drops entry definitions required by citation resolution

## Summary
The serving tracker-hygiene skill's D10 step 4 replaces a session log with tables and an outcomes digest, keeping full prose only in git. Tracker conventions require archived entries to keep their defining headings. The recipe does not require those headings in its digest.

## Symptom (Effect)
The installed retirement recipe permits the removal of definitions the citation resolver needs.

## Reproduction
At checkout 21237e7f2734d8757c6b9666e2f79a9cfd6a318c:
1. Read D10 step 4 in the serving tracker-hygiene skill.
2. Compare `get_guide("tracker-conventions")`, Compaction and archival.
3. Read `resolve` and `tests/a_qualified_citation_to_a_file_without_that_entry_dangles` in `src/librarian/tools/link_scan/resolve.rs`.
The documentation conflict was observed. This session did not perform a destructive retirement or measure a new runtime survivor.

## Environment
Linux, codescout experiments; serving codescout-companion 1.20.16; observed 2026-10-08.
Serving skill:
```text
/home/marius/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/1.20.16/skills/tracker-hygiene/SKILL.md
```

## Root cause
D10 specifies table/digest replacement without preserving definition headings. The guide says those headings must survive; `resolve` looks for an entry's defining artifact. The citation consequence is inferred from the implementation and its existing regression test, not newly measured by applying D10.

## Evidence
D10 step 4:
```text
Replace the body with an outcomes digest: the Index / Wins Index
tables (statuses updated), one paragraph per promoted/rehomed entry naming its
destination, and unfired Promote-when criteria. Full prose history stays in git.
```
Tracker conventions, Compaction and archival:
```text
live body → archived section (heading kept) → nothing further
Never reduce an entry to a bare row to "compact" it.
```

## Hypotheses tried
1. Archived status alone might preserve entry definitions. Rejected by code inspection: resolution needs definitions in the indexed artifact body; git history is not consulted by this resolver.

## Fix
Not implemented. Preserve defining headings and evidence in the archive, and treat an outcomes digest as an additional reading surface. The fix belongs in the companion source and serving copies if this recipe is revised. No fix SHA or patch-id exists.

## Tests added
None: documentation conflict logged during exploration. Existing resolver test inspected; no new failure was executed.

## Workarounds
Preserve complete entry sections during retirement. Resolve the separate file-stem change when repairing qualified citations.

## Resume
Reconcile D10's compact step with the heading-preservation contract; verify the eventual recipe with an actual retirement fixture and citation resolution before applying it to a live tracker.

## References
- `get_guide("tracker-conventions")`, Compaction and archival.
- [Citation resolver](../../src/librarian/tools/link_scan/resolve.rs)
- [Archive policy](../trackers/archive-cadence-policy.md)
