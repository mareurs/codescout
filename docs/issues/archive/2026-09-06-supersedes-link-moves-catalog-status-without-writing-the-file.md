---
id: c5bdee8fff6146dd
kind: bug
status: fixed
title: 'BUG: doc(action=link, rel=supersedes) moves the status in the catalog and never writes the file'
tags:
- cluster/unclassified
closed: 2026-09-07
unverified: The write-through is fixed and regression-tested; the pre-existing corpus is NOT. No `frontmatter_status_mismatch` check exists, so artifacts that diverged before `05da2db7` are unmeasured and unrepaired.
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

Fix SHA: `05da2db7` (`experiments`)
Patch-id: `889c5c8029d446f688409ca6aae92fb4da631aab`

**Done 2026-09-07 — the first two paragraphs above.** `link` now calls
`update::write_field_to_frontmatter`, the same primitive `event_create` uses for
`status_change`, and adopts its ordering: **file before catalog**, so a failed disk write
leaves the catalog untouched rather than recording a transition that never reached the
file. Writing the row first would have reproduced this bug with a smaller window.

One hazard worth recording because it does not fail loudly: that primitive takes
`ctx.catalog.lock()` itself, and `link::call` held the lock across its whole body.
`parking_lot::Mutex` is not reentrant, so the naive call **deadlocks rather than errors**.
The lock is now scoped in narrow blocks, the shape `event_create::call` already used.

The supersedes response is no longer bare `"ok"` — it returns
`{ok, superseded: {id, status, previous_status}}`. Non-supersedes links still return
`"ok"`. This is inside the no-echo-writes convention, not an exception to it: the rule
reserves richer responses for *genuinely new info*, and a status transition on a second
artifact the caller never named is exactly that.

**NOT done — the third paragraph, and it is a different defect.** There is still no
`frontmatter_status_mismatch` in `doctor`, so the corpus-wide population of
already-diverged artifacts remains unmeasured and unrepaired. Split out rather than
absorbed here, because an instrument that has never existed is not the same work as a
write path that was missing one call.

## Tests added

Two, in `src/librarian/tools/link.rs`, **both observed RED before the fix** rather than
asserted into existence:

- `supersedes_writes_the_new_status_to_the_dst_file` — reads the destination **file** and
  asserts its frontmatter. Failed with `status: draft` on disk while the catalog said
  `superseded`.
- `supersedes_reports_the_status_transition_it_caused` — failed against the bare `"ok"`.

**The measurement that matters most is the third line of that run:** the pre-existing
`supersedes_transitions_dst_status` **passed** while the defect was live. It asserts the
catalog value — the half that already worked — so it is monotone under exactly the failure
it looks like it guards. It passed for the whole life of the bug. That is this repo's
monotone-assertion law demonstrated rather than restated, and it is why the acceptance
criterion this file wrote in advance ("assert the FILE") was the correct one.

Both older supersedes tests now build **real on-disk fixtures** instead of `mk_row`
synthetics, and say so at the fixture with a do-not-revert note: now that `link` writes
frontmatter, a row whose `abs_path` names no file cannot reach the transition at all.
That is the contract, not an obstacle — but it is also exactly the kind of detail a
tidy-up removes, leaving the test passing and no longer discriminating.
## Workarounds

After any `supersedes` link, follow with
`doc(action="update", id=<dst>, patch={status:"superseded"})`. It is idempotent against the catalog
and is what actually writes the file.

## Resume

Write-through and response: **done**, `05da2db7`, gate green (fmt, clippy, lean 3560,
default 5508 / 0 failed).

Still open, and now carried by its own bug file: `doctor` has `frontmatter_id_mismatch`
but no `frontmatter_status_mismatch`, so **every `supersedes` link recorded before
`05da2db7` is a candidate for divergence and nobody can say how many, because the question
has never had an instrument.** This file deliberately does not guess the number.
## References

- Mirror-direction sibling, archived: `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`.
- Same file/catalog pair, **id** rather than **status**:
  `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`.
- **Candidate class, not yet a cluster** — *"a write path updates one half of the file/catalog pair
  and reports success"*. Three instances now span two directions and two fields (status via `link`,
  status via `edit_markdown`, id via move/worktree). Tagged `cluster/unclassified` deliberately
  rather than forced into an existing slug; if a fourth appears, this is the shape to promote.
