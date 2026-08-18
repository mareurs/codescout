---
id: '9c953c973226d4e0'
kind: bug
status: fixed
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
closed: 2026-08-18
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

**SHIPPED 2026-08-18.** Both halves, and the fix turned out to be smaller than filed because
the predicate it needed already existed.

1. **Two findings, not one.** `check_frontmatter_id_matches_catalog` now branches: a declared
   value that is not a 16-hex catalog id yields **`frontmatter_id_is_not_a_catalog_id`**, whose
   message says the value was never minted by the catalog, names the likely causes (template
   placeholder, hand-written slug), states that the repair deliberately skips the row, and says
   why. The stale-move message is unchanged for the rows it is actually true of.
2. **The repair cannot reach those rows.** `scan_frontmatter_id_mismatches` — which feeds
   `fix=repair_frontmatter_id` — now filters to `check == "frontmatter_id_mismatch"`. Its name
   had always promised only those rows; nothing enforced it while the check emitted one kind.
3. **And the move path is guarded independently.** `mv::repair_frontmatter_id`'s gate went from
   `id != new_id` to `id != new_id && is_librarian_id(id)`. `artifact(move)` reaches that
   function without passing through `doctor` at all, so the filter in (2) would not have covered
   it.

**`is_librarian_id` already existed** in `src/util/librarian_guard.rs` — the fix was making it
`pub(crate)`, not writing it. That matters for more than economy: it **strips matching quotes**,
because a quoted id is 18 characters and once failed a raw length test, leaving 15 files in
`docs/trackers/` unguarded (BL-33). A hand-rolled 16-hex regex here would have reintroduced that
exact defect in a second place — and the real corpus has quoted ids, including the BL-41 bug file
archived the same day (`id: '52269554ea4f51a4'`). Pinned by the quoted fixture in the test below.

**The sharper framing, found while fixing rather than while filing:** this is not a new
abstention. The existing no-`id:` abstention's *stated reason* — stamping an id newly subjects
the file to the librarian guard — already covers this case verbatim, because
`is_librarian_id("ADR-{NUMBER}")` is false, so a placeholder-bearing template is **unguarded
today** and the repair would have created precisely the condition that abstention exists to
avoid. The code was not missing a rule; it was testing the wrong predicate — "is there an `id:`
value?" instead of "is there a *catalog* id?".

Option (3) from the original filing — excluding templates from the catalog via a per-repo
classifier rule — was not taken. It is still the cleaner end state, but it is a decision for each
repo that owns those templates, and the rows are now reported honestly rather than silently
misfiled.
## Tests added

Two, both written before the code and both watched fail.

**`a_declared_value_that_was_never_a_catalog_id_is_a_different_finding`** — the discriminating
pair the check could not tell apart. A **quoted** stale hex id must still be
`frontmatter_id_mismatch` (the BL-33 regression guard), and `ADR-{NUMBER}` must be the new check
with a detail that does **not** contain "moved away from". A single-fixture test could not
separate these: both differ from the row.

**`repair_frontmatter_id_never_rewrites_a_value_that_was_never_a_catalog_id`** — the destructive
half, end to end through `call` with `confirm=true`. Kept isolated from
`repair_frontmatter_id_sweeps_the_stale_and_leaves_everything_else` on purpose: this rejects one
class, so its fixture must be the only reason anything survives. It carries a stale-hex row as a
**positive control**, without which a repair that silently did nothing would pass.

### Two things the RED runs established that the filing could not

- **The destruction is REPRODUCED, not merely reasoned.** The filing was careful to say the
  destructive half was "reasoned from the code path, NOT executed". The first RED run executed
  it: the sweep reported `repaired: [adr-template.md …]` and rewrote `id: ADR-{NUMBER}` to
  `id: 2222222222222222`. Upgrading that claim is the point of watching a test fail.
- **The positive control caught MY error, not the code's.** After the guard landed, the test
  still failed — on the control, because I asserted `read(…).contains("id: 1111111111111111")`
  and the splice does not emit that spelling. The code was right; my assertion was. Rewritten to
  read through `frontmatter::parse`, which is what `mv::repair_frontmatter_id`'s own comment
  calls authoritative about what YAML sees. Asserting a spelling the writer never produces is the
  mirror of the fixture-derived-from-the-code defect in memory `test-design-discipline`.
## Workarounds

**Always read the dry run's `files[]` before `confirm=true`**, and scope `root=` to the repo you
actually mean. That is already the documented discipline for this fix — the sweep's own test
records a dry run listing **207 files across five unrelated repositories** — and it is enough to
catch this, because the templates are visible by name in the preview.

## Resume

Nothing outstanding on the code. Verify on the wire after the next `cargo rb` + `/mcp`: the six
`frontmatter_id_mismatch` rows should split **3 / 3** between the two check names, and the three
non-id rows should be the two ADR/FDR template placeholders plus the `meetings-reranker` slug.

Still open, and deliberately so: **whether those templates belong in the catalog at all.** Option
(3) is a per-repo `.codescout/librarian.toml` classifier decision in repos this session does not
own, and all six affected files are outside codescout. The defect was in codescout's code, so it
is fixed here against synthetic fixtures; the data question goes with the repos.

Not yet re-measured either: **why the count moved 4 → 6** during the session. No codescout change
explained it and the concurrent session's commits touched unrelated files. Most likely a reindex
admitting two rows — recorded as unexplained rather than guessed, and now cheaper to answer,
since the two check names separate the populations.

Fast-forward path (`master...experiments` is `0` on the left), so the fix SHA below is already the
master-side SHA and there is no second one to record.
## References

- `src/librarian/tools/doctor.rs` — `check_frontmatter_id_matches_catalog`, and the
  `repair_frontmatter_id` arm of `call`
- `src/librarian/tools/mv.rs` — `repair_frontmatter_id`, whose BL-34 comment names the
  placeholder hazard for the other keys
- `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md` (BL-23 — why the repair exists)
- `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` (why stamping an `id:` changes guard behaviour)
