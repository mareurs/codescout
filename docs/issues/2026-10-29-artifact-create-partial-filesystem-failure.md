---
id: '22ae2baefdfdd73a'
kind: bug
status: mitigated
title: Failed artifact creation left a catalog row and an empty file
owners:
- codex
tags:
- cluster/unclassified
- librarian
- filesystem
closed: 2026-11-04
opened: 2026-10-29
severity: medium
unverified: A real disk-full or quota failure was not induced; the failure was injected with a read-only parent directory, which reproduces the leftover catalog row but not the empty file. The empty-file case is covered by a hand-built fixture. Linux only; the injection test needs a non-root user and unix mode bits.
---

# BUG: Failed artifact creation left a catalog row and an empty file

## Summary

During the compiler experiment, CLI document creation exited with an error but left both a live bug catalog row and a zero-byte file. Repeating creation refused the existing path. A later body-only catalog update restored the body with empty frontmatter, while catalog metadata still reported the original id and kind.

## Class fit

This is an unclassified catalog/filesystem consistency incident: the durable row and its file projection disagreed after a failed operation. It differs from the earlier orphan-file incident because this time the catalog row existed.

## Symptom (Effect)

The affected file was docs/issues/2026-10-29-compiler-harness-assumed-cargo-path.md, catalog id03cc35f248d78f73. A direct read measured zero lines. A semantic lookup found the fixed bug row. Body-only update produced an empty YAML frontmatter block; catalog get still exposed the original metadata.

## Reproduction

The observed CLI failure, duplicate-path refusal and zero-line read are retained in this session and private compile-mechanism-r2-20261029 evidence. An isolated filesystem-failure regression is still needed. Do not induce quota exhaustion against a real catalog to reproduce this.

## Root cause

Confirmed 2026-11-04 by reading `src/librarian/tools/create.rs` `call` and by a failing test. `create` writes the catalog row first (`upsert_and_mint_slug`, then the augmentation row), on purpose: BUG-058 moved the disk write after all catalog writes so a failed upsert would leave no orphan file. The disk write that follows is `std::fs::create_dir_all` then `std::fs::write`, and its error was returned with `?` and nothing else. `fs::write` opens with truncation and writes afterwards, so a write error after the open (a full disk, a quota) leaves an empty or partial file, and the row already in the catalog stays. The next create of the path is then refused as "path exists". That is the state the record observed. The cause of the original failure (quota exhaustion, per the compiler experiment's output) was not recovered; the child's stderr was never kept.

## Evidence

The file existed after the failed create; the next create refused path exists. The catalog returned the original row. Before restoration the file had zero lines; a body-only update wrote an empty frontmatter block without reconstructing id/kind. The root restored the file projection from retained body and the observed catalog metadata, then performed a normal catalog update and checked the header.

## Fix

Applied 2026-11-04 in `src/librarian/tools/create.rs`. When `create_dir_all` or `fs::write` fails, `call` now runs `undo_failed_create`: it deletes the catalog row (the augmentation, events and links go with it, `ON DELETE CASCADE`) and removes the file at the path, then returns the write's own error with the context "could not write <path>; the catalog row was taken back". Removing the file is safe because `call` refuses an existing path before it writes anything. The parent directory it may have created is left in place.

Second symptom, fixed in `5e211b0d9` (`experiments`, patch-id `caca9c27d379090075924082bf4ababa3c987ea7`): `doc(update)` read the file, found no frontmatter block, and re-serialized a default `Frontmatter` (`unwrap_or_default()` in the `patch.body` branch and in `rewrite_frontmatter_normalizing`). That wrote `---\n---` with no `id` or `kind` while the catalog row kept both. `update::call` now seeds the missing block with the row's `id` and `kind` before any branch runs, so the patch applies on top of it. Test: `updating_a_file_that_lost_its_frontmatter_restores_the_catalog_identity` (a body patch and a status patch on a zero-byte file). It failed before the change with the empty block `---\n---` and `id` None, and it passes after. The other 114 tests under `librarian::tools::update` still pass. Only `id` and `kind` are seeded: `status`, `title`, `owners` and `tags` stay in the catalog row and are not rebuilt in the file.

## Tests added

Two tests in `src/librarian/tools/create.rs`. Both were red against a no-op `undo_failed_create` and green after the fix.

- `a_failed_disk_write_leaves_neither_a_catalog_row_nor_an_augmentation`: the parent directory is made read-only (mode 0555), so `fs::write` fails after the row exists; it asserts that `artifact` and `artifact_augmentation` have no rows and the file is absent. It returns early when the process is root, because root ignores the mode bits.
- `undoing_a_failed_create_removes_the_row_and_the_partial_file`: a zero-byte file and a row are made by hand, the helper runs, both must be gone, and a second call must not fail.

The read-only directory makes `open` fail, so it reproduces the row left behind and not the empty file. The empty file is covered only by the second test, which builds it by hand. A real disk-full or quota failure was not induced: the record forbids inducing quota exhaustion against a real catalog.

## Resume

Open: a real disk-full or quota failure was not induced (see `unverified`). Nothing else is outstanding in this record.

## Fix provenance

- **SHA:** `b0fa1d5aa` (`experiments`)
- **patch-id:** `987ebb7fa121accd3eb7355adc2b592778dfef91`

Gate: `PYTHON_LIGHT=0 ./scripts/gate.sh` in a detached worktree at `b0fa1d5aa` exited 0 (FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0; 6010 and 8874 tests passed). The whole `librarian::` lib module also passed in the shared tree (2548 passed, 0 failed).

## References

- [Earlier inverse orphan-file case](archive/2026-05-09-artifact-create-orphan-file.md)
- The compiler experiment's evidence for the original failure exists only on the research branches (commit `6b70fbb1b`).
