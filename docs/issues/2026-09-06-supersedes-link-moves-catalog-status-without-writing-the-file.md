---
id: a5bc701f69b9a317
kind: bug
status: open
title: 'BUG: doc(action=link, rel=supersedes) moves the status in the catalog and never writes the file'
tags:
- cluster/unclassified
---

## Summary

`doc(action="link", rel="supersedes")` sets `dst.status = "superseded"` **in the catalog only**. The
destination file's frontmatter keeps its previous status, nothing is written to disk, and `git
status` reports the tree clean. The file — which this project declares the source of truth — then
says `open` while every `find`, every triage query and every "what's still live?" report says
`superseded`.

## Symptom (Effect)

A superseded bug reads as open to anyone reading the file, and as superseded to anyone querying the
catalog. Neither party sees a conflict, and the divergence survives a commit because there is
nothing to commit.

## Reproduction

Observed 2026-09-06 while merging a duplicate bug file into its original:

```
doc(action="link", src_id=876d7282ddc61f06, dst_id=36ff17248b2c6ec7, rel="supersedes")  → "ok"

doc(action="get", id=36ff17248b2c6ec7)      → "status": "superseded"
head -8 <that file> | grep '^status:'        → status: open
git status --porcelain -- <that file>        → (empty)
```

Three readers, two answers, no error anywhere. Repaired by hand with
`doc(action="update", id=…, patch={status:"superseded"})`, which does round-trip to the file.

## Environment

`experiments`. Not branch-scoped.

## Root cause

`update` writes through to frontmatter; `link` does not, though both change the same catalog column.
The invariant the manual states — *"File is source of truth. Catalog is a derived index; writes
round-trip through frontmatter on disk"* (`src/librarian/prompts/companion_hint.md` § Gotchas) — has
one documented writer honouring it and one silently not.

The status transition is also a **side effect of a different verb**. A caller asks to record a
relation; the status change is a consequence they did not name and are not shown. The response is
the bare string `"ok"` — it does not report that a status moved, so even an attentive caller has
nothing to reconcile against.

## Hypotheses tried

1. **Hypothesis:** already filed as the `edit_markdown` frontmatter desync
   (`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`).
   **Verdict:** rejected — that one runs **file → catalog** (a frontmatter write the catalog did not
   see). This runs **catalog → file** and is the mirror. Both leave the pair disagreeing; neither
   fix reaches the other direction.
2. **Hypothesis:** `doctor` already reports it.
   **Verdict:** rejected. `doctor` carries `frontmatter_id_mismatch` and
   `frontmatter_id_is_not_a_catalog_id` — **id** only. There is no `frontmatter_status_mismatch`,
   so this divergence is not merely unfixed, it is unmeasured. The corpus-wide count is unknown and
   was not derived here.

## Fix

Make `link` write through, exactly as `update` does, when it transitions `dst.status`. And have it
say so: returning bare `"ok"` for a call that moved a status on a second artifact is the reason the
divergence went unnoticed for the length of a session — report the transition in the response.

Then measure the existing population before assuming it is one: add
`frontmatter_status_mismatch` to `doctor` alongside the two id checks. Until that exists, **every
`supersedes` link ever recorded is a candidate** and nobody can say how many diverged, because the
question has never had an instrument.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: link two artifacts with `rel="supersedes"`, then assert
the destination FILE's frontmatter reads `superseded`. That test fails today. Asserting the catalog
value instead would pass today and prove nothing — it is the half that already works.

## Workarounds

After any `supersedes` link, follow with
`doc(action="update", id=<dst>, patch={status:"superseded"})`. It is idempotent against the catalog
and is what actually writes the file.

## Resume

Wire the write-through, observe the RED above, then add the doctor check and derive the corpus count
that this file deliberately does not guess at.

## References

- Mirror-direction sibling, archived: `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`.
- Same file/catalog pair, **id** rather than **status**:
  `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`.
- **Candidate class, not yet a cluster** — *"a write path updates one half of the file/catalog pair
  and reports success"*. Three instances now span two directions and two fields (status via `link`,
  status via `edit_markdown`, id via move/worktree). Tagged `cluster/unclassified` deliberately
  rather than forced into an existing slug; if a fourth appears, this is the shape to promote.

