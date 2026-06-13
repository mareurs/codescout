---
status: open
opened: 2026-06-13
closed:
severity: medium
owner: marius
related: []
tags: [librarian, catalog, doctor]
kind: bug
---

# BUG: Catalog rows orphaned by a repo rename/move are not migrated, and `doctor` detects but cannot safely prune them

## Summary
Renaming or moving a registered repo's directory leaves dead rows in **two**
catalog tables — `artifact` (keyed by `abs_path`, `id = sha256(abs_path)`) and
`commits` (keyed by `git_root`) — with no migration to the new path. `doctor`
*detects* the dead artifact rows (`missing_file`) but offers no safe, scoped
*fix*; the only built-in prune is the unsafe global `reindex(scope="all")`
path ([[2026-06-13-delete-orphan-repos-cross-workspace-wipe]]). Manual cleanup
is also blocked: the bare `sqlite3` CLI cannot mutate the catalog because the
`artifact_vec` cascade trigger requires the `vec0` extension **and**
`trusted_schema=ON`.

## Symptom (Effect)
After `/home/marius/work/claude/code-explorer` was renamed to `…/code-explorer.old`
(and a fresh `codescout` repo took its place), the catalog retained:
- **603** dead `artifact` rows under `/home/marius/work/claude/code-explorer/…`
  (the repo's own files — CLAUDE.md, docs/adrs, …), surfacing as 603
  `missing_file` violations in `doctor`.
- **1940** dead `commits` rows with `git_root = /home/marius/work/claude/code-explorer`.
- `artifact(repo="code-explorer")` still resolved the phantom repo.

`doctor`'s `commits` check only validates git_root *form*, not existence, so
the 1940 dead commits were not even reported.

## Reproduction
1. `reindex` a repo at path `P` (registers artifact + commits rows under `P`).
2. Rename/move `P` → `P2` on disk; register/activate `P2`.
3. Catalog still holds `P`-rooted artifact + commits rows; `doctor` shows the
   artifacts as `missing_file`; there is no built-in command to prune just `P`.

## Environment
codescout MCP server, 2026-06-13. Global catalog
`~/.local/share/librarian/catalog.db` (WAL, schema v6). System `/usr/bin/sqlite3`
built **without** the `vec0` module.

## Root cause
- **No rename migration.** Catalog identity is path-derived
  (`artifact.id = sha256(abs_path)` per `src/librarian/ids.rs`; `commits.git_root`).
  A directory rename changes the path but nothing rewrites the rows, so the old
  identity persists as dead rows and a fresh set is created for the new path.
- **No safe scoped prune.** `librarian(action="doctor")` is read-only
  (`src/librarian/...` doctor). The only deletion path is
  `delete_orphan_repos` via `reindex(scope="all")`, which is unsafe on the
  shared global catalog ([[2026-06-13-delete-orphan-repos-cross-workspace-wipe]]).
- **CLI surgery blocked.** `schema.sql` defines
  `CREATE TRIGGER artifact_vec_cascade_delete AFTER DELETE ON artifact …
  DELETE FROM artifact_vec …`. `artifact_vec` is a `vec0` virtual table, so any
  `DELETE FROM artifact` from the bare CLI fails first with
  `no such module: vec0`, then (once the extension is loaded) with
  `unsafe use of virtual table "artifact_vec"` unless `PRAGMA trusted_schema=ON`.

## Evidence
- This session's `doctor scope=all`: 742 `missing_file` violations, 603 under
  the dead `code-explorer` path; after cleanup, 139 (all other workspaces).
- Cleanup required hand-compiling the upstream `sqlite-vec.c`
  (`~/.cargo/registry/src/.../sqlite-vec-0.1.9/sqlite-vec.c`) into a loadable
  `vec.so`, then `sqlite3 … ".load /tmp/vec.so sqlite3_vec_init"` plus
  `PRAGMA trusted_schema=ON; PRAGMA foreign_keys=ON;` before the scoped
  `DELETE FROM artifact WHERE abs_path LIKE '…/code-explorer/%'` (603 rows,
  cascades verified: 9 augmentations + 1 event + embeddings) and
  `DELETE FROM commits WHERE git_root = '…/code-explorer'` (1940 rows).

## Hypotheses tried
N/A — diagnosed directly from schema + catalog state.

## Fix
Plan (not implemented). Either or both:
1. **Migration on rename/move.** A `librarian(action="move_repo"/"rename_repo")`
   (or `reindex` detecting a moved root) that rewrites `abs_path` / `git_root`
   for the old root to the new one, preserving ids/events where possible.
2. **vec0-aware scoped prune.** Extend `doctor` with an opt-in fix
   (e.g. `doctor(fix="prune_missing", root=<path>)`) or an `artifact` bulk
   delete-by-path-prefix, executed through codescout's own (vec0-linked,
   trusted-schema) connection so the cascade trigger works — removing the need
   for hand-compiled-extension surgery.

## Tests added
None yet. When fixed: regression test that a moved/renamed root's rows are
migrated (option 1) or prunable via a scoped, vec0-aware path (option 2),
asserting cascade to `artifact_augmentation`/`events`/`artifact_vec`.

## Workarounds
Manual surgical cleanup with a backup (used this session):
```bash
# 0. consistent backup
sqlite3 catalog.db ".backup 'catalog.db.bak'"
# 1. compile the loadable vec0 extension from the cargo source
gcc -O2 -fPIC -shared <…>/sqlite-vec-0.1.9/sqlite-vec.c -o /tmp/vec.so -I/usr/include
# 2. scoped delete (trailing slash excludes <root>.old); cascades fire with FK on
sqlite3 catalog.db ".load /tmp/vec.so sqlite3_vec_init" \
  "PRAGMA trusted_schema=ON; PRAGMA foreign_keys=ON;
   DELETE FROM artifact WHERE abs_path LIKE '<dead-root>/%';
   DELETE FROM commits  WHERE git_root  =    '<dead-root>';"
```

## Resume
Decide between fix option 1 (rename migration) and 2 (vec0-aware scoped prune);
option 2 also resolves the "doctor detects but can't fix" gap. Implement in the
librarian tools layer (`src/librarian/tools/`), routing the DELETE through the
server's vec0-linked connection (trusted schema already enabled there).

## References
- `src/librarian/catalog/schema.sql` — `artifact_vec` vec0 vtable + the
  `artifact_vec_cascade_delete` trigger; FK `ON DELETE CASCADE` on
  `artifact_link`/`artifact_observation`/`events`/`event_edges`/`artifact_augmentation`.
- `src/librarian/ids.rs` — `id = sha256(abs_path)`.
- Sibling: [[2026-06-13-delete-orphan-repos-cross-workspace-wipe]] (why the
  built-in prune is unsafe). CLAUDE.md § "Querying active trackers" (rename →
  reindex mints new ids / orphans events).
