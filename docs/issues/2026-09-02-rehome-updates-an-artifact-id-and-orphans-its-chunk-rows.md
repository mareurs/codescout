---
id: '935743998158e226'
kind: bug
status: open
title: 'BUG: gc::apply_rehome changes an artifact id without updating artifact_chunk.artifact_id, which references it'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- catalog
- schema
- gc
opened: 2026-09-02
owner: marius
related:
- docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md
severity: medium
unverified: NOT REPRODUCED. The mechanism is composed from three facts each read at the bytes (FK without ON UPDATE, zero artifact_chunk references in gc.rs, defer_foreign_keys=ON), but no run has confirmed which of the two forks occurs — COMMIT failure or silent orphaning. They imply different severities and different fixes. See Resume.
---

## Summary

`gc::apply_rehome` changes an artifact's primary key (`UPDATE artifact SET id = ?1 … WHERE id = ?3`) and never updates `artifact_chunk.artifact_id`, which references it. The FK carries `ON DELETE CASCADE` but **no `ON UPDATE` clause**, so the update action is `NO ACTION`. Rehoming an artifact that has chunk rows therefore leaves children naming an id that no longer exists.

## Symptom (Effect)

Not yet observed at runtime — see § *Root cause* for why it is filed anyway, and § *Reproduction* for the check that would settle it. The predicted symptom is a **failure at COMMIT** rather than at the `UPDATE`, because `apply_rehome` sets `PRAGMA defer_foreign_keys = ON`:

```
FOREIGN KEY constraint failed
```

surfacing out of `librarian(action="doctor", fix="rehome", …)` at transaction commit, after every row has been rewritten.

## Reproduction

`git rev-parse HEAD` at filing: `a2a4090a`, branch `experiments`. Not yet run:

1. `Catalog::open_in_memory()`, `artifact::upsert` an artifact `a`.
2. `chunk::replace_chunks(&cat, "a", &chunk::build_chunks("a", "# T\n\n## W-1 — x\n\nalpha\n", 2048))` so `artifact_chunk` holds ≥1 row for `a`.
3. Drive `apply_rehome` so `a`'s id changes.
4. Expect the commit to fail, or — if it succeeds — expect `SELECT artifact_id FROM artifact_chunk` to name the dead id.

**Step 4 is the honest fork and it must be run before this file claims a symptom.** Both outcomes are defects, they are *different* defects, and they need different fixes: a COMMIT failure makes rehome unusable on any chunked catalog; a silent success makes it a data-corruption path that leaves orphaned chunks whose `artifact_vec_v2` rows are then unreachable by the cascade trigger.

## Environment

Linux, `experiments`, schema v11. Reached through `librarian(action="doctor", fix="rehome", old_root=…, new_root=…)`, which is the only production caller.

## Root cause

**Composed from three facts each verified at the bytes, not from a run.** Marked `unverified` in frontmatter for exactly that reason.

1. `src/librarian/catalog/mod.rs:262` — `artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE`. There is no `ON UPDATE` clause, so SQLite's default `NO ACTION` applies to parent-key updates.
2. `src/librarian/catalog/gc.rs` — `grep artifact_chunk` returns **0 matches** in the whole file, while `apply_rehome` rewrites `entry_cite`, calls `migrate_vec_id`, and then does `UPDATE artifact SET id = ?1, abs_path = ?2, missing_since = NULL WHERE id = ?3` (`:485-488`).
3. `src/librarian/catalog/gc.rs:453` — `PRAGMA defer_foreign_keys = ON`. This **defers** the constraint check to COMMIT; it does not disable it. FKs are enabled globally (`PRAGMA foreign_keys = ON` on all three `Catalog` constructors).

*Why it was not caught:* `artifact_chunk` is new in v11 (Task 4 of the chunk-grain plan). `apply_rehome` predates it, was correct when written, and nothing re-derives the list of child tables when one is added. `migrate_vec_id` exists precisely because someone once had to hand-handle a table the FK graph did not cover — so the file already contains the evidence that this class recurs, one table earlier.

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

Not implemented. The candidate is one statement in `apply_rehome`, before the parent update:

```rust
tx.execute(
    "UPDATE artifact_chunk SET artifact_id = ?1 WHERE artifact_id = ?2",
    rusqlite::params![row.new_id, row.old_id],
)?;
```

**But do not write it until § *Reproduction* step 4 has been run**, because the two outcomes imply different severities and the fix should ship with a regression test that observes the *actual* red rather than a predicted one. This project's own rule: run the reproduction before reading the fix plan.

**A second question the fix must answer, and it is the more interesting one:** rather than patching one child table, is the FK graph enumerable? A rehome that hand-lists children is the same shape as `migrate_vec_id` — a manual compensation for a relationship the schema already declares — and it will break again the next time a child table is added. `PRAGMA foreign_key_list(artifact)` returns the children at runtime. Whether that is worth it here is a design call, not a defect.

## Tests added

None yet. When the fix lands, the regression test must create chunk rows **before** the rehome and assert on `artifact_chunk.artifact_id` **after** it — asserting only that the rehome succeeds is monotone under the silent-corruption outcome.

## Workarounds

Do not run `librarian(action="doctor", fix="rehome")` on a catalog whose artifacts have been chunk-indexed until this is settled. The affected population is any catalog at schema v11 that has run `reindex` since Task 5 shipped (`e811ffd6`).

## Resume

Run § *Reproduction* steps 1–4 and record which fork actually occurs. That single result decides the severity, the fix, and the shape of the regression test; everything else in this file is already established.

## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` § *Task 7* — found while scouting that task; the plan's `gc.rs` instruction was wrong in two other ways, corrected there
- `src/librarian/catalog/gc.rs` `apply_rehome`, `migrate_vec_id`
- `src/librarian/catalog/mod.rs:257-262`, `:294-298`

