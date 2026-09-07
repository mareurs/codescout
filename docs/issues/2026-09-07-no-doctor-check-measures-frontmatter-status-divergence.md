---
id: '18fc99be15d48940'
kind: bug
status: open
title: 'BUG: no doctor check measures frontmatter/catalog status divergence, so the corpus population is unknowable'
tags:
- cluster/unclassified
- librarian
- doctor
- catalog-drift
opened: 2026-09-07
owner: marius
severity: medium
---

## Summary

`doctor` carries `frontmatter_id_mismatch` and `frontmatter_id_is_not_a_catalog_id` — **id
only**. There is no equivalent for `status`, so a file whose frontmatter `status` disagrees
with its catalog row is not merely unfixed, it is **unmeasured**: no query, check or gate
can name one, and the corpus-wide count has never been derivable.

This is the residual of
`docs/issues/archive/2026-09-06-supersedes-link-moves-catalog-status-without-writing-the-file.md`,
split out rather than absorbed because an instrument that has never existed is different
work from a write path that was missing one call.

## Symptom (Effect)

Nothing observable — which is the defect, and it is the second-order form of the bug it
came from. That one made a *file* say `open` while every `find` said `superseded`. This one
means nobody can ask **how many** files are in that state. Both readers are confident, no
error is raised anywhere, and `git status` is clean because a catalog-only divergence has
nothing to commit.

## Reproduction

```
librarian(action="doctor")   → violations include frontmatter_id_mismatch,
                               frontmatter_id_is_not_a_catalog_id
                             → no check named frontmatter_status_mismatch
```

Then, for the population itself: there is no command to run. That absence *is* the
reproduction.

## Root cause

`doctor`'s file-vs-catalog checks were written for the **id** field, which is the one that
breaks under `doc(action="move")` (`id = sha256(abs_path)`). `status` is the other field
both halves store, it drifts by a different mechanism, and no check was extended to it.

## Why this is worth an instrument rather than a sweep

Two known write paths produced this divergence in **opposite directions**, and each was
fixed without either fix reaching the other:

- catalog → file: `doc(action="link", rel="supersedes")` — fixed `05da2db7`
- file → catalog: `edit_markdown` frontmatter write —
  `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`

A third, on the *id* field, is
`docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`.

So the class has at least three members across two directions and two fields, every one
found by a human noticing an oddity rather than by any check. A one-off sweep would answer
today's count and rebuild no capability; the next write path that forgets to round-trip
lands in exactly the same silence.

## Fix

Add `frontmatter_status_mismatch` to `librarian(action="doctor")`, beside the two id
checks: for each catalogued artifact whose file exists, compare frontmatter `status`
against the row's, and report the pair.

Then **derive the corpus count and publish the derivation, not the value** — every
`supersedes` link recorded before `05da2db7` is a candidate, and the number will decay.

Two design notes, both learned from the id checks rather than guessed:

- **Report, do not repair.** The id checks are read-only by default with an opt-in `fix=`.
  Which side is authoritative is not always the file: a row can be right and a file
  hand-edited. `doctor` should say so and let the reader choose.
- **Missing-file is a distinct outcome, not a mismatch.** `frontmatter_id_mismatch`
  already has a documented false-positive mode for ids minted in another checkout; a
  status check needs its own answer for an artifact whose file is absent, rather than
  reporting it as disagreement.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: construct an artifact whose file says
`active` and whose row says `superseded`, run `doctor`, assert the violation is reported
and names both values. Asserting only that `doctor` runs would pass today.

## Workarounds

None for measurement. For a single artifact, `doc(action="update", id=…,
patch={status:…})` round-trips to the file and is what repairs one by hand — but you have
to already suspect it, which is the whole problem.

## Resume

Unclaimed. Write the check first; the count is a consequence of having it, and guessing
the count without the check is what this file exists to avoid.

## References

- Parent, fixed and archived:
  `docs/issues/archive/2026-09-06-supersedes-link-moves-catalog-status-without-writing-the-file.md`
- Mirror direction, archived:
  `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`
- Same pair, id rather than status:
  `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`
- Tagged `cluster/unclassified` deliberately. The parent named a candidate class — *"a
  write path updates one half of the file/catalog pair and reports success"* — and this
  file is **not** a fourth instance of it: it is the absence of the instrument that would
  have counted the first three. If that class is promoted, this belongs beside it as its
  measurement gap, not as a member.

