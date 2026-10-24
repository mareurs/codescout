---
id: d0310f7b0f499c83
kind: bug
status: fixed
title: 'BUG: tracker hygiene''s retirement recipe drops entry definitions required by citation resolution'
owners:
- marius
tags:
- cluster/unclassified
- tracker-archiving
- documentation
closed: 2026-10-24
opened: 2026-10-08
owner: marius
related: []
severity: medium
unverified: The issue's own Resume verification (a throwaway retirement fixture run through `link_scan`) was not run. A skill text cannot be unit-tested here; the check is that the instruction exists and that cached copies at version 1.20.19 carry it (older cached versions 1.20.14 to 1.20.18 do not), and which version each profile's `installed_plugins.json` selects was not checked.
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

Fixed in the sibling repo, not in this one. `claude-plugins` commit `e0532f08` ("fix(tracker-hygiene): D10 compaction keeps every entry's heading so link_scan citations still resolve") changes `codescout-companion/skills/tracker-hygiene/SKILL.md` step 4, which now reads "Never reduce an F-N / W-N entry to a bare row: every entry keeps ..." the `## <ID> — <title>` heading and is verified with `link_scan`. Confirmed in this session: the commit is on `claude-plugins` branches `main`, `effort-routing-r1` and `fix/codescout-issues-20261016`, and the text is at `SKILL.md:244` on `main`.

- Fix commit (cross-repo, branch `main` of `claude-plugins`): `claude-plugins:e0532f0877fcf7e8c12bf69636e3ef927ed52130`
- patch-id: `e21f4bb868f412e6a880d41db750bae556ad864c`

## Tests added
None: documentation conflict logged during exploration. Existing resolver test inspected; no new failure was executed.

## Workarounds
Preserve complete entry sections during retirement. Resolve the separate file-stem change when repairing qualified citations.

## Resume

User action: apply the step 4 text above to the companion SKILL.md (and its serving copies), then verify with a retirement fixture: compact a throwaway session log per the new step 4, run `link_scan`, and confirm its F-N / W-N tokens still resolve. Flip this record to `fixed` once the plugin commit exists, recording its SHA and `git patch-id --stable`.

## References
- `get_guide("tracker-conventions")`, Compaction and archival.
- [Citation resolver](../../src/librarian/tools/link_scan/resolve.rs)
- [Archive policy](../trackers/archive-cadence-policy.md)
