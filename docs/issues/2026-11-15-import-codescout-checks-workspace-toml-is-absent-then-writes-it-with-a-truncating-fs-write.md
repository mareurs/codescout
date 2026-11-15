---
id: e0a1ec8d7f0e76f9
kind: bug
status: open
title: 'BUG: librarian import_codescout checks that workspace.toml is absent and then writes it with a truncating std::fs::write'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-15
severity: low
unverified: 'Read from the code, not reproduced. Established: the exists check at L499 and the std::fs::write at L597 are separate calls in the same function. Not established: that anyone has run two imports together; the path is CLI only.'
---

# BUG: librarian import_codescout checks that workspace.toml is absent and then writes it with a truncating std::fs::write

## Summary
`import_codescout` (`src/librarian/mod.rs`, the CLI import of a codescout registry) refuses when `ws_path.exists()` (`src/librarian/mod.rs:499`) and later writes the serialised workspace with `std::fs::write(ws_path, &toml_str)` (`src/librarian/mod.rs:597`). The two calls are separate, so a `workspace.toml` that appears between them is truncated and replaced. The write also truncates before it writes, so a failed write leaves a short or empty file. `exists()` follows a link and answers false for a dangling one, so the write goes through such a link and creates its target.

## Class fit
cluster/unclassified: the same check-then-write shape as the `create_file`, `doc(move)` and `doc(create)` bugs fixed or filed on 2026-11-14, on a CLI-only path. No existing class names a path replacement that follows a separate existence check.

## Symptom (Effect)
Two imports run together, or an import runs while a person writes `workspace.toml`. Both pass the check at `:499`. The second write replaces the first file, and both callers print `imported N roots`. The refusal "workspace.toml already exists ... Merge manually." exists to stop this loss, and it does not hold in the window.

## Reproduction
Read path only: `import_codescout` -> `ws_path.exists()` (L499) -> read of the registry and the build of roots and rules -> `create_dir_all` -> `std::fs::write` (L597). The build between the check and the write widens the window.

## Environment
Branch `experiments` at `1c6ec18e3`. Found by the whole-branch review of the no-clobber publish plan (`docs/superpowers/plans/2026-11-15-no-clobber-publish.md`).

## Root cause
The check and the write are two calls and the write replaces by design. The helper that closes this exists now: `crate::util::fs::write_new_utf8`, which publishes with a hard link and fails with `AlreadyExists` when anything is at the path. (inferred from the code, not measured)

## Fix
Replace the `std::fs::write` with `write_new_utf8(ws_path, &toml_str)`. On `AlreadyExists` (with `symlink_metadata(ws_path).is_ok()`) return the same "already exists ... Merge manually." error. Keep the early check for the common case. Test through the seam `crate::util::fs::set_before_publish`: a file planted between the check and the write stays unchanged and the import fails with the message above.

## Tests added
None yet.

## Resume
Open. Not started. A small change with the existing helper; the test uses the seam as `create_file` does (`src/tools/create_file.rs`, module `tests`).

## References
- `src/librarian/mod.rs:499`, `src/librarian/mod.rs:597`
- `src/util/fs.rs` (`write_new_utf8`, `set_before_publish`)
- a2baa3ad8e32cff9 (the `create_file` sibling), 4b345c816297115b (the `doc(move)` sibling), 5f7254ff59295050 (the `doc(create)` sibling)
