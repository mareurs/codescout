---
status: open
opened: 2026-10-20
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'By reading only, at commit a3bdc48fb (every line number below is at that commit; the fix branch fix/config-bounded-read-2026-10-20 moves some of them). No FIFO was driven through any site below except where a row says so. Reachability is a claim about the code path, not a probe.'
---

# BUG: reads of repository files that the config-loader fix did not touch still use a plain read, so a linked FIFO or device can hold them or an oversized file can fill memory

## Summary

`docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md` fixed the
config loaders by reading through `util::fs::read_regular_config_text`. The reads below have the
same shape (a path inside the repository or the user's config tree, read with `std::fs::read*`,
which blocks opening a FIFO and reads a device or an oversized file to the end) and were left
alone on purpose, so that branch stays one change. They are ranked by how directly repository
content reaches them.

## Class fit

A read whose bound is on the file's type and size as the file reports them, or on nothing, applied
to a path that repository content controls. It is the same family as the config-loader bug and is
filed unclassified for the same reason: no existing class names a limit checked before, or not at
all, on the operation it protects.

## Symptom (Effect)

Not measured for these sites. By reading: a FIFO, a link to one, or a link to `/dev/zero` at the
path holds the call (FIFO) or exhausts memory (`/dev/zero`); a regular file of any size is read
whole.

## Reproduction

Not run. Shape, per site: replace the named file with `mkfifo` (or a link to one) and make the
call that reaches it, under `timeout`. Never a link to `/dev/zero` on a machine you need.

## Environment

Linux; tree at `a3bdc48fb` (branch `fix/config-bounded-read-2026-10-20`, base `experiments`
79dcf2c58).

## Root cause

`std::fs::read` / `read_to_string` have no type check and no bound, and the open of a FIFO blocks
(inferred from the call sites and the std contract; not measured here, except that the same
mechanism was measured for the config loaders in the related issue).

Ranked, most directly repository-controlled first. All line numbers at `a3bdc48fb`:

1. `src/tools/onboarding.rs:62` calls `crate::memory::hash::hash_file_or_empty` on
   `.codescout/system-prompt.md`, which does `std::fs::read(path)` at `src/memory/hash.rs:31`
   (`unwrap_or_default`, so a refusal would read as an empty file, which is the intended
   conservative answer for change detection). Repository content; no type check, no cap. Found
   while tracing item 6; not in the review's list.
2. `src/agent/mod.rs:1974`: `.codescout/system-prompt.md` is type-checked first
   (`symlink_metadata`, regular file only, so a link or FIFO is never served) but then read with
   `std::fs::read_to_string` and no size cap: a large regular file is read whole and goes into the
   `initialize` instructions.
3. `src/workspace.rs:601`, `:622`, `:650`, `:692`, `:713` (`deps_from_cargo`, `deps_from_npm`,
   `deps_from_pyproject`, `deps_from_requirements`, `deps_from_gradle`): each does
   `std::fs::read_to_string(project_root.join("<manifest>"))`. `project_root` is a discovered
   sub-project, found by the manifest walk, so one manifest in the directory is a regular file;
   the others named here (a `requirements.txt` next to a `Cargo.toml`) need not be. A FIFO or link
   there holds project discovery. Note `read_file_capped` (`:24`) is NOT in this list: it caps with
   `take(max_bytes)` and is reached only for entries the `ignore` walker reports as regular files
   by their own type (`entry.file_type().is_file()`, `:61`; the walker does not follow links), so
   a linked FIFO is not opened there.
4. `src/memory/hash.rs:13` (`hash_file`, `std::fs::read`), called from `src/memory/anchors.rs:126`,
   `:162`, `:213`, `:390` on paths named by memory anchors resolved under the project root.
   Reachability not traced: whether an anchor can name a path that is a repository-planted FIFO
   or device, and which tool calls run the anchor check (the `status` call runs
   `check_all_memories`), was not followed.
5. `src/server.rs:1909` (`librarian_enabled_at_runtime`) and `:1960` (`peer_enabled_at_runtime`):
   `std::fs::read_to_string` of `<project>/.codescout/project.toml`. Reached only after
   `ProjectConfig::load_with_global_base` has read the same path through the bounded helper, in
   the ordinary start-up order; a `project.toml` swapped to a FIFO between the two reads would
   hold the second. Low.
6. `src/agent/write_guard.rs:75` (`read_holder_record`): `std::fs::read_to_string` of the
   `write.lock.holder` record, which follows links. The file is under `.codescout/`, and is
   written by this process; reachable by a planted link only. Low.

## Evidence

The grep at `a3bdc48fb`:

```
src/agent/mod.rs:1974:  Ok(m) if m.file_type().is_file() => std::fs::read_to_string(&prompt_file).ok(),
src/agent/write_guard.rs:75:  let buf = std::fs::read_to_string(path).ok()?;
src/memory/hash.rs:13:  let bytes = std::fs::read(path)?;
src/memory/hash.rs:31:  let bytes = std::fs::read(path).unwrap_or_default();
src/server.rs:1909:  if let Ok(text) = std::fs::read_to_string(&cfg) {
src/server.rs:1960:  if let Ok(text) = std::fs::read_to_string(&cfg) {
src/workspace.rs:601/622/650/692/713: std::fs::read_to_string(project_root.join(...))
```

## Hypotheses tried

None.

## Fix

Not applied. Direction: the same helper, `util::fs::read_regular_text` with a cap chosen per file
(system-prompt.md may want a larger or smaller cap than 1 MiB; the manifests 1 MiB), a refusal
treated as the file being absent where the caller already collapses errors, and one FIFO test per
site under `util::fs::special_file_fixtures::within_deadline`.

## Tests added

N/A. Not fixed.

## Workarounds

Do not point repository files at special files; look at `.codescout/` and the manifests of an
untrusted project before activating it.

## Resume

Start with items 1 and 3 (the only ones a repository can plant directly), prove each red with a
FIFO under `within_deadline`, then 2, 4, 5, 6.

## References

- `docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md`
