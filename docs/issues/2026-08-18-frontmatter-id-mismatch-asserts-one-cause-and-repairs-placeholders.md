---
id: '9c953c973226d4e0'
kind: bug
status: open
title: 'BUG: frontmatter_id_mismatch asserts one cause for every mismatch, and its repair would overwrite a template''s id placeholder'
owners:
- marius
tags:
- librarian
- doctor
- misleading-error
- destructive-repair
- cross-repo
topic: catalog-drift
---

# BUG: `frontmatter_id_mismatch` asserts one cause, and its repair would overwrite a placeholder

## Summary

`doctor`'s `frontmatter_id_mismatch` fires whenever a file's frontmatter `id:` differs from
its catalog row's id, and its detail names exactly **one** cause:

> `a move re-keys the row and this file kept the id it was moved away from`

That is true for a stale 16-hex id. It is **false** for a value that was never a catalog id at
all — a template placeholder, or a human-readable slug. Measured 2026-08-18: **3 of 6** live
instances are the second kind.

The worse half is the repair. `fix=repair_frontmatter_id` rewrites every reported file's `id:`
to its catalog id, filtered only by *path* containment. Nothing checks what the declared value
**was**, so pointing it at the affected root would overwrite `id: ADR-{NUMBER}` in an ADR
template with a 16-hex id — silently destroying the placeholder that makes it a template.

## Symptom (Effect)

Two effects, of different severity.

1. **Misleading detail (observed).** Three live rows assert a move that never happened:

| declared | what it actually is |
|---|---|
| `ADR-{NUMBER}` | an ADR template's fill-in placeholder |
| `FDR-{NUMBER}` | an FDR template's fill-in placeholder |
| `meetings-reranker` | a human-readable slug, never a catalog id |

A reader following the stated cause goes looking for a move in the file's history and finds
none. The remaining 3 declare real 16-hex ids, where the message is correct.

2. **Destructive repair (reasoned from the code path, NOT executed).** `confirm=true` on the
   root holding those templates would splice the `id:` line to the catalog id. The next person
   copying the template gets a file asserting another artifact's identity — and because a
   16-hex `id:` is one of the three things `librarian_guard` keys on, the copy also becomes
   guard-refused for `edit_markdown`.

## Reproduction

```
librarian(action="doctor")
# -> 6 frontmatter_id_mismatch rows; read the `detail` of the two *-template.md ones

librarian(action="doctor", fix="repair_frontmatter_id",
          root="<the repo holding those templates>")     # dry_run, lists them as repairable
```

The dry run is enough to see it: the templates appear in `files[]`, so `confirm=true` would
rewrite them. **Do not run the confirm** — that is the defect, not the diagnostic.

## Environment

codescout `experiments` @ `282586b1`, measured on the live catalog (1,056 artifacts spanning
several repos, umbrella `codescout-ecosystem`). All 6 instances are in repos other than
codescout: `mirela/eduplanner-ui` (3), `stefanini/invest-europe` (2+).

## Root cause

Two gaps, one per effect.

- **The message is a `format!` with no branch.** `check_frontmatter_id_matches_catalog`
  (`src/librarian/tools/doctor.rs`) computes `declared != id` and then states a single cause
  unconditionally. The check has at least two causes and reports one.
- **The repair's only filter is path containment.** In `doctor.rs`'s `repair_frontmatter_id`
  arm, `scan_frontmatter_id_mismatches` rows are filtered by `containing_root` and nothing
  else, then handed to `mv::repair_frontmatter_id`, whose gate is
  `fm.id.as_deref().is_some_and(|id| id != new_id)`. A placeholder satisfies that.

**The near-miss is worth recording, because the author reasoned about placeholders and the
reasoning landed one field away.** `mv::repair_frontmatter_id`'s own comment says it writes
through a line splice rather than re-emitting the block because *"a `{Placeholder}` would stop
being one"* (BL-34). That protects every **other** key's placeholder from the write. It does not
consider that the `id:` value being rewritten might itself be a placeholder — which is the one
field this function exists to change.

