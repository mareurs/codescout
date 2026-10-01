---
id: 044f65842ed538c1
kind: bug
status: fixed
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

Read at the code level, 2026-10-01. In `link_scan::call`, `rows.truncate(limit)` ran before extraction, so the `DefinitionIndex` (which resolves entry tokens) and the `Corpus` (artifact ids, rel paths, file stems) were both built from the window. A citation whose destination or definer lay outside it found nothing, and resolved to `Dangling`. `ea151a39` bounded the prune's destination by that same corpus and left the corpus alone.

**Measured on this tree before the fix**, `limit=10` against the full scan (`write=false`): 89 of the 100 reported `dangling` citations resolve unambiguously in the full scan, and 11 are dangling in both. Of the 89, 68 are entry tokens and 21 are artifact ids. This file named only the ids, so it understated the defect by more than three to one.

The same window has a second failure this file did not list. A token ambiguous across the corpus resolves to its one in-window definer instead. The narrowed fixture reported `F-2` (two definers) as a derived edge where the full scan reports it ambiguous, and `write=true` records such an edge.

## Fix

Shipped in `a18d0da947c073695f31370ac008c1548c5d63a3`. `limit` now bounds the artifacts **walked**. Every artifact in scope is fetched and parsed (population cap `max(limit, 10000)`), and only the first `source_count` are walked, counted as scanned and used as the prune's sources. `scan_truncated` still means the window is narrower than the population.

The repair widens the corpus that `ea151a39` bounds the prune's destination by, so two source bounds had to stay the window: `prunable` for `cites` edges and `scanned_slugs` for the entry-grain prune under `write=true`. With the population parsed, an unwalked source has no desired edges, and bounding either by every extract would delete its edges. Both are pinned by tests.

The advertised `limit` description changed from "scanned" to "walked", one character shorter. The tool surface sits at its budget with no slack, and a longer wording failed `tool_surface_under_budget` by 147 characters; the full semantics are in the Rust doc comment on `Args::limit`.

**Not verified live:** the running MCP server predates this change. Re-run `link_scan(limit=10, write=false)` after a rebuild; `dangling` should fall from 100 toward the 11 that are dangling in both scans.

## Tests added

Five tests in `src/librarian/tools/link_scan/mod.rs`, all in the default lane (the lean lane never compiles librarian code):

- `a_narrowed_scan_resolves_citations_whose_destination_is_outside_the_window` and `a_narrowed_scan_keeps_a_token_ambiguous_when_one_definer_is_outside_the_window`, red before the fix.
- `a_narrowed_scan_reports_only_the_sources_it_walked`, so "resolve against everything" cannot be implemented as "walk everything".
- `a_narrowed_scan_never_reports_stale_the_edges_of_a_source_it_did_not_walk` and `a_narrowed_write_scan_leaves_the_entry_rows_of_a_source_it_did_not_walk`, each with a full-scan control, since "kept" is also what a scan that prunes nothing reports.

**Mutation:** 8 sites. Six were killed on the first run. The prune's source bound and the entry-grain `scanned_slugs` SURVIVED until the last two tests existed, then were killed. The fixture measures what the tests assert, not a stand-in: the window is `src` alone, with `F-1`, the live id and one of `F-2`'s two definers outside it.

## Fix provenance

- **SHA:** `a18d0da947c073695f31370ac008c1548c5d63a3` (`experiments`)
- **patch-id:** `848a1973a47fab70bbf4fe1105a937e468cbbd29`

## References

- `docs/issues/archive/2026-09-21-a-narrowed-link-scan-prunes-edges-whose-destination-it-never-looked-at.md`
  — the sibling, fixed for pruning; its § Evidence documents this half
