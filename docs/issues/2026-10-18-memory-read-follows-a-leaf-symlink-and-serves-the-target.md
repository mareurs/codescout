---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-18-opening-the-private-memory-store-copies-a-gitignore-symlink-targets-bytes-into-the-project.md, docs/issues/2026-10-14-pi-memory-write-guard-reencodes-invalid-utf8-before-backup.md]
tags:
- cluster/unclassified
unverified: 'Windows behaviour was not run. The check is symlink_metadata per component then read_to_string, so a link swapped in between the two calls is not closed (the same window WriteTransaction has). anchors::hash_file still reads project paths named inside a sidecar (project_root joined with the anchor path) and was not changed: it reveals a SHA-256, not content. The SQLite semantic memory database file was not examined for a leaf link. The fix SHA is on the review branch and may not survive a rebase: use the patch-id.'
kind: bug
---

# BUG: `MemoryStore::read` follows a leaf (or parent) symlink, so `memory(action="read")` serves any file a memory topic links to

## Summary

`MemoryStore::read` (`src/memory/mod.rs`, 129-136 before the fix) gated on `path.exists()` and returned
`read_to_string(path)`, both following links. `WriteTransaction` already refuses every non-regular topic
(`src/memory/write_transaction.rs:68`, "memory target must be a regular file") and every linked parent
(`safe_topic_parent`), so the read side was the only direction left open. A repository can ship
`.codescout/memories/arch.md -> ~/.ssh/id_rsa` (git carries symlinks); `memory(action="read", topic="arch")`
then returns the key. The same holds for the private store, the dashboard `get_memory` handler and the
onboarding reader, which all call `read`.

## Class fit

The write path has the guard and the read path lacks it for the same shape of input, which is close to a
guard narrower than the operation it protects, but no existing class names a pair of directions where only
one is guarded. Filed unclassified rather than forced into a class whose trait it shows only in part.

## Symptom (Effect)

Reviewer evidence 2026-10-18: `read("arch")` on `arch.md -> home/.ssh/id_rsa` returned `Ok(Some("PRIVATE"))`.
Through the tool dispatch, before the fix: `Object {"content": String("PRIVATE")}`.

## Reproduction

Measured 2026-10-18 on branch `fix/memory-leaf-links-2026-10-18` at `0a8af94d7`, before the fix, by new tests
under `tempdir()` (no real home). Red lines (12 failing of the 16 tests added by this fix and run in the `memory::` filter;
the other four pin behaviour that was already correct):

```
memory::tests::read_refuses_a_leaf_symlink_to_a_file_inside_the_store: must be refused: Some("inside")
tools::memory::tests::read_refuses_a_topic_that_is_a_symlink_to_a_secret: a linked topic must not be served: Object {"content": String("PRIVATE")}
memory::tests::read_refuses_a_directory_at_the_topic_path: must be a RecoverableError, got: Is a directory (os error 21)
memory::anchors::tests::read_anchor_file_refuses_a_symlinked_sidecar: must be a RecoverableError, got: TOML parse error at line 1, column 16 | 1 | SECRET-BYTES = not toml
test result: FAILED. 197 passed; 12 failed; 0 ignored; 0 measured; 7218 filtered out
```

The anchor line shows a second leak: a linked `<topic>.anchors.toml` that is not TOML produces a parse error
that quotes the target's line.

## Environment

Linux, `rustc 1.97.1`, branch above.

## Root cause

`src/memory/mod.rs` `MemoryStore::read` (pre-fix 129-136): `path.exists()` follows the leaf link, and
`std::fs::read_to_string` follows it again. Nothing inspected the final component or any parent with
`symlink_metadata`. `src/memory/anchors.rs` `read_anchor_file` had the same shape (`read_to_string` of a
sidecar path), and a link there is parsed as the topic's anchor set. Measured 2026-10-18: red lines above.

## Evidence

Which memory paths follow a leaf link (read from source at `0a8af94d7`, then run where marked):

- follows, fixed: `MemoryStore::read` (run), `read_anchor_file` (run), through `memory(action=read)` (run),
  dashboard `get_memory`, onboarding `memory.read`, `migrate/memories.rs` re-embed (read from callers).
- does not follow, unchanged: `MemoryStore::list` and `anchors::check_all_memories` (walkdir without
  `follow_links`; pinned by `list_does_not_report_or_enter_symlinks`), `MemoryStore::write` and `delete`
  (`WriteTransaction` refuses), `write_anchor_file` (`atomic_write` replaces the entry and never writes through).
- documented design, unchanged: a symlinked store root (`docs/conventions/memory-write-history.md`).
- 0 symlinks tracked under `.codescout/` (`git ls-files -s .codescout`, mode 120000, 2026-10-18) and no
  production code creates one; no tracked doc or test describes a legitimate symlinked topic.

## Hypotheses tried

1. **Hypothesis:** a legitimate flow symlinks topic files to a file inside the project. **Test:**
   `git log` of `docs/conventions/memory-write-history.md`, `grep -i symlink` over `docs`, `src/memory`,
   `src/tools/memory`, `src/tools/onboarding.rs` and `tests/memory_write_history.rs`. **Verdict:** rejected for
   anything shipped; the only symlink handling there is refusal. Every leaf link is therefore refused,
   including one landing inside the store, to match `WriteTransaction`.

## Fix

Fixed on branch `fix/memory-leaf-links-2026-10-18` by `732178af8` (patch-id
`7113e260a6ec1d52b12efb5a160ed54c5cca292b`, `git show 732178af8 | git patch-id --stable`).

`MemoryStore::read` walks the topic path below the store root with `symlink_metadata`. A missing component is
`Ok(None)` as before. A final component that is not a regular file returns `RecoverableError`
"memory target must be a regular file: <path>"; an intermediate component that is not a directory returns
"memory parent must be a real directory: <path>". The store root itself may still be a link. `read_anchor_file`
refuses a non-regular sidecar the same way ("anchor sidecar must be a regular file"). Behaviour changes: a dangling
leaf link is a refusal instead of "no such topic"; an unreadable component (EACCES) is now an error instead of
`None`; a directory at the topic path is a `RecoverableError` instead of a raw io error.

## Tests added

`src/memory/mod.rs`: `read_refuses_a_leaf_symlink_to_a_file_outside_the_store`,
`read_refuses_a_leaf_symlink_to_a_file_inside_the_store`, `read_refuses_a_dangling_leaf_symlink`,
`read_refuses_a_directory_at_the_topic_path`, `read_refuses_a_topic_below_a_symlinked_directory`,
`read_through_a_symlinked_store_root_still_works`, `read_of_a_missing_nested_topic_is_none`,
`list_does_not_report_or_enter_symlinks`; `src/memory/anchors.rs`: `read_anchor_file_refuses_a_symlinked_sidecar`;
`src/tools/memory/tests.rs`: `read_refuses_a_topic_that_is_a_symlink_to_a_secret` (shared and private store through
the tool dispatch). Mutation: six guarded sites mutated one at a time, all killed (table in the session report).

## Workarounds

Do not read memory from a repository you did not author until on the fix; inspect `.codescout/memories` for links.

## Resume

N/A once fixed. Open residuals: see `unverified` above.

## References

`src/memory/mod.rs`, `src/memory/anchors.rs`, `src/memory/write_transaction.rs`,
`docs/conventions/memory-write-history.md`, the sibling `.gitignore` issue.
