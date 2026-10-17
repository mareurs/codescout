---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `doc(create)` checks `full.exists()` (false for a dangling link) and then `std::fs::write`s, so a dangling in-root symlink makes it create a file outside the root

## Summary

`src/librarian/tools/create.rs:476-481` refuses an existing path with `full.exists()`, which follows links and is
`false` for a dangling symlink, creates the parent directory, and writes with `std::fs::write(&full, &content)`,
which follows the link and creates its target. Measured: the file was created outside the root with the document's
content.

## Class fit

Same shape as the `doc(update)` leaf-link write (a librarian write that follows a link in a path built from
`base_dir.join(rel_path)`), and not the write-allowlist class: `doc(create)` consults no write classifier. Filed
unclassified for lack of a named class for a check-then-write that treats a dangling link as absent.

## Symptom (Effect)

`doc(create, rel_path="dangling.md")` returns an id and path, no error; a new file appears at the dangling link's
destination, outside the project root. It cannot overwrite an existing outside file: for a link to an existing
file `exists()` is `true` and the call refuses with `path exists`.

## Reproduction

Measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a` with a throwaway test in
`src/librarian/tools/update.rs` (removed afterwards), under `TempDir`s: `root/dangling.md` symlinked to
`outside/never-created.md` (absent), `mk_ctx(root)`, then
`create::call(ctx, {"repo":"r","rel_path":"dangling.md","kind":"spec","title":"T","body":"CREATED-BY-DOC-CREATE"})`:

```
PROBE doc create dangling: ok=true None
PROBE outside file created=true content_has_marker=true
```

## Environment

Linux, worktree branch cut from `experiments` at 65fc6d6a, `cargo test --lib`.

## Root cause

`create.rs:476` `if full.exists() { return Err(path exists) }` then `create.rs:481` `std::fs::write(&full, &content)?`.
`Path::exists` follows the link and reports `false` when the target is missing; `fs::write` is `File::create`,
which follows the link and creates the target. Measured 2026-10-17 (above).

## Evidence

The `create_file` tool does NOT have this defect: it routes through `atomic_write`, which renames over the link.
Measured the same day: `create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`
(`src/tools/edit_file/tests.rs`) includes a dangling link and asserts the outside file is never created.

## Hypotheses tried

None needed.

## Fix

Not applied. Candidate: check `symlink_metadata(&full).is_ok()` for the existence test, and write through
`atomic_write`.

## Tests added

N/A: filed, not fixed. The probe above becomes the regression test.

## Workarounds

None needed for the project's own tree; a repository that ships dangling `.md` links is the exposure.

## Resume

Add the probe as a red test, fix both the existence check and the write.

## References

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
