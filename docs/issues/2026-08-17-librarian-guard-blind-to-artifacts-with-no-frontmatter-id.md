---
id: '88129ecc9c4c87a2'
kind: bug
status: open
title: 'BUG: the librarian guard is blind to any artifact whose frontmatter omits `id:` — 26 of 66 tracker/bug files are unprotected, including the most-damaged ledger'
tags:
- librarian
- guard
- trackers
- data-integrity
---

## Summary

`is_librarian_artifact` (`src/util/librarian_guard.rs:115-128`) decides whether a file
is librarian-managed by reading the file's **own text** for an `id:` line in frontmatter.
If no `id:` line is present, it returns `false` and the file is treated as an ordinary
markdown document — `edit_markdown`, `read_markdown`, and `edit_file` all proceed.

But the catalog does **not** need the file to carry an id: `id = sha256(abs_path)`, so a
tracker can be fully registered, queryable via `artifact(find/get)`, and still have no
`id:` in its frontmatter. Those artifacts are completely unguarded.

## Evidence

Measured 2026-08-17 across `docs/trackers/*.md` + `docs/issues/*.md`:

- **66 files checked, 26 have no `id:` line (39%)** — 13 trackers, 13 bug files.
- Unguarded trackers include `skill-frictions.md` and `codescout-usage-hookify.md`,
  both of which CLAUDE.md explicitly instructs agents to append to, and
  `archive-cadence-policy.md`, a live convention surface.

Demonstrated live in the same session:

```
edit_markdown("docs/trackers/reconnaissance-patterns.md", ...)   -> ACCEPTED  (no id: line)
edit_markdown("docs/trackers/tracker-hygiene-log.md",    ...)   -> REFUSED   (id: '7e49…')
```

Both are `kind: tracker`, `status: active`, `augmentation: null`, both registered in the
catalog. The only difference is whether the *file* happens to restate its own id.

## Why this matters more than the count suggests

The single most structurally damaged ledger in this repo is
`docs/trackers/reconnaissance-patterns.md`, and it is exactly the file the guard cannot
see. Today's sweep found in it: 13 body entries with no index row, 9 ids allocated
twice, 39 of 57 entries with no disposition field, and ~30 of the 39 sampled dangling
citation tokens in the whole project. An unguarded artifact accumulates hand-edits in
arbitrary shapes, because nothing routes its writers through the one surface that could
impose a shape. The correlation is causal, not coincidental.

## The repair path explicitly declines to fix it

`librarian(action="doctor", fix=repair_frontmatter_id)` exists and sweeps
`frontmatter_id_mismatch` files — but its own contract states that **"a file with NO
frontmatter id is left alone rather than stamped."** So the one tool positioned to close
this gap is documented to skip precisely this class.

## Relationship to the quoting bug

`docs/issues/archive/2026-08-08-librarian-guard-misses-quoted-frontmatter-ids.md`
(status `fixed`) reported "15 of 27 trackers are unprotected" because the predicate
mishandled YAML quoting. That fix works — `id: '7e49…'` is now correctly guarded, as
demonstrated above. This is a **distinct and larger class**: not a mis-parsed id, but no
id at all. The earlier fix was validated against the corpus that produced it (files that
*had* ids, quoted) and did not reach the adjacent case (files with none). Kin R-97.

## Fix options

1. **Key the guard on the catalog, not the file text.** A path lookup against the
   catalog is authoritative and cannot drift. Most correct; cost is catalog access on a
   hot edit path.
2. **Widen the text predicate** to also treat `kind: tracker` / `kind: bug` frontmatter
   as managed, id or not. Cheap, no catalog access, no new drift surface. Catches
   everything under `docs/trackers/` and `docs/issues/` and nothing else.
3. **Backfill `id:` into the 26 files.** Cheapest to reason about, but it re-creates the
   very drift surface `frontmatter_id_mismatch` exists to police — every future move
   re-breaks it.

Recommended: **2 now, 1 as the principled endpoint.** 3 alone treats the symptom and
adds maintenance.

## Reproduction

```
grep -L '^id:' docs/trackers/*.md          # the unguarded set
edit_markdown("docs/trackers/skill-frictions.md", heading="...", action="edit", ...)
# succeeds; no librarian_guard error
```

## Status

`open` — found 2026-08-17 during the R-N tracker-hygiene design work. Not yet fixed.

