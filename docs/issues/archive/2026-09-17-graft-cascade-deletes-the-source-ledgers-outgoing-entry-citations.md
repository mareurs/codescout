---
kind: bug
status: fixed
tags:
- cluster/blast-radius-exceeds-visibility
closed: 2026-09-17
opened: 2026-09-17
owner: marius
related: []
severity: medium
---

# BUG: `graft` — and therefore `doc(action="move")` — cascade-deletes the source ledger's outgoing `entry_cite` rows

## Summary

`graft_rows` deletes the source artifact row and re-attaches its slug to the surviving
artifact. `entry_cite.src_slug` is `REFERENCES artifact(slug) ON DELETE CASCADE`, so that
delete destroys every outgoing entry-grain citation the source held. The slug is then written
onto the destination with the rows already gone. `origin='scan'` rows self-heal on the next
write-mode `link_scan`; **`origin='write'` rows do not, and nothing rebuilds them.**

## Symptom (Effect)

Archiving a tracker with `doc(action="move")` silently drops its hand-written outgoing
citations. Nothing errors. `move`'s return value reports `inbound_citations_cleared`,
`inbound_path_citations`, `inbound_id_citations` and `citation_stem_preserved` — an entire
citation-accounting block — and says nothing about the outgoing entry-grain edges it just
destroyed.

## Reproduction

`git rev-parse HEAD` → `c6f0027d`. Reproduced against the real schema, not the real catalog:

```sql
PRAGMA foreign_keys = ON;
CREATE TABLE artifact (id TEXT PRIMARY KEY, slug TEXT);
CREATE UNIQUE INDEX ux_artifact_slug ON artifact(slug);
CREATE TABLE entry_cite (
  src_slug TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local TEXT NOT NULL, dst_ref TEXT NOT NULL, rel TEXT NOT NULL,
  origin TEXT NOT NULL DEFAULT 'write', created_at INTEGER NOT NULL,
  PRIMARY KEY (src_slug, src_local, dst_ref, rel));
INSERT INTO artifact VALUES ('from','my-ledger'), ('into', NULL);
INSERT INTO entry_cite VALUES ('my-ledger','T-19','abc123','cites','write',1);
SELECT COUNT(*) FROM entry_cite;                          -- 1
DELETE FROM artifact WHERE id='from';                     -- graft.rs:94
UPDATE artifact SET slug='my-ledger' WHERE id='into';     -- graft.rs:97
SELECT COUNT(*) FROM entry_cite;                          -- 0
```

Measured 2026-09-17: `1` before, `0` after.

## Environment

codescout `experiments` @ `c6f0027d`, Linux, sqlite via rusqlite.

## Root cause

Three facts that only compose into a defect together, which is why none of them looks wrong
on its own:

- `src/librarian/catalog/mod.rs:200` — `src_slug TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE`.
- `src/librarian/catalog/mod.rs:600` — `PRAGMA foreign_keys = ON` on every connection, so the
  cascade is live.
- `src/librarian/catalog/graft.rs:94,97` — delete the source row, *then* write its slug onto
  the destination. The ordering comment at `:88-93` explains why the delete must come first
  (`slug` is UNIQUE, both rows cannot hold it at once mid-transaction) and is correct about
  that. It does not mention `entry_cite`, which is the table the ordering costs.

`doc(action="move")` inherits it: `src/librarian/tools/mv.rs:352` calls `graft_rows`.

**Measured, not inferred** — the SQL above, 2026-09-17. The mechanism was also read out of the
source; the reproduction is what makes it a fact rather than a reading.

## Evidence

`graft.rs` contains **no** occurrence of `entry_cite` — the table is not mentioned, not
re-pointed, and not tested. `repoint_history` (`graft.rs:112-198`) carefully re-points
`events`, `artifact_observation`, `artifact_link` and `event_edges`; `entry_cite` is absent
from that list.

Live blast radius at the time of writing — `origin='write'` rows are the unrecoverable half:

```
sqlite> SELECT origin, COUNT(*) FROM entry_cite GROUP BY origin;
scan|5340
write|13
```

All 13 durable rows sit on two artifacts: `open-issue-work-queue-bl-n` (9, `BL-21`…`BL-24`)
and `tool-usage-patterns` (4, `T-19`/`T-20`/`T-21`). Archiving either through
`doc(action="move")` destroys them.

## Hypotheses tried

1. **Hypothesis:** the cascade does not fire because `foreign_keys` defaults off.
   **Test:** grepped every pragma site. **Verdict:** rejected — `mod.rs:600` sets it `ON` on
   every connection, and the SQL reproduction above was run with it `ON`.
2. **Hypothesis:** scan-origin rows make this self-healing, so there is no data loss.
   **Test:** read `entry_cite.rs:19-26` and `:69-79`. **Verdict:** partially confirmed and
   therefore *not* a dismissal — `prune_scan_rows` re-derives `origin='scan'` rows on the next
   write-mode scan, but `origin='write'` rows are written only by `append_entry(cites=…)`
   (`augmentation.rs:863`) and no scan ever recreates them.

## Root-cause candidates for the fix

Settled by the fix, and recorded with what the reproduction did to each — because two of
the three were wrong in ways reading could not have separated.

