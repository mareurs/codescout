---
status: open
opened: 2026-08-23
closed:
severity: low
owner: marius
related: []
tags: [librarian, trackers, docs-drift]
kind: bug
unverified: 'Root cause not established — catalog-rebuild loss vs. never-attached augmentation is undetermined; no git history for the catalog exists to distinguish them.'
---

# BUG: Research Index tracker documents a [LIVE] table and a params refresh, but has no augmentation

## Summary

`docs/research/README.md` (artifact `5086e3c7c0b9d83c`, kind `tracker`) tells every
agent that its entry table is "rendered automatically … by the librarian augmentation
refresh", and its § *How to save a research* step 4 prescribes an
`artifact(action="update", patch={params: {...}}, commit_refresh=true)` call. The
catalog holds **no augmentation** for that artifact, so there is no `render_template`
to project and no `params` collection to write into. Every agent that saves a
research file and follows the documented procedure will either no-op or error at
step 4.

## Symptom (Effect)

`artifact(action="get", id="5086e3c7c0b9d83c", full=true)` returns:

```
"augmentation": null,
```

while the body of the same artifact states:

```
This folder catalogs every research artifact in the codescout repo. The
[LIVE] table above is rendered automatically from each file's frontmatter
by the librarian augmentation refresh — do not edit it by hand.
```

No index table exists on disk either (expected — `render_template` projects into
`librarian(action="context")` only, never to the file), so the catalog is the only
place the table could ever have appeared, and it cannot.

## Reproduction

Measured 2026-08-23 at HEAD `e97d89bc1725cae0adb0838bc9bde025383d7469`, branch
`experiments`:

```
workspace(action="activate", path="/home/marius/work/claude/codescout")
artifact(action="get", id="5086e3c7c0b9d83c", full=true)
```

Read the `augmentation` field → `null`. Read the body → claims a [LIVE] table.

## Environment

codescout MCP, project `codescout`, branch `experiments`, Linux. Catalog is the
local SQLite catalog (not in git — see CLAUDE.md § *Tool Usage Patterns*).

## Root cause

Unknown — see Hypotheses tried. Two candidates, not yet separated.

## Evidence

### E-1 — catalog state

`artifact(action="get", id="5086e3c7c0b9d83c", full=true)`, 2026-08-23:
`"augmentation": null`, `"time_scope": "dated_snapshot"`, `"status": "active"`,
`created_at` 2026-05-08, `updated_at` 2026-08-17.

### E-2 — the body's own claim

Body § *How to save a research*, step 4 prescribes `artifact_refresh(action="gather")`
followed by `patch={params: {...}}` with `commit_refresh=true` — both of which
presuppose an augmentation that E-1 shows is absent.

### E-3 — existing entries do not carry the frontmatter the table would read

`head -12` of `docs/research/2026-07-03-mcp-guidance-findings.md` and
`docs/research/2026-07-02-lite-vs-hybrid-benchmark.md` (2026-08-23): neither file
opens with the five-key YAML block that § C-7 declares mandatory. So even with an
augmentation attached, a gather pass would find nothing to render for those entries.
This suggests the augmentation may never have been attached, rather than lost.

## Hypotheses tried

1. **Hypothesis:** the augmentation was attached at index creation (2026-05-08) and
   lost in a later catalog rebuild.
   **Test:** not yet run. The catalog is not in git, so there is no history to diff.
   **Verdict:** deferred.
2. **Hypothesis:** the augmentation was never attached — the index was authored from
   its design spec describing the intended end state, and the
   `artifact_augment` call was never made.
   **Test:** not yet run. E-3 is weak supporting evidence (no entry file carries the
   frontmatter the template would consume), as is the spec reference in the body's
   § History.
   **Verdict:** deferred, currently the likelier of the two.

## Fix

Not yet planned. If hypothesis 2 holds, the fix is a one-time
`artifact_augment(id="5086e3c7c0b9d83c", prompt=…, params={entries: [...]},
render_template=…, entry_collection="entries")` plus backfilling the five-key
frontmatter onto the pre-C-7 entry files. If hypothesis 1 holds, the same call
restores it, but the catalog-durability question is the real defect and belongs in a
separate record.

Decide which before writing either — per CLAUDE.md, run the reproduction before
reading the fix plan.

## Tests added

None — bug is filed, not fixed.

## Workarounds

Save research files by writing them directly with the § C-7 frontmatter (the
procedure's steps 1–3), and skip step 4. The body already states that a file saved
without the refresh is still saved correctly. Discovery still works via
`artifact(action="find", …)` after a `librarian(action="reindex")`.

## Resume

Run `artifact_event(action="list", artifact_id="5086e3c7c0b9d83c")` and look for an
`artifact_augment` event dated near 2026-05-08. Its presence confirms hypothesis 1
(lost in rebuild); its absence, with other events from that date present, confirms
hypothesis 2 (never attached). That single command separates them.

## References

- `docs/research/README.md` — the affected tracker (artifact `5086e3c7c0b9d83c`)
- `docs/superpowers/specs/2026-05-08-researcher-tracker-design.md` — the spec the
  index body cites as its origin
- CLAUDE.md § *Tool Usage Patterns* — the augmented-artifact pattern and the
  "catalog is not in git" caveat
- `docs/architecture/augmented-artifacts.md` — body / params / render_template deep-dive
- Noticed while saving `docs/research/2026-08-23-opus5-harness-minimalism.md`
