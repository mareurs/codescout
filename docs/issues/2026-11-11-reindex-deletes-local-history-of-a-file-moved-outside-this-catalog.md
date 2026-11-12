---
id: '99a4d0be1318c3c1'
kind: bug
status: fixed
title: 'BUG: reindex deletes a moved file''s local history when the move was not made in this catalog'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-11-12
opened: 2026-11-11
owner: marius
severity: medium
unverified: Not run on a rebuilt server after a real git pull. The external vector-store drop for kept rows is written (reindex tool and CLI path) but no test drives an external store with a kept vanished row; the in-memory chunk drop is tested. Whether any tool restores a catalog_audit image to the live tables was not checked. The number of kept rows a real pull produces was not measured.
---

# BUG: reindex deletes a moved file's local history when the move was not made in this catalog

## Summary

When a catalogued file's path disappears and `doc(action="move")` did not run in **this**
catalog, `librarian(action="reindex")` hard-deletes the old row in the same pass. Seven
tables cascade from that delete, so the row's events, augmentation, observations, links and
entry reservation left the live catalog on this machine. The append-only `catalog_audit` trail keeps
full images of the deleted `artifact`, `artifact_augmentation`, `events`, `artifact_link` and
`entry_cite` rows (the live catalog held 303 event and 15 augmentation delete images on
2026-11-12); observations, `entry_reservation` and `event_edges` are not in it. Reading or restoring
from the trail was not checked. The routine trigger is a `git pull` that brings
a move another host made, for example a bug file archived there.
`docs/conventions/cross-machine-catalog-resume.md` names this reindex "the repair" and does
not say that it deletes anything.

## Class fit

The class is a guard whose coverage is narrower than its name. The guard here is
`has_history` in `index_repo_sync`; its own comment asks "Does this row carry state that
exists nowhere else (an augmentation, or events)?", which describes every row with history.
But the code consults it only for rows that escape the root through a link. A row whose
file simply vanished carries the same state and is deleted without the check. It differs
from the other members because the narrow guard sits beside a convention page that calls
the unguarded branch a repair, so the documentation reinforces the gap instead of exposing it.

## Symptom (Effect)

Observed in an in-memory catalog test (2026-11-11); not observed on the live catalog. From the code: reindex reports the row under `removed: N`, and the
row's history is gone. The convention quotes such a run:

```
*2026-08-31: 4 added, 8 updated, **2 removed**, 12 embedded — the two removals were bug
files the other host had fixed and archived the same day.*
```

## Reproduction

Reproduced 2026-11-11 by `librarian::indexer::tests::a_vanished_file_with_history_keeps_its_events` (`src/librarian/indexer.rs`, marked `#[ignore]` because it is red by design). Run it with `scripts/with-slot.sh cargo test --lib a_vanished_file_with_history_keeps_its_events -- --ignored`; it fails with `left: 0, right: 1` at the final assertion, after its setup assertion confirmed the event existed. The test automates these steps:

1. In a scratch catalog, create an artifact and add one event (`doc(action="event_create")`).
2. Rename its file with plain `git mv` (simulating a move pulled from another host).
3. Run `librarian(action="reindex")`; then `doc(action="event_list")` on the old id and look
   for the event under any id.

## Environment

Linux, `experiments` at `53b3e9d75`, catalog `~/.local/share/librarian/catalog.db`.

## Root cause

- `src/librarian/indexer.rs:800-809` defines `has_history` as "an augmentation, or events".
- `src/librarian/indexer.rs:820` applies it only as `escaping && has_history(cand_id)`.
- `src/librarian/indexer.rs:811` sets `gone` for a missing file, and the branch from
  `:840` runs `DELETE FROM artifact WHERE id = ?1` (`:851-854`) for any `gone` row,
  whatever it carries.
- The artifact id is `sha256(abs_path)[:16]` (`src/librarian/ids.rs:17-23`), so a moved file
  is a new row and the old row is always `gone`.
