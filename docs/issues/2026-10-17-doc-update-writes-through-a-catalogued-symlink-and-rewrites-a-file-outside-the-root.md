---
status: open
opened: 2026-10-17
closed:
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `doc(update)` writes with `std::fs::write(row.abs_path)`, so a catalogued in-root symlink to a file outside the root has that file rewritten

## Summary

`src/librarian/tools/update.rs` writes the new document with `std::fs::write(&full, &new_content)` (line 775),
where `full = row.abs_path` (line 585). `fs::write` follows a symlink. The reindex deliberately catalogs an in-root
`.md` symlink that points OUTSIDE the root (`src/librarian/indexer.rs:398-425`), so `doc(update)` on such a row
rewrites the outside file. Measured: it did.

## Class fit

No existing class names a write that follows a link in a path the catalog, not the caller, supplied. It is not
the write-allowlist class: `doc(update)` takes an artifact id, not a path, so no write classifier is consulted at
all. Filed unclassified; it is the librarian-side sibling of the `edit_file` leaf-link result in the allowlist issue.

## Symptom (Effect)

A file outside the project root is rewritten (status, title, tags or a whole new body, whatever the patch says) and
the call returns `updated: true`. The in-root link is left in place, so nothing in the project changes.

## Reproduction

Measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a` (code of `3d190f55` for the librarian) with a
throwaway unit test in `src/librarian/tools/update.rs` (removed afterwards), all under `TempDir`s:
`outside/elsewhere.md` holding a document with frontmatter `status: draft`; `root/outward.md` a symlink to it;
`mk_ctx(root)`; the real `index_repo_sync` with a `**/*.md` rule; then
`update::call(ctx, {"id": artifact_id_from_abs(root/outward.md), "patch": {"status": "fixed"}})`. Output:

```
PROBE reindex added=1 row_exists=true
PROBE doc update: ok=true None
PROBE victim changed=true victim_now="---\nkind: spec\nstatus: fixed\ntitle: T\n---\n\nbody\n"; link still symlink=true
```

## Environment

Linux, worktree branch cut from `experiments` at 65fc6d6a, `cargo test --lib`.

## Root cause

`update.rs:585` `let full = row.abs_path.clone();` and `update.rs:775` `std::fs::write(&full, &new_content)?;`, with
no `symlink_metadata` check between them. `indexer.rs:398-425` keeps a symlink whose target resolves outside the
root (comment there: skipping it would trade a silent duplicate for a silent omission), so the row exists.
Measured 2026-10-17 (above).

## Evidence

Same defect class, read from the code and NOT run: `std::fs::write` on a catalog-supplied path at
`src/librarian/catalog/augmentation.rs:1023`, `:1953`, `:2222`, `src/librarian/catalog/rekey.rs:470`,
`src/librarian/tools/mv.rs:638`, `src/librarian/tools/doctor.rs:2042`,
`src/librarian/augmentation_sidecar.rs:178` and `:312` (line numbers from the review of this branch, not
re-read here).

## Hypotheses tried

None needed for the measured result.

## Fix

Not applied (out of scope for the `atomic_write` branch). Candidates: route these writes through
`util::fs::atomic_write`, which replaces a leaf link instead of writing through it (measured for `create_file` and
`edit_code`); or refuse a row whose `abs_path` is a symlink resolving outside the root.

## Tests added

N/A: filed, not fixed. The red test is the probe above, kept as a regression test when this is fixed.

## Workarounds

Do not keep `.md` symlinks that point outside the project root in a repository that uses the librarian.

## Resume

Add the probe as a red test, route the write through `atomic_write` (or refuse the row), then run the same probe
against each site in Evidence.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
