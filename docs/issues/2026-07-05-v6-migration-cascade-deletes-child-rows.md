---
id: '7b73a5a33df03e80'
kind: bug
status: fixed
title: v6 catalog migration cascade-deletes augmentation/events/links/observations (DROP TABLE under foreign_keys=ON)
owners: []
tags:
- librarian
- catalog
- migration
- data-loss
- sqlite
- foreign-keys
topic: null
time_scope: null
closed: '2026-07-05'
opened: '2026-07-05'
owner: marius
related:
- docs/issues/2026-07-02-tool-usage-patterns-augmentation-lost.md
severity: high
---


# v6 catalog migration cascade-deletes augmentation/events/links/observations (DROP TABLE under foreign_keys=ON)

## Summary
`migrate_v6::drop_legacy_and_stamp` rebuilds the `artifact` table with a
table-copy (`CREATE artifact_new` → copy → `DROP TABLE artifact` → rename) to
drop the legacy `repo`/`rel_path` columns. It ran while `PRAGMA foreign_keys =
ON` (set by `Catalog::open_with_workspace`). In SQLite, `DROP TABLE` under FK
enforcement performs an **implicit row-DELETE that invokes foreign-key
actions** — so dropping `artifact` fired `ON DELETE CASCADE` on every child
table (`artifact_augmentation`, `events`, `artifact_link`,
`artifact_observation`, `event_edges`) and deleted their rows. The copy carries
only `artifact` + `commits` forward, so all augmentations and event history for
**every artifact present when the migration ran** were silently lost. The
artifact rows themselves survived (re-inserted with original `created_at`), so
the loss is invisible until you notice an augmented tracker returns
`augmentation: null` with an empty event log.

## Symptom (Effect)
`docs/trackers/tool-usage-patterns.md` (id `f2ecdd76a6189efb`), documented in
CLAUDE.md as an augmented tracker, returned `augmentation: null` from
`artifact(get)` and `[]` from `artifact_event(list)`, while its body prose
(T-001…T-010) and its artifact row (original `created_at` 2026-06-02) were
intact. Tracked as the symptom in
`docs/issues/2026-07-02-tool-usage-patterns-augmentation-lost.md`.

## Reproduction
Empirical, minimal (Python stdlib sqlite3 — the exact table-copy pattern):
```
PRAGMA foreign_keys = ON;
CREATE TABLE artifact(id TEXT PRIMARY KEY, repo TEXT, kind TEXT);
CREATE TABLE artifact_augmentation(artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE, prompt TEXT);
CREATE TABLE events(id TEXT PRIMARY KEY, artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE, kind TEXT);
INSERT ... (1 artifact, 1 augmentation, 1 event);
BEGIN; CREATE artifact_new(...); INSERT SELECT; DROP TABLE artifact; ALTER TABLE artifact_new RENAME TO artifact; COMMIT;
```
→ before `aug=1 events=1`; after `artifact=1 aug=0 events=0`.

In-repo: `migration_v6_preserves_augmentation_and_events` (this file's fix)
FAILS on the pre-fix code with `augmentation must survive … left: 0 right: 1`.

## Environment
codescout with the librarian catalog at any schema < v6, opened by a build that
contains the v6 migration. Catalog DB is machine-local (`~/.local/share/librarian/catalog.db`
or `$LIBRARIAN_DB`); not in git, so there is no VCS record of the loss.

## Root cause
`src/librarian/catalog/mod.rs:199` sets `PRAGMA foreign_keys = ON`, then
`:203` calls `migrate_v6::drop_legacy_and_stamp`. That function's
`DROP TABLE artifact` (`src/librarian/catalog/migrate_v6.rs`) cascade-deletes
the child rows because SQLite's `DROP TABLE` implicit DELETE invokes FK actions
when foreign keys are enabled. The child tables all declare
`... REFERENCES artifact(id) ON DELETE CASCADE` (`src/librarian/catalog/schema.sql:117`
augmentation, `:58` events, `:21-22` links, `:30` observations, `:97`
event_edges). The migration author guarded against the trigger side effect
("DROP TABLE implicitly drops the artifact_vec_cascade_delete trigger") but
missed the FK-cascade side effect.

## Evidence
- Empirical sqlite3 repro above (before/after counts).
- `tool-usage-patterns` state: `augmentation: null`, events `[]`, `created_at`
  preserved (2026-06-02) — rules out delete+recreate (would reset created_at
  and not pre-populate then wipe events); consistent with cascade during a
  table-copy that re-inserts artifact rows.
- Selectivity: 3 trackers augmented AFTER the migration ran retain augmentation;
  the one augmented before (tool-usage-patterns) lost it — matches a one-time
  wipe at migration, not an ongoing bug.

## Hypotheses tried
1. **delete+recreate of the catalog row orphaned the augmentation.** Test:
   check `created_at`. Verdict: **rejected** — created_at is the original
   2026-06-02; a recreate would reset it.
2. **DROP TABLE under foreign_keys=ON cascade-deletes children.** Test: sqlite3
   repro + in-repo regression test. Verdict: **confirmed.**

## Fix
Wrap the table-copy in `PRAGMA foreign_keys = OFF` … `ON`, toggled **outside**
the `BEGIN`/`COMMIT` (SQLite ignores the pragma inside a transaction), with a
`ROLLBACK` on batch failure so the re-enable is honored. With FKs off, the
`DROP TABLE` is a bare drop and the child rows (whose ids still match the copied
artifact rows) survive. `src/librarian/catalog/migrate_v6.rs::drop_legacy_and_stamp`.

## Tests added
`migration_v6_preserves_augmentation_and_events` (migrate_v6 tests): seeds a
pre-v6 DB with an augmented artifact + an event, runs the full
`open_with_workspace` v6 migration, asserts both survive. Verified it FAILS on
the pre-fix code (`aug=0`) and PASSES with the fix; all 9 migrate_v6 tests +
clippy `-D warnings` green.

## Workarounds
Re-augment affected artifacts after the migration
(`artifact_augment(...)`); event history is unrecoverable (never persisted to
the .md file).

## Resume
Fixed. Note: catalogs already migrated to v6 before this fix have already lost
their pre-migration augmentations/events irrecoverably — this fix only protects
catalogs migrating v(<6)→v6 from here on. If a future table-copy migration is
added, apply the same foreign_keys-off discipline.

## References
- Symptom: `docs/issues/2026-07-02-tool-usage-patterns-augmentation-lost.md`
- `src/librarian/catalog/migrate_v6.rs` (drop_legacy_and_stamp)
- `src/librarian/catalog/mod.rs:199,203` (foreign_keys ON, then migration)
- `src/librarian/catalog/schema.sql` (ON DELETE CASCADE FKs)