- **Re-point `entry_cite.src_slug` inside `graft_rows` before the delete, the way
  `repoint_history` handles the other four child tables.** — **WRONG SHAPE.** There is
  nothing to re-point. The slug VALUE never changes; it moves from one artifact row to
  another, so the citation rows are correct before and after and are destroyed only in
  the window between. A re-point would rewrite rows that were never wrong.
- **Capture-and-replay: read the rows before the delete, re-insert after the slug
  lands.** — Workable, strictly worse. It copies rows that never needed to move, carries
  `created_at` only if done explicitly, and re-inserts into a PK the destination's own
  rows can already occupy, so it needs collision handling the chosen fix does not.
- **`PRAGMA defer_foreign_keys` for the transaction, as `gc.rs` does for a related
  ordering problem.** — **FALSE AS WRITTEN, measured.** With the delete-first ordering
  kept, the pragma leaves **0 rows**, exactly as before. `ON DELETE CASCADE` is an ACTION
  that fires at statement time; the pragma governs only when the CHECK runs. It is a
  necessary part of the fix and not the operative part of it.

**What the three share is how they were produced.** All were derived by reading — the
schema, `gc.rs`, `repoint_history` — and reading cannot separate *defers the check* from
*suppresses the action*, nor *the row must move* from *the row must survive*. One command
against real sqlite separated both. This is CLAUDE.md's *run the reproduction before
reading the fix plan* paying out on the file's own author: the plan is a hypothesis about
the reproduction, and here two thirds of it was wrong.

## Fix

Hand the slug over **before** the delete, in three steps under deferred foreign keys
(`graft_rows` step 6):

1. `PRAGMA defer_foreign_keys = ON` — makes step 2 legal, since between it and step 3 the
   child rows reference a slug no row holds.
2. `UPDATE artifact SET slug=NULL WHERE id=<from>` — the source gives the slug up.
3. `UPDATE artifact SET slug=<slug> WHERE id=<into>` — the UNIQUE index is now satisfied.

The `DELETE FROM artifact` then removes a row that no longer owns a slug, so there is
nothing to cascade. At commit the slug is held by the destination and every child
resolves again.

`GraftReport` gains `entry_citations_carried`, counted **after** the delete — the same
query before it reports rows FOUND, and *found, then destroyed by the next statement* is
this defect, so a pre-delete count is green in both worlds and could never be the number
that moves. `mv.rs`'s `history_grafted` is a hand-picked projection rather than a
serialization of the report, so the field is added there too — together with
`entry_reservations_folded`, absent since its own fix landed.

**That absence is the reason this bug is worth reading twice.** `entry_reservation` was
the FIRST table this same cascade silently emptied; it was found, fixed, and its count
never reached a caller. Nobody swept for siblings, and `entry_cite` was the sibling.

**Fixed:** `0370c1ab`
**patch-id:** `694c26639e0e64ccd16534109910357a6284e057`

## Tests added

- `librarian::catalog::graft::tests::graft_carries_durable_entry_citations_forward_not_only_the_slug`
  — the catalog-level guard. **Red observed before the fix**: `left: 0, right: 1`, while
  the sibling `graft_carries_the_slug_forward_so_a_move_does_not_orphan_it` stayed green
  in the same run. The pairing is the finding: the slug arrived and everything keyed by it
  was gone, and the sibling could not see that because it asks only where the slug went.
- `librarian::tools::mv::tests::move_carries_durable_outgoing_citations_and_reports_the_count`
  — the tool surface, because the number has to survive a second hand-written projection.
  This one **passed on its first run**, so its red was never observed and it is not
  evidence on that basis; mutation is what established that it discriminates.

Both carry an **inbound positive control**: `dst_ref` has no foreign key, so the cascade
cannot reach it. It does double duty — it is why a red can name its own cause instead of a
blanket wipe, and it is the second row that makes a *widened* count (2) distinguishable
from a correct one (1).

Five guarded sites, one mutation each, all KILLED, re-derived against the final bytes
after the count moved (a verdict measures bytes):

| site | verdict |
|---|---|
| NULL handover step | KILLED — `UNIQUE constraint failed` |
| `defer_foreign_keys` pragma | KILLED — `FOREIGN KEY constraint failed` |
| count predicate, arity-preserving `?1=?1` | KILLED — `left: 2, right: 1` |
| revert to delete-first ordering | KILLED |
| `mv.rs` projection field | KILLED — `left: Null, right: 1` |

The count mutation is arity-preserving deliberately: dropping the parameter instead breaks
rusqlite's binding and kills every test on an error, which is a verdict about plumbing
rather than about the predicate.

## Workarounds

Superseded by the fix — kept because a catalog written by a pre-`0370c1ab` binary still
needs them. Re-run `librarian(action="link_scan", write=true)` after any
`doc(action="move")` on a ledger; that rebuilds the `origin='scan'` rows. It does **not**
rebuild `origin='write'` rows — for those, re-issue the `doc(action="append_entry",
cites=[…])` calls that created them.

## References

- `src/librarian/catalog/graft.rs:88-100` — the delete/slug ordering and its comment
- `src/librarian/catalog/mod.rs:186-213` — the v9 block defining `entry_cite`
- `src/librarian/catalog/entry_cite.rs:16-26` — why `origin` sits outside the PK
- `src/librarian/tools/mv.rs:344-357` — `move`'s two-transaction shape and its `graft_rows` call
- Found while designing `rekey_prefix` for
  `docs/issues/archive/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md`,
  which needs the same `entry_cite` migration this omits.
