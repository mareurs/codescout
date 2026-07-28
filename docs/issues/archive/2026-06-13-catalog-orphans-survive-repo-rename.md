---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- catalog
- doctor
topic: null
time_scope: null
closed: 2026-06-14
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

**Shipped on `experiments` in `4d3f32cd`** (`feat(doctor): add fix=prune_missing to safely prune a dead repo root from the catalog`). Not yet on `master` — archive after cherry-pick, cite the master-side SHA then.

Implemented **option 2 (vec0-aware scoped prune)**. New opt-in repair on the doctor tool:
`librarian(action="doctor", fix="prune_missing", root="<absolute dead-root path>")` deletes every `artifact` row whose `abs_path` is `root` or under `root/`, and every `commits` row whose `git_root` is `root` or under `root/`, through codescout's own (vec0-linked, trusted-schema) connection — so the `artifact_vec` cascade trigger and the FK `ON DELETE CASCADE`s (augmentation / links / events) all fire. This retires the hand-compiled-`vec0`-extension sqlite3 surgery in Workarounds. Gated for safety: `root` must be absolute and must **not** exist on disk (a live root's rows are not orphans; per-file deletion is reindex's job). Default `doctor` stays read-only. `src/librarian/tools/doctor.rs` (`run_fix` / `validate_prune_request` / `prune_dead_root`), `src/librarian/tools/librarian.rs` (schema + description).

**Deferred (option 1):** auto-migration on rename (rewrite `abs_path` / `git_root` for a moved root, preserving ids/events) — a rename still orphans rows; the prune is the sanctioned cleanup. Also deferred: a `missing_git_root` *detection* check so dead commits rows are auto-reported, not just prunable-once-known.
## Tests added

`src/librarian/tools/doctor.rs` tests:
- `prune_dead_root_removes_rows_under_root_only` — deletes exact-root + nested artifact rows + the matching commits row; asserts a path-PREFIX sibling (`/gone/repo-other`) and an unrelated live row are NOT matched.
- `validate_prune_request_gates` — unknown fix, missing root, relative root, and a still-existing root (`/tmp`) are all refused; a dead absolute root is accepted.

Full lib suite 2737 pass; clippy `-D warnings` clean. Cascade-to-augmentation is covered by `delete.rs`'s existing cascade tests over the same connection.
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
