---
status: fixed
opened: 2026-05-09
closed: 2026-05-09
severity: medium
owner: marius
related: []
tags: ["artifact", "create", "side-effects", "orphan-file", "transaction"]
kind: bug
---

# BUG: `artifact(create)` left orphan file on disk when DB insert failed

## Summary

`artifact(create)` wrote content to disk *before* the DB insert. Any subsequent DB failure (e.g. `NOT NULL constraint failed: artifact.repo` during v6 pre-migration state) left the file orphaned. The next retry tripped the `if full.exists()` gate and returned `"path exists"` even though no row existed in the DB.

> Originally filed as BUG-055 on a parallel branch; renumbered on merge to avoid ID collision with the `edit_code` doc-comment-stripping BUG-055.

## Symptom (Effect)

- `artifact(create)` returned an error citing the DB constraint.
- File still on disk at the target path, no DB row.
- Subsequent `artifact(create)` calls for the same path failed with `"path exists"` — blocked retry even though the artifact was missing from the DB.

## Reproduction

Trigger any DB error after the disk write — e.g. install a `BEFORE INSERT` trigger on the `artifact` table that always raises (as the regression test does), then call `artifact(create)` with an `augment` block that fails its own validation.

## Environment

- Date: 2026-05-09
- Component: `crates/librarian-mcp/src/tools/create.rs::call`

## Root cause

Confirmed 2026-05-09: `call` wrote the file via `std::fs::write` *before* `artifact::upsert` (and the optional `augmentation::upsert`). Any DB error after the disk write left the file orphaned and blocked retry on the `if full.exists()` gate.

## Evidence

- DB error returned from `upsert`.
- File present on disk after the call returned `Err`.
- Retry blocked by `"path exists"`.

## Hypotheses tried

1. **Hypothesis:** Reorder so DB rows commit before the disk write. **Verdict:** Confirmed — adopted as the fix. The remaining (much rarer) failure mode — DB rows committed but the file write fails — is benign because `upsert` is idempotent: a retry rewrites the row and writes the file. **Evidence link:** see Fix.

## Fix

Applied 2026-05-09: Reordered `call` so the disk write is the last side effect — content is computed and `file_sha256` derived from in-memory bytes; both `artifact::upsert` and `augmentation::upsert` run first; only after both succeed does `std::fs::write(&full, &content)` happen. A DB error now leaves the disk untouched, so retry isn't blocked.

## Tests added

- `create_does_not_leave_orphan_file_when_upsert_fails` in `crates/librarian-mcp/src/tools/create.rs::tests` — installs a `BEFORE INSERT` trigger that always raises, then asserts no file remains after the call returns `Err`.

## Workarounds

Pre-fix: manually delete the orphan file before retry. The `"path exists"` error message named the path that needed cleanup.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-058** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Renaming note: originally **BUG-055** on a parallel branch; renumbered on merge to avoid ID collision with the `edit_code` doc-comment-stripping BUG-055.
