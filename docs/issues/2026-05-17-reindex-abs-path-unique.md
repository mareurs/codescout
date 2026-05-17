---
status: fixed
opened: 2026-05-17
closed: 2026-05-17
severity: high
owner: marius
related: []
tags: ["librarian", "reindex", "sqlite", "unique-constraint", "upsert"]
kind: bug
---

# BUG: `librarian(reindex)` failed with `UNIQUE constraint failed: artifact.abs_path`

## Summary

Calling `librarian(action="reindex", scope="project")` on this project returned `UNIQUE constraint failed: artifact.abs_path`. The default (non-force) path failed immediately; no rows updated. Root cause: `artifact::upsert` only handled `ON CONFLICT(id)`, not the schema's `abs_path UNIQUE` constraint. Fixed by pre-cleaning `DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2` before the INSERT.

## Symptom (Effect)

```
librarian(action="reindex", scope="project")
→ Err: "UNIQUE constraint failed: artifact.abs_path"
```

The default (non-force) reindex path failed immediately; no rows updated.

## Reproduction

Pre-fix state: any project whose catalog had prior rows whose `abs_path` collided with what the current walk tried to insert. Force the collision by re-walking a project after a path normalization change (trailing slash, symlink resolution, etc.) — two artifact rows for the same logical file.

Repro on a clean catalog: not yet established.

## Environment

- Date observed: 2026-05-17
- Tool: `mcp__codescout__librarian(action="reindex")`
- Component: `crates/librarian-mcp/src/catalog/artifact.rs::upsert`

## Root cause

`artifact::upsert` only handled `ON CONFLICT(id)`, not the schema's `abs_path UNIQUE` constraint. Two paths could lead to the same `abs_path` but different `id`s (path normalization changes, prior failed walks leaving orphan rows), and the upsert had no codepath for that collision.

## Evidence

- Reproduced live on this project's catalog (4 augmented artifacts, ~497 files) — see session log `docs/trackers/archive/artifact-code-linkage-session-log.md` F-6.
- Post-fix verification: `reindex(scope=project)` returns `unchanged: 493, backfill_error_count: 0`.

## Hypotheses tried

1. **Hypothesis:** Path normalization (trailing slash, symlink resolution) producing two rows for the same logical file. **Test:** Inspected `artifact` table for duplicates. **Verdict:** Confirmed — duplicates existed under different normalized paths. **Evidence link:** session log F-6.
2. **Hypothesis:** Pre-clean conflicting `abs_path` rows before INSERT. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.

## Fix

Fixed by commit `d482ca8a` (2026-05-17). `artifact::upsert` (`crates/librarian-mcp/src/catalog/artifact.rs`) now pre-cleans `abs_path` UNIQUE conflicts:

```rust
cat.conn.execute(
    "DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2",
    params![row.abs_path.to_string_lossy().as_ref(), row.id],
)?;
```

before the INSERT. Verified post-rebuild — `reindex(scope=project)` now succeeds.

## Tests added

*N/A in commit description; the fix shipped alongside two other reindex fixes in the same commit (`d482ca8a`). Recommend `upsert_resolves_abs_path_collision_with_different_id` as a regression.*

## Workarounds

Pre-fix: catalog was read-only for reindex until both #5 and #6 (embedding dim mismatch on `force=true`) were fixed.

## Resume

N/A — fixed. After commit lands on master, move this file to `docs/issues/archive/`.

## References

- Originally tracked as **#5** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Session log: `docs/trackers/archive/artifact-code-linkage-session-log.md` F-6.
- Fix commit: `d482ca8a` on `experiments`.
- Related: bug-tracker.md #6 (embedding dim mismatch), #7 (cascade-delete data loss) — same commit fixes all three reindex failure modes.
