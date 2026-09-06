---
id: b51f6ece4db5a8e2
kind: bug
status: fixed
title: 'BUG: gc::apply_rehome changes an artifact id without updating artifact_chunk.artifact_id, which references it'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- catalog
- schema
- gc
closed: 2026-09-06
opened: 2026-09-02
owner: marius
related:
- docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md
severity: medium
---

## Summary

`gc::apply_rehome` changes an artifact's primary key (`UPDATE artifact SET id = ?1 … WHERE id = ?3`) and never updates `artifact_chunk.artifact_id`, which references it. The FK carries `ON DELETE CASCADE` but **no `ON UPDATE` clause**, so the update action is `NO ACTION`. Rehoming an artifact that has chunk rows therefore leaves children naming an id that no longer exists.

## Symptom (Effect)

**OBSERVED 2026-09-06.** The predicted fork is the one that occurs: a failure at
COMMIT, not silent orphaning.

```
rehome must not fail on an artifact that has chunk rows: FOREIGN KEY constraint failed
Caused by: Error code 787: constraint failed
```

So the integrity news is good and the availability news is bad. Nothing is
corrupted — `apply_rehome` is one transaction, so the whole batch rolls back and
no artifact is left half-rehomed and no chunk row is left naming a dead id. What
is broken is that `librarian(action="doctor", fix="rehome", …)` **cannot run at
all** on an artifact carrying chunk rows, which since v11 is every artifact that
has been embedded. The documented cross-machine recovery path was dead for the
modern corpus.
## Reproduction

**RUN 2026-09-06 at `2b8ac612`. Step 4 resolved: it is the COMMIT-failure fork.**

The check is now a permanent test rather than a recipe —
`rehome_carries_chunk_rows_and_their_vectors_to_the_new_id` in
`src/librarian/catalog/gc.rs`. It seeds two chunk rows plus an `artifact_vec_v2`
row for each, drives `apply_rehome`, and asserts the chunks follow the new id and
the vectors survive.

Written as the correct-behaviour assertion **on purpose**, so that *how* it failed
would discriminate between the two forks without needing a third run: an error out
of `apply_rehome` means COMMIT failure, a failed count assertion means silent
orphaning. It failed with `FOREIGN KEY constraint failed`.
## Environment

Linux, `experiments`, schema v11. Reached through `librarian(action="doctor", fix="rehome", old_root=…, new_root=…)`, which is the only production caller.

## Root cause

**Composed from three facts each verified at the bytes, not from a run.** Marked `unverified` in frontmatter for exactly that reason.

1. `src/librarian/catalog/mod.rs:262` — `artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE`. There is no `ON UPDATE` clause, so SQLite's default `NO ACTION` applies to parent-key updates.
2. `src/librarian/catalog/gc.rs` — `grep artifact_chunk` returns **0 matches** in the whole file, while `apply_rehome` rewrites `entry_cite`, calls `migrate_vec_id`, and then does `UPDATE artifact SET id = ?1, abs_path = ?2, missing_since = NULL WHERE id = ?3` (`:485-488`).
3. `src/librarian/catalog/gc.rs:453` — `PRAGMA defer_foreign_keys = ON`. This **defers** the constraint check to COMMIT; it does not disable it. FKs are enabled globally (`PRAGMA foreign_keys = ON` on all three `Catalog` constructors).

*Why it was not caught:* `artifact_chunk` is new in v11 (Task 4 of the chunk-grain plan). `apply_rehome` predates it, was correct when written, and nothing re-derives the list of child tables when one is added. `migrate_vec_id` exists precisely because someone once had to hand-handle a table the FK graph did not cover — so the file already contains the evidence that this class recurs, one table earlier.

**And the list was behind by TWO tables, not one.** `entry_reservation.artifact_id`
carries the same FK and was equally absent. It appears in no report because a
reservation row exists only for a ledger that has allocated an entry id, so it
takes a rehome of a ledger specifically to surface — a narrower trigger than
`artifact_chunk`'s, not a smaller defect. It was found by enumerating every FK
onto `artifact(id)` rather than by repairing the reported instance, which is the
same sentence this section already contains: nothing re-derives the list.

## Evidence

### The FK, with no ON UPDATE

```
CREATE TABLE IF NOT EXISTS artifact_chunk (
   chunk_id     TEXT PRIMARY KEY,
   artifact_id  TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
   chunk_ix     INTEGER NOT NULL,
```

### `gc.rs` never names the child table

`grep(pattern="artifact_chunk", glob="src/librarian/catalog/gc.rs")` → `0 matches`.

### The parent-key update, and the deferral that moves the error

`gc.rs:485-488` rewrites `artifact.id`; `gc.rs:453` sets `defer_foreign_keys = ON` for the enclosing transaction.

## Hypotheses tried