## Evidence

- 6 rows measured; 3 declare non-id values, listed above.
- `check_frontmatter_id_matches_catalog` already documents **three** deliberate abstentions
  (no `id:` at all, missing file, unparseable frontmatter). "Declared value was never an id" is
  a fourth of the same kind and is absent.
- The no-`id:` abstention exists for a closely related reason — *"stamping one would newly
  subject the file to the librarian guard"* — which is exactly the consequence of rewriting a
  placeholder. The principle is already stated in this file; it just is not applied to this case.

## Hypotheses tried

- *"Maybe my `entry_prefix` frontmatter additions caused the 4 → 6 rise."* **Ruled out** by
  reading the rows: all 6 are in other repos, none in codescout, and the four files I touched
  (`structural-debt-refactor`, `2026-08-16-iron-law-gate-firing-audit`, `fable-tuning-findings`,
  `fable-tuning-tasks`) appear in none of them. Checking that is what surfaced this bug.
- *"Maybe the line-splice already protects placeholders."* **No** — read it. The splice
  preserves other keys; the gate that decides whether to splice at all is a bare inequality.

**Why the count moved 4 → 6 is NOT established.** No codescout change explains it, and the
peer session's commits touch only `src/server.rs`, `src/tools/mod.rs`, `src/tools/session_key.rs`
and a plan doc. Most likely a reindex admitted two more rows. Recorded as unexplained rather
than guessed.

## Fix

Not implemented. The two halves are independent and the first is nearly free.

1. **Branch the message on the shape of `declared`.** If it is not a 16-hex id, say so — *"the
   declared value is not a catalog id (template placeholder? slug?), so this is not a stale
   move"*. One `if`, and it stops sending readers after a nonexistent commit.
2. **Add the fourth abstention: never rewrite a non-16-hex `id:`.** Cheapest correct form is a
   shape guard in `mv::repair_frontmatter_id` beside the existing present-or-absent gate, so
   both the move path and the sweep inherit it. Report such files under a distinct check name
   (or a `severity_reason`) rather than silently skipping — a template sitting in the catalog is
   itself worth a human's attention.
3. **Consider whether templates belong in the catalog at all.** `docs/adr/templates/*.md` is
   arguably not an artifact. Classifier exclusion would remove the rows rather than annotate
   them — cleaner, but it is a per-repo `.codescout/librarian.toml` decision in repos this
   session does not own.

## Tests added

None yet. Fix (2) needs the discriminating pair memory `test-design-discipline` calls for: one
fixture with a genuinely stale 16-hex id (**must** be repaired) and one with `ADR-{NUMBER}`
(**must not** be), because a test that only asserts "the stale one got fixed" stays green under
a repair that rewrites everything. The existing
`repair_frontmatter_id_sweeps_the_stale_and_leaves_everything_else` is the right shape and its
`d.md` row makes exactly this argument for the no-id case — it needs a placeholder sibling.

## Workarounds

**Always read the dry run's `files[]` before `confirm=true`**, and scope `root=` to the repo you
actually mean. That is already the documented discipline for this fix — the sweep's own test
records a dry run listing **207 files across five unrelated repositories** — and it is enough to
catch this, because the templates are visible by name in the preview.

## Resume

Fix (1) is a one-line branch and can ship independently of everything else. Fix (2) is the one
with teeth. Neither is urgent: the repair is opt-in, root-scoped, and requires `confirm=true`,
and no session has run it against the affected repos.

Note the affected files are all outside codescout, so verifying a fix end-to-end means either a
synthetic fixture (preferred) or the owner's go-ahead to touch those repos.

## References

- `src/librarian/tools/doctor.rs` — `check_frontmatter_id_matches_catalog`, and the
  `repair_frontmatter_id` arm of `call`
- `src/librarian/tools/mv.rs` — `repair_frontmatter_id`, whose BL-34 comment names the
  placeholder hazard for the other keys
- `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md` (BL-23 — why the repair exists)
- `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` (why stamping an `id:` changes guard behaviour)

