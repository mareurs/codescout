---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
- librarian
- artifact
- filter
claimed_at: 2026-09-11
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
closed: null
opened: 2026-08-14
owner: marius
related: []
severity: medium
---

# BUG: `artifact(find)` `rel_path` filter matches the stored absolute path, not the repo-relative path the docs promise

## Summary
`artifact(action="find")`'s own guide text says `rel_path` in find results is
"path relative to repo root" and that filtering with `{"rel_path":{"prefix":"docs"}}`
should find artifacts under `docs/`. In practice the filter runs against the
catalog's stored **absolute, forward-slash** path, so a repo-relative `prefix`
silently returns zero rows. `contains` with a distinctive fragment works;
`prefix` with a repo-relative fragment does not.

## Symptom (Effect)
```
artifact(action="find", filter={"rel_path": {"prefix": "docs"}})
→ {"count": 0, "items": []}

artifact(action="find", filter={"rel_path": {"contains": "docs/issues"}})
→ {"count": 3, "items": [...]}   # real matches, same project
```
No error, no warning — a legitimate-looking query returns an empty result
set that reads as "nothing catalogued under docs/".

## Reproduction
Reproduced live on `codescout` itself, HEAD `b9a67d1dc1f0a3458c35ac6f047d8f3d640fd329`,
branch `feat/local-onnx-embedder`, project already indexed:

1. `artifact(action="find", scope="project", filter={"rel_path": {"prefix": "docs"}})` → 0 results.
2. `artifact(action="find", scope="project", filter={"rel_path": {"contains": "docs/issues"}})` → 3 results, confirming the project *is* catalogued under `docs/`.

Independently discovered and documented 2026-06-09 in the sibling project
`Mercury BOM` (`docs/trackers/conversations-tracker-session-log.md` F-1), with
the same repro shape: `{rel_path:{prefix:"docs"}}` → 0; `{rel_path:{contains:"docs/conversations"}}` → 2;
`{rel_path:{contains:"C:/"}}` → all rows (confirming the stored value is the
full absolute path, e.g. `//?/C:/Users/.../Mercury BOM/docs/...`).

## Environment
Windows 11, codescout MCP server, `feat/local-onnx-embedder` branch. Not
confirmed whether this is Windows-path-specific (verbatim `\\?\` + absolute
storage) or also reproduces on POSIX-style abs paths — both repro instances
above are Windows.

## Root cause
Unknown — see Hypotheses tried. `get_guide("librarian")`'s own Filter Syntax
section documents `{"rel_path": {"prefix": "docs/trackers"}}` as a working
example, which is the exact shape that fails here — the guide and the runtime
disagree with each other, not just with user expectation.

## Evidence
### codescout self-repro (2026-08-14)
```
filter={"rel_path": {"prefix": "docs"}} → count: 0
filter={"rel_path": {"contains": "docs/issues"}} → count: 3, abs_path e.g.
  "docs/issues/2026-08-07-artifact-move-cannot-resolve-source-in-subroot-workspace.md"
```
Note `abs_path` in the *response* is shown repo-relative — the mismatch is
specifically in what the `rel_path` filter matches against internally, not in
what's displayed back.

### Mercury BOM session log (2026-06-09)
`docs/trackers/conversations-tracker-session-log.md` F-1 (external repo,
cited here for cross-reference only — not re-quoted verbatim).

## Hypotheses tried
1. **Hypothesis:** `rel_path` filter is matched against the catalog's stored
   absolute path (verbatim `\\?\C:\...` form converted to forward slashes),
   not a repo-relative column.
   **Test:** `{"rel_path":{"contains":"C:/"}}` in the Mercury BOM session
   returned all 16 catalogued rows.
   **Verdict:** confirmed (by the external repro; not independently
   re-run here — see Resume).
   **Evidence link:** Mercury BOM Evidence section above.

## Fix

Fixed by duplicate. This bug (filed 2026-08-14 against the then-current `artifact(find)` tool name) and `docs/issues/archive/2026-09-04-rel-path-filter-is-an-alias-onto-an-absolute-column.md` (id `d0a4d6e530048d6a`, filed 2026-09-04) describe the identical defect — `compile_leaf` (`src/librarian/filter.rs`) remapped the `rel_path` field NAME to the `abs_path` column but left the bound VALUE alone, so a repo-relative argument was compared against an absolute stored path. The later filing was never cross-referenced against this one, fixed and archived on its own, and this one went zombie-open.

No code work remains: `5253297a4583d77fb254dcae26808d924741ccb0` anchors every op (`contains`/`prefix`/`eq`/`ne`/`in`/`nin`) at a `/` boundary against `abs_path` and refuses `gt`/`lt`/`gte`/`lte` outright as not meaningful on an aliased column. Mutation discipline already applied there per its own commit message ("Watched RED first: left [] right [t1, t2]"), with a real-query, real-rows regression test per op — re-deriving that here would be redundant, not confirmatory.

**Live-reverified on this checkout, 2026-09-11**, rather than taken on the strength of the commit message alone: `doc(action="find", filter={"rel_path":{"prefix":"docs"}})` now returns real matches under `docs/` (50, capped), where the bug's own reproduction recorded `count: 0`.

**SHA:** `5253297a4583d77fb254dcae26808d924741ccb0`
**patch-id:** `66b4bcda49601dfee601c6a458d438f5d4891159`

Worth naming as its own small lesson: a keyword search before filing (`doc(find, semantic=...)` or a title-fragment `contains` filter) would have surfaced this bug already open and let the later session fix the SAME file instead of a sibling one — the tracker verify-open cadence catches a fix that outlives its own tracker entry, but nothing currently catches two open entries for one defect at file time.
## Tests added

None added here — see `docs/issues/archive/2026-09-04-rel-path-filter-is-an-alias-onto-an-absolute-column.md` for the regression suite that covers this defect (all six affected ops, real queries against real rows, both directions).
## Workarounds
Use `contains` with a distinctive fragment of the path (e.g.
`{"rel_path": {"contains": "docs/issues"}}`) instead of `prefix` with a
repo-relative fragment. Never trust a `count: 0` from a `rel_path` `prefix`
filter as proof nothing is catalogued under that directory.

## Resume

Done — see § Fix. Closed as fixed-by-duplicate; nothing left to resume.
## References
- Sibling-repo finding: `Mercury BOM` `docs/trackers/conversations-tracker-session-log.md` F-1 (2026-06-09)
- `get_guide("librarian")` § Filter Syntax (the doc example this contradicts)