- Measured 2026-11-11 against the live schema: `artifact_augmentation`, `artifact_chunk`,
  `artifact_link`, `artifact_observation`, `entry_reservation`, `event_edges` and `events`
  are `REFERENCES artifact(id) ON DELETE CASCADE`
  (`sqlite3 -readonly … "SELECT name FROM sqlite_master WHERE … sql LIKE '%REFERENCES artifact(id) ON DELETE CASCADE%'"`).

The deletion is measured for the `events` table (the test above). The other cascade tables are inferred from the schema and not measured.

## Evidence

### Population at risk

Measured 2026-11-11 (read-only catalog query, codescout rows outside worktrees): 1380 rows
carry events or an augmentation; 1052 of them are bug files under `docs/issues/`, the kind
another host archives most often.

## Hypotheses tried

1. **Hypothesis:** the escaping-row guard also covers vanished rows. **Test:** read
   `indexer.rs:810-860`. **Verdict:** rejected; the guard is consulted only when `escaping`.

## Fix

Option A, chosen by the operator on 2026-11-12, implemented in `b53baf521`: `index_repo_sync` keeps a vanished row that carries an augmentation or events and whose stored path is not risky (`stored_path_is_risky`). The row is cleared of everything its file supplied (`clear_kept_row`, renamed from `clear_escaping_row`, so a former escaping row whose link was removed leaks nothing), its chunk rows are dropped, and the report names it in `vanished_kept_ids` and `vanished_kept_paths`. The reindex response carries both plus `vanished_kept_note`, and the external vector store drops the kept rows' vectors. A kept row is not counted in `removed`.

The indexer does not stamp `missing_since`: the reconcile pass owns that column, stamps the row on its own schedule, and `find` hides it after the grace window. Nothing deletes a kept row. A file that returns is read again and the row recovers.

Not changed: a vanished row with no history is still removed; a row with a risky stored path (a dangling link, a lossy name) is still removed with its trail elided; a skipped duplicate is still removed. Not chosen: option B, which re-attaches the history to a row with the same content hash. The approved identity spec (`docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`) owns that rule.

The gate was green on 2026-11-12: FMT, CLIPPY, LEAN and DEFAULT lanes exit 0, Python 1836 passed.

## Tests added

- `librarian::indexer::tests::a_vanished_file_with_history_keeps_its_events` (the reproduction; `#[ignore]` removed)
- `librarian::indexer::tests::a_kept_vanished_row_is_reported_cleared_and_kept_on_the_next_run`
- `librarian::indexer::tests::a_vanished_row_with_only_an_augmentation_is_kept`
- `librarian::indexer::tests::a_vanished_row_without_history_is_still_removed` (control)
- `librarian::indexer::tests::a_kept_vanished_row_recovers_when_its_file_returns`
- `librarian::tools::reindex::tests::reindex_response_reports_a_kept_vanished_row`
- `librarian::tools::reindex::tests::reindex_response_has_no_vanished_note_when_nothing_vanished` (control)

## Fix provenance

- **SHA:** `b53baf521` (`experiments`)
- **patch-id:** `221fadba5fa8bec085e486ea7b0c51186c519037`

## Workarounds

Not needed on a build that contains `b53baf521`. On an older server binary, make moves through `doc(action="move")` on every host whose catalog holds history for the file: after a pull that brings a move, the moved file's local history is already at risk at the next reindex.

## Resume

Fixed in `b53baf521`. Open follow-ups: a live reindex after a real `git pull` on a rebuilt server; the external vector-store drop for kept rows, which no test drives; whether a tool can restore a `catalog_audit` image. Kept rows are never deleted, so they accumulate until the identity cutover exports them or an operator folds them into the moved file's row with `doc(action="graft")`.

## References

- `docs/conventions/cross-machine-catalog-resume.md` (§ fifth mode, § 2. Reindex the catalog)
- `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` § Binding a file to its row
- `docs/trackers/temp/artifact-identity-refactor.md`
