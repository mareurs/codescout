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

Not implemented; the fix site is in another repository, so this record stays `open`. `src/librarian/tools/link_scan/resolve.rs` needs no change. Verified 2026-10-16 at checkout f5f1c6f7: in `resolve` (resolve.rs:320-451) the `EntryToken` arm and the qualified `CrossRepoToken` arm both ask `index.definers(token)` for the defining artifact. `DefinitionIndex::build` fills that from `## PREFIX-N` headings in the indexed body, and nothing reads git history. A digest that drops the headings leaves zero definers, so a token of a known prefix resolves to `Outcome::Dangling` (resolve.rs, `0 =>` branch). The defect is only the skill text.

**Where.** `/home/marius/work/claude/claude-plugins/codescout-companion/skills/tracker-hygiene/SKILL.md`, D10 step 4 (line 244 of that file at 2026-10-16; the same text also sits in the serving copy under `~/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/` and in the `claude-plugins-effort-steering-*` checkouts). The file's other uses of the word "heading" (lines 193, 392-404) concern D11 and the ledger template, not compaction, so nothing in the skill preserves entry headings.

**Proposed replacement for step 4** (the owner applies it; this session did not edit the plugin repo because its tree is dirty on branch `effort-routing-r1`):

```text
4. **Compact.** Never reduce an F-N / W-N entry to a bare row: every entry keeps
   its `## <ID> — <title>` heading in the archived file, because `link_scan` binds
   PREFIX-N to that heading and to nothing else, and git history is not consulted.
   Prose beneath a heading may be shortened. Add the outcomes digest (the Index /
   Wins Index tables with updated statuses, one paragraph per promoted/rehomed
   entry naming its destination, and unfired Promote-when criteria) as an
   ADDITIONAL reading surface at the top. Verify with `link_scan`: no new dangling
   citation for the stem's tokens.
```

After the plugin edit, bump the plugin version and sync the serving copies.

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
