---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: low
owner: marius
related: [src/librarian/classify/]
tags: [librarian, classification, kind]
kind: bug
---

# BUG: librarian classifies `docs/issues/*.md` (bug files) as `kind: tracker`

## Summary

The librarian's markdown classifier assigns `kind: tracker` to every bug file
in `docs/issues/*.md`, even though their lifecycle (status enum
`open|investigating|fixed|mitigated|wontfix`) and purpose (per-bug investigation
log) are distinct from tracker artifacts (status enum `active|draft|archived`,
purpose: living dashboards / session logs). Result: `artifact(find, kind=tracker)`
without a path filter returns 68 mixed bug + tracker rows.

## Symptom (Effect)

```
artifact(action="find", kind="tracker", limit=100)
→ count: 68
  items: [
    {kind: tracker, status: fixed, abs_path: docs/issues/...},     ← bug
    {kind: tracker, status: active, abs_path: docs/trackers/...},  ← tracker
    {kind: tracker, status: open, abs_path: docs/issues/...},      ← bug
    ...
  ]
```

Confirmed in session 2026-05-18 after backfilling `docs/trackers/` with
librarian frontmatter and running `librarian(reindex)`.

## Reproduction

1. Branch `experiments` at HEAD `136f4c48`.
2. `librarian(action="reindex", scope="project")`.
3. `artifact(action="find", kind="tracker", limit=100)`.
4. Observe: items include both `docs/issues/*.md` and `docs/trackers/*.md`.

## Environment

- codescout MCP server, release build.
- 26 bug files in `docs/issues/` (mix of `open` / `fixed` / `wontfix` /
  `mitigated`) all return `kind: tracker`.

## Root cause

`Unknown — best lead:` the markdown classifier in `src/librarian/classify/`
(or wherever the kind-from-path heuristic lives) treats any markdown under
`docs/` without an explicit `kind:` frontmatter as a tracker. Bug files don't
carry `kind:` in their frontmatter (`docs/issues/_TEMPLATE.md` ships only
`status` + `opened` + `closed` + `severity` + `owner` + `related` + `tags`).

## Evidence

- `docs/issues/_TEMPLATE.md` has no `kind:` field. Every bug file inherits
  this template, so none of them declare a kind.
- The current canonical query workaround (CLAUDE.md `## Session Intelligence
  Trackers → Querying active trackers`) requires `rel_path: {contains:
  docs/trackers/}` to disambiguate. Without it, the query is polluted.
- Status distribution from `artifact(find, kind=tracker, limit=100)` in the
  session: 16 active, 8 done (now archived), 13 draft, 24 fixed, 1
  investigating, 2 open, 1 template, 3 wontfix. The four "fixed/wontfix/
  open/investigating" values are bug-file vocab — confirms bug files are
  in the result set.

## Hypotheses tried

1. **Hypothesis:** Path filter `docs/trackers/` suffices as a workaround.
   **Test:** Ran `artifact(find, kind=tracker, rel_path contains docs/trackers/)`.
   **Verdict:** confirmed — 24 clean results.
   **Evidence link:** session commit `136f4c48`.

## Fix


Approach (1) shipped — frontmatter-driven classification.

**Steps:**
1. Added `kind: bug` to `docs/issues/_TEMPLATE.md` frontmatter.
2. Migrated 35 existing bug files via python script that appends `kind: bug`
   to frontmatter blocks lacking it. One file (`2026-04-16-mcp-cancel-disconnect.md`)
   had a stale `kind: null` from an older librarian auto-write, patched
   separately via `edit_markdown(frontmatter={set:{kind:"bug"}})`. Total: 37 files.
3. Ran `librarian(action="reindex", scope="project")` — 35 updated rows.
4. Verified:
   - `artifact(find, kind="tracker")` → 32 rows, **zero from `docs/issues/`**
     (previously 56, with 32 mislabeled bugs)
   - `artifact(find, kind="bug")` → 37 rows, all from `docs/issues/`
5. Simplified canonical tracker query in `CLAUDE.md` — dropped the
   `rel_path: docs/trackers/` workaround filter; bare `kind="tracker"` now suffices.

**Indexer behavior (scouted, no code change required):**
The classifier already supports frontmatter-driven override at
`src/librarian/indexer.rs:102-105`:
```rust
let kind = fm.and_then(|f| f.kind.clone())               // frontmatter wins
    .or_else(|| rule_match.as_ref().map(|r| r.kind.clone()))  // rule fallback
```
The default rule at `src/librarian/classify.rs:90-93`
(`docs/issues/**/*.md → kind=tracker`) was kept as defense-in-depth for any
future bug file that omits the field.

**Commit:** `<tba>` on `experiments`.

## Tests added

`N/A — bug only filed.` Future fix should add:
- `bug_files_classified_as_kind_bug_after_template_update`
- `find_kind_tracker_excludes_bug_files_after_migration`

## Workarounds

Use the path-scoped query documented in `CLAUDE.md`:

```
artifact(action="find",
         filter={"and": [{"kind": {"eq": "tracker"}},
                         {"rel_path": {"contains": "docs/trackers/"}}]})
```

Or, for bugs:

```
artifact(action="find",
         filter={"and": [{"kind": {"eq": "tracker"}},
                         {"rel_path": {"contains": "docs/issues/"}}]})
```

## Resume

Pick approach. If (1): edit `docs/issues/_TEMPLATE.md` to include
`kind: bug` in the frontmatter block; write a one-pass migration that
sets `kind: bug` on every `docs/issues/*.md` (incl. `archive/`); run
`librarian(reindex)`; verify `artifact(find, kind=tracker)` no longer
returns bug rows. Update CLAUDE.md `## Bug Tracking` section to document
the `kind:` requirement. Drop the `rel_path` filter from the canonical
tracker query in CLAUDE.md once the classification is clean.

## References

- Workaround query landed in `CLAUDE.md` via commit `136f4c48`.
- 24-vs-68 count discrepancy observed in session 2026-05-18.
- Related: librarian classification heuristic in `src/librarian/classify/`
  (path to be confirmed during fix).
