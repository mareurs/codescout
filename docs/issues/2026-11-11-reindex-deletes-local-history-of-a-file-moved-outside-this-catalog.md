---
id: '99a4d0be1318c3c1'
kind: bug
status: open
title: 'BUG: reindex deletes a moved file''s local history when the move was not made in this catalog'
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-11-11
owner: marius
severity: medium
unverified: 'The deletion path is read from code and the cascade from the live schema; no reproduction was run. Not checked: what reindex re-attaches afterwards from an expects_augmentation sidecar (shape only, never params).'
---

# BUG: reindex deletes a moved file's local history when the move was not made in this catalog

## Summary

When a catalogued file's path disappears and `doc(action="move")` did not run in **this**
catalog, `librarian(action="reindex")` hard-deletes the old row in the same pass. Seven
tables cascade from that delete, so the row's events, augmentation, observations, links and
entry reservation are lost on this machine. The routine trigger is a `git pull` that brings
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

Not observed at runtime. From the code: reindex reports the row under `removed: N`, and the
row's history is gone. The convention quotes such a run:

```
*2026-08-31: 4 added, 8 updated, **2 removed**, 12 embedded — the two removals were bug
files the other host had fixed and archived the same day.*
```

## Reproduction

Not yet reproduced — best lead, all read-only except the scratch catalog:

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

Inferred from the code; not measured at runtime.

## Evidence

### Population at risk

Measured 2026-11-11 (read-only catalog query, codescout rows outside worktrees): 1380 rows
carry events or an augmentation; 1052 of them are bug files under `docs/issues/`, the kind
another host archives most often.

## Hypotheses tried

1. **Hypothesis:** the escaping-row guard also covers vanished rows. **Test:** read
   `indexer.rs:810-860`. **Verdict:** rejected; the guard is consulted only when `escaping`.

## Fix

Two candidates; not decided.

- Short term: do not hard-delete a `gone` row that `has_history`; stamp `missing_since` and
  report it, as the escaping branch already keeps such rows.
- Structural: `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`
  (draft v3) gives rows a uid that survives moves and rebinds a pulled move by key or through
  `.codescout/aliases.tsv`, which removes the trigger for folder moves and archives.

## Tests added

None yet; the bug is open.

## Workarounds

Make moves through `doc(action="move")` on every host whose catalog holds history for the
file. After a pull that brings a move, the local history of the moved file is already at
risk at the next reindex.

## Resume

Run the reproduction above in a scratch catalog to turn the inferred mechanism into a
measured one; then decide between the two fix candidates with the identity spec's owner.

## References

- `docs/conventions/cross-machine-catalog-resume.md` (§ fifth mode, § 2. Reindex the catalog)
- `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` § Binding a file to its row
- `docs/trackers/temp/artifact-identity-refactor.md`