1. **Hypothesis:** `artifact_vec_v2` also needs migrating on rehome, like `artifact_vec` does via `migrate_vec_id`.
   **Test:** read the v11 schema comment at `catalog/mod.rs:257-258`.
   **Verdict:** **rejected, and it is the reason this bug is narrow.** `artifact_vec_v2` is keyed by `chunk_id`, which does not change when the artifact id does — the comment names avoiding an `O(chunks)` `migrate_vec_id` loop as the *reason* for that key. So the vectors are fine; only the `artifact_chunk.artifact_id` back-reference is stale.

## Fix

Applied at `35f97afb` (`experiments`), patch-id
`e401b6d763373dda77c59a3e186b7de2cf8e46bf`.

**The second question in this section was the right one, and it is what shipped.**
Rather than adding one `UPDATE` per missing table, the hand-list is now a named
constant, `REHOME_CHILD_COLUMNS`, and a test derives the true set from the live
schema and fails when the constant lags it. Two entries were added to the
constant — `artifact_chunk` and `entry_reservation` — but the entries are not the
fix; the derivation is. Patching the reported table alone would have left the
second one to be discovered exactly as the first was.

**One correction to this file's own suggested query.**
`PRAGMA foreign_key_list(artifact)` lists the foreign keys **from** `artifact` to
other tables — the opposite direction from the one needed. Finding the children
means enumerating every table and filtering on the parent:

```sql
SELECT m.name, p."from"
  FROM sqlite_master m
  JOIN pragma_foreign_key_list(m.name) p
 WHERE m.type = 'table'
   AND p."table" = 'artifact'
   AND (p."to" = 'id' OR p."to" IS NULL)
```

The prescribed query was not wrong about SQLite; it answered the adjacent
question, and would have returned a confident empty-ish result rather than an
error.

**Considered and dismissed:** whether rewriting `artifact_id` can collide, since
`artifact_chunk` has `UNIQUE (artifact_id, chunk_ix)` and `entry_reservation` is
keyed `(artifact_id, prefix)`. A rehome's `new_id` is derived from the new path
and does not already exist — `plan_rehome` routes the case where it does into
`collisions`, which `apply_rehome` skips — so no surviving row can land on an
occupied key.
## Tests added

Both in `src/librarian/catalog/gc.rs`, and **each watched red on the production
path** — one mutation per guarded site, because a kill at one says nothing about
the other.

**`rehome_carries_chunk_rows_and_their_vectors_to_the_new_id`** — the instance.
Red before the fix with `FOREIGN KEY constraint failed`. It asserts the chunk rows
follow the new id **and** that their `artifact_vec_v2` rows survive: a count of
chunk rows alone cannot distinguish "the rows moved" from "the rows were
cascade-deleted and something re-created them", because
`artifact_vec_v2_cascade_delete` fires on any chunk DELETE. Two chunks rather than
one, so the assertion cannot be satisfied by a single row moving.

**`rehome_child_columns_cover_every_fk_onto_artifact_id`** — the mechanism.
Mutated by deleting `entry_reservation` from `REHOME_CHILD_COLUMNS`: it was the
**only** test of 16 to fail, and its message named the missing table. Carries a
non-vacuity floor (`actual.len() >= 6`), because a schema query that silently
matched nothing would satisfy every assertion below it and read as a clean result;
and a `stale` leg in the opposite direction, so a dropped or renamed table cannot
leave a dead entry behind to fail later with `no such table`.

**What the mutation also showed, and it is the reason this file exists:**
`rehome_rewrites_id_and_preserves_all_children` stayed **green** throughout. Its
name claims the whole child set; its body seeds one row per child table *as they
stood before v11*. Fifteen tests could not see a missing child table. The
sixteenth names it.
## Workarounds

Do not run `librarian(action="doctor", fix="rehome")` on a catalog whose artifacts have been chunk-indexed until this is settled. The affected population is any catalog at schema v11 that has run `reindex` since Task 5 shipped (`e811ffd6`).

## Resume

> **CLOSED 2026-09-06 — fixed at `35f97afb`, patch-id
> `e401b6d763373dda77c59a3e186b7de2cf8e46bf`, four-command gate green on
> `experiments` (both lanes read per-lane; the lean lane runs zero librarian
> tests and is not evidence here).** Nothing outstanding.

The severity question this file deferred to the reproduction is answered:
`medium` stands. The fork that occurred is the loud, atomic one — no catalog was
ever corrupted — and the cost was a dead repair path rather than lost data. It is
deliberately not raised after the fact: this file pre-committed to letting the
run decide, and the run chose the less severe branch.
## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` § *Task 7* — found while scouting that task; the plan's `gc.rs` instruction was wrong in two other ways, corrected there
- `src/librarian/catalog/gc.rs` `apply_rehome`, `migrate_vec_id`
- `src/librarian/catalog/mod.rs:257-262`, `:294-298`
