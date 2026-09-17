---
status: open
opened: 2026-09-17
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/blast-radius-exceeds-visibility
kind: bug
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

Not settled — recorded so the next session does not re-derive them:

- Re-point `entry_cite.src_slug` inside `graft_rows` before the delete, the way
  `repoint_history` already handles the other four child tables. Cheapest, matches the
  existing shape.
- Capture-and-replay: read the source's rows before the delete, re-insert after the slug
  lands. Loses `created_at` fidelity unless carried explicitly.
- `PRAGMA defer_foreign_keys` for the transaction, as `gc.rs:451` already does for a related
  ordering problem.

## Fix

Not yet fixed.

## Tests added

None yet. Note that `graft.rs`'s existing suite cannot catch this by construction — it never
creates an `entry_cite` row, so every assertion about what survives a graft is computed over a
population the defect cannot appear in.

## Workarounds

Re-run `librarian(action="link_scan", write=true)` after any `doc(action="move")` on a ledger
— that rebuilds the `origin='scan'` rows. It does **not** rebuild `origin='write'` rows; for
those, re-issue the `doc(action="append_entry", cites=[…])` calls that created them.

## Resume

Write a failing test in `src/librarian/catalog/graft.rs`'s test module: `art()` two rows, give
the source a slug, insert one `origin='write'` `entry_cite` row via
`entry_cite::insert_with`, run `graft_rows`, assert the row survives on the destination slug.
Observe the red, then decide between the three candidates above.

## References

- `src/librarian/catalog/graft.rs:88-100` — the delete/slug ordering and its comment
- `src/librarian/catalog/mod.rs:186-213` — the v9 block defining `entry_cite`
- `src/librarian/catalog/entry_cite.rs:16-26` — why `origin` sits outside the PK
- `src/librarian/tools/mv.rs:344-357` — `move`'s two-transaction shape and its `graft_rows` call
- Found while designing `rekey_prefix` for
  `docs/issues/2026-09-02-prefix-t-collides-again-with-the-zero-padding-protection-gone.md`,
  which needs the same `entry_cite` migration this omits.
