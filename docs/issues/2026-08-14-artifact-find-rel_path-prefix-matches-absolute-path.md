---
status: open
opened: 2026-08-14
closed:
severity: medium
owner: marius
related: []
tags: [librarian, artifact, filter]
kind: bug
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
Not yet planned. Two candidate directions: (a) store/also-index a true
repo-relative column and match `rel_path` filters against that, or (b) if the
absolute-path match is intentional, fix the guide text (`get_guide("librarian")`
Filter Syntax example) to stop advertising a repo-relative `prefix` example
that doesn't work.

## Tests added
N/A — not yet fixed.

## Workarounds
Use `contains` with a distinctive fragment of the path (e.g.
`{"rel_path": {"contains": "docs/issues"}}`) instead of `prefix` with a
repo-relative fragment. Never trust a `count: 0` from a `rel_path` `prefix`
filter as proof nothing is catalogued under that directory.

## Resume
Locate the SQL/filter-compilation code for `rel_path` (`src/librarian/filter.rs`
per the `catalog-sql-hazards` memory's LIKE-escaping precedent) and confirm
whether the column backing `rel_path` filters is the stored absolute path or a
derived relative one. If absolute, either add a relative column + index, or
fix the two conflicting doc surfaces (`get_guide("librarian")` Filter Syntax
section and this file) to match runtime behavior.

## References
- Sibling-repo finding: `Mercury BOM` `docs/trackers/conversations-tracker-session-log.md` F-1 (2026-06-09)
- `get_guide("librarian")` § Filter Syntax (the doc example this contradicts)
