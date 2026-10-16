---
id: '63dbc4b5331f356e'
kind: bug
status: fixed
title: graft onto a destination whose slug has durable citations fails at COMMIT with a bare FK error
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-09-17
closed: 2026-10-16
related: []
severity: low
---

## Summary

`graft_rows` writes `from_id`'s slug over `into_id`'s. `entry_cite.src_slug` is
`REFERENCES artifact(slug)` with an `ON DELETE CASCADE` and **no `ON UPDATE` clause**, so
the default `NO ACTION` applies: rows keyed by the *displaced* slug reference a value no
artifact row holds, and the transaction fails at `COMMIT` with a bare
`FOREIGN KEY constraint failed`.

It rolls back whole, so there is **no data loss** — the cost is an unactionable error.

## Symptom (Effect)

An opaque `FOREIGN KEY constraint failed` (sqlite code 787) naming no table, no column and
no row, surfaced from a `COMMIT` rather than from the statement that caused it. A reader
has nothing to grep for.

## Reproduction

Against the real schema, at the ordering shipped in `0370c1ab`:

```sql
PRAGMA foreign_keys = ON;
INSERT INTO artifact VALUES ('from','my-ledger'), ('into','other');
INSERT INTO entry_cite VALUES ('my-ledger','T-19','abc','cites','write',1);
INSERT INTO entry_cite VALUES ('other','F-1','xyz','cites','write',1);
BEGIN;
PRAGMA defer_foreign_keys = ON;
UPDATE artifact SET slug=NULL WHERE id='from';
UPDATE artifact SET slug='my-ledger' WHERE id='into';   -- displaces 'other'
DELETE FROM artifact WHERE id='from';
COMMIT;                                                  -- FOREIGN KEY constraint failed
```

Measured 2026-09-17: error at `COMMIT`, both `entry_cite` rows intact after rollback.
Under the *pre*-`0370c1ab` ordering the same case failed one statement earlier, at the
`UPDATE`; the failure moved, its character did not.

## Environment

codescout `experiments` @ `0370c1ab`, Linux, sqlite via rusqlite.

## Root cause

`entry_cite.src_slug` declares `ON DELETE CASCADE` and nothing for `ON UPDATE`. The
handover overwrites a parent key, which is an update, not a delete.

## Evidence

**No current caller can reach this, and that is the finding rather than a dismissal.**
Both in-tree callers seed the destination with plain `artifact::upsert`, so `into_id`
arrives with a NULL slug and nothing is displaced:

- `src/librarian/tools/mv.rs` — `artifact::upsert`, then `graft_rows`.
- `src/librarian/tools/merge_worktree.rs` — `reseat_one` does the same. Its sibling
  `merge_one` never calls `graft_rows` at all, by an invariant stated in that module's
  own header.

`artifact::upsert_and_mint_slug`'s doc comment is explicit that it is **"Not for
`mv`/`graft_rows`"**, for an unrelated reason (a premature mint would hand the new row a
needless `-2` suffix, permanently). So the precondition holds today as a **side effect of
a decision made about slug suffixes** — nothing states it, tests it, or would notice it
changing.

## Why this is filed and not fixed

**Superseded 2026-10-16: the premise was wrong, so the law it invoked does not apply.** This section argued that no caller reaches the case. That held for the two callers the file enumerated (`mv.rs`, `reseat_one`) and missed a third, `src/librarian/tools/graft.rs` — `doc(action="graft")` — which passes two **caller-supplied** ids straight to `graft_rows`. Any two established artifacts reach it. Verified at the bytes: `graft_rows` called on two rows that each hold a slug, the destination's with `entry_cite` children, returns `FOREIGN KEY constraint failed: Error code 787` (red test `graft_onto_an_established_slug_with_citations_refuses_actionably_and_changes_nothing`, observed failing before the fix). So the guard sits on a reachable path, which is exactly the condition this section set for promoting it.

## Hypotheses tried

1. **Hypothesis:** `ON DELETE CASCADE` also covers the update, so the displaced slug's
   rows follow it. **Test:** the SQL above. **Verdict:** rejected — the cascade clause is
   delete-only; the update takes the `NO ACTION` default and refuses.
2. **Hypothesis:** `doc(action="move")` reaches this, since it is the archive route.
   **Test:** read both `graft_rows` call sites and `upsert_and_mint_slug`'s doc comment.
   **Verdict:** rejected — a move seeds the destination slug-less. This is what turned the
   file from a live bug into a latent precondition.

## Fix

`graft_rows` (`src/librarian/catalog/graft.rs`) now calls `refuse_if_handover_would_strand_citations` as step 0, inside the `IMMEDIATE` transaction and before any write. It refuses with a `LibrarianRecoverableError` naming the displaced slug, the count of durable citations it would strand, both ids, and the remedy (graft the other way so the slug that owns citations survives). It refuses **only** the case that failed — both rows hold differing slugs and the destination's slug has `entry_cite` children — so an established destination slug with no children is still overwritten as before (pinned by a control test).

Deliberately NOT done: re-pointing the displaced slug's rows (the `rekey.rs` style handler). Which slug survives a graft between two established artifacts is a product choice, and a refusal loses nothing. The `graft_rows` doc comment was rewritten from "precondition satisfied by accident" to describe the refusal.

## Tests added

In `src/librarian/catalog/graft.rs`:

- `graft_onto_an_established_slug_with_citations_refuses_actionably_and_changes_nothing` — red before the fix (opaque FK error), asserts the message names the slug and count, is a `LibrarianRecoverableError`, and that both rows, the source's events and the citations are untouched.
- `graft_onto_an_established_slug_without_citations_still_succeeds` — control: the refusal is no wider than its cause.

## Workarounds

N/A once fixed. Before the fix: graft in the other direction, or only graft onto a destination with no slug / no durable citations.

## References

- Found while fixing
  `docs/issues/archive/2026-09-17-graft-cascade-deletes-the-source-ledgers-outgoing-entry-citations.md`,
  by an experiment run to settle that bug's remedy rather than by reading this code.
- `src/librarian/catalog/mod.rs` — the v9 block defining `entry_cite`
- `src/librarian/catalog/graft.rs` — the handover and its stated precondition
