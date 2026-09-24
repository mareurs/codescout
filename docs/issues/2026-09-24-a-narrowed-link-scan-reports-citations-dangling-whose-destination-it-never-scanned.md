---
id: '82b69559ddb18983'
kind: bug
status: open
title: 'BUG: a narrowed link_scan reports a citation dangling when its destination is merely outside the window'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related: []
severity: low
---

## Summary

A `link_scan` narrowed with `limit` resolves artifact-id citations only against the artifacts
it scanned, so a citation whose destination lies outside the window is reported **dangling**
even though the catalog resolves it. The sibling defect — pruning edges whose destination was
never scanned — was fixed at `ea151a39`
(`docs/issues/archive/2026-09-21-a-narrowed-link-scan-prunes-edges-whose-destination-it-never-looked-at.md`),
whose § Evidence records this symptom too; the fix bounded the prune and left the dangling
report as it was.

## Symptom (Effect)

Measured 2026-09-24 on the rebuilt binary:
`librarian(action="link_scan", scope="project", write=false, limit=10)` reports

```
"dangling": [{"src_id": "b161f5ed9b7bfbd9", "raw": "33e740960f758b6d", "kind": "ArtifactId", "line": 27}]
```

while the catalog holds `33e740960f758b6d` (status `fixed`,
`docs/issues/archive/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md`, frontmatter
id matching), and the unlimited scan on the same tree resolves the same citation into an
`edges_missing` row `b161f5ed9b7bfbd9 -> 33e740960f758b6d`. The response does carry
`scan_truncated: true`, but `dangling` is presented as a finding with no scope qualifier, so a
reader acting on it "repairs" a live citation.

## Reproduction

1. `librarian(action="link_scan", scope="project", write=false, limit=10)` on a tree where one of
   the first 10 artifacts cites (by 16-hex id) an artifact outside that window.
2. The citation appears in `dangling`; the same call without `limit` shows it resolved.

## Root cause

Not read at the code level in this filing. Inferred from the sibling: resolution uses the
scanned corpus's id set, the same set `ea151a39` stopped using as the prune bound.

## Fix

Not attempted. Resolve artifact-id citations against the catalog (as the prune's destination
bound now effectively does), or suppress / scope-label `dangling` for destinations outside a
truncated window.

## Tests added

None.

## References

- `docs/issues/archive/2026-09-21-a-narrowed-link-scan-prunes-edges-whose-destination-it-never-looked-at.md`
  — the sibling, fixed for pruning; its § Evidence documents this half
