---
status: open
opened: 2026-05-18
closed:
severity: low
owner: marius
related: [src/librarian/classify/]
tags: [librarian, classification, kind]
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

Three viable approaches, ordered by ergonomics:

1. **Add `kind: bug` to `docs/issues/_TEMPLATE.md`** and run a one-pass
   migration that flips existing bug-file frontmatter to `kind: bug`. The
   librarian classifier then has a typed source of truth. Smallest behavior
   change.
2. **Path-classifier rule:** any markdown under `docs/issues/` gets
   `kind: bug` regardless of frontmatter. Stronger guarantee, but
   convention-coupled.
3. **No change** — accept that the canonical query needs a `rel_path` filter
   and document it (already done in CLAUDE.md commit `136f4c48`).

Recommendation: (1) — frontmatter is the source of truth elsewhere in the
librarian; bug files should follow.

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
