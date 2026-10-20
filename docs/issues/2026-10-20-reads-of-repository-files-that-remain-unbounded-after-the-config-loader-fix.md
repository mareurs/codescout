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
unverified: By reading only, at commit 44338ba87 (every line number in Root cause is at that commit; the fix branch fix/config-bounded-read-2026-10-20 may move some). No FIFO or link was driven through any site below. Reachability is a claim about the code path, not a probe. Whether a hard link to a protected project.toml can be catalogued and rewritten by write_catalogued_file is reasoned from the code, not tested; the memory anchor path (row 6) was not traced.
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
mechanism was measured for the config loaders in the related issue). A link in a repository can
also name `/dev/zero` (memory) or `/dev/stdin` (the server's own input), which no FIFO is needed
for; a repository can commit a link, not a FIFO.

**Every line number below is at commit `44338ba87`** (branch `fix/config-bounded-read-2026-10-20`,
the tip before this docs commit; `src/` is the same at the docs commit). Each was read at that
commit, not carried over from the first version of this file (which cited `a3bdc48fb`: the
system-prompt read has since moved from `:1974` to `:1972`, and the two `server.rs` reads from
`:1909` / `:1960` to `:2034` / `:2085`).

Ranked, most directly repository-controlled and most often run first:

1. **`src/library/auto_register.rs`, HIGH.** `std::fs::read_to_string` of the project root's own
   manifests, with no type or size check: `Cargo.toml` `:80`, `package.json` `:242`,
   `pyproject.toml` `:407`, `requirements.txt` `:412`, `go.mod` `:534`, `build.gradle.kts` /
   `build.gradle` `:640` / `:641`, `pom.xml` `:644`. Run on every activation, from
   `src/tools/config/mod.rs:281` and `:363` (`auto_register_deps`). Each name is repository
   content and can be a committed link to `/dev/zero` or `/dev/stdin`.
2. **`src/tools/onboarding.rs:112`** (`patch_project_table`): `std::fs::read_to_string(config_path)`
   of `project.toml`, a second read after the bounded load, so a path swapped in between, or any
   caller that did not load first, is unbounded. (It also writes back with `std::fs::write`; see
   the writers below.)
3. **`src/workspace.rs:601`, `:622`, `:650`, `:692`, `:713`** (`deps_from_cargo`, `deps_from_npm`,
   `deps_from_pyproject`, `deps_from_requirements`, `deps_from_gradle`): plain
   `read_to_string(project_root.join("<manifest>"))`. `project_root` is a discovered sub-project,
   so one manifest in the directory is a regular file; the others (a `requirements.txt` next to a
   `Cargo.toml`) need not be. `read_file_capped` (`:24`) is NOT in this list: it caps with
   `take(max_bytes)` and is reached only for entries the `ignore` walker reports as regular files
   by their own type (`:61`; no `follow_links`).
4. **`src/library/discovery.rs:54`, `:84`** and **`src/library/versions.rs:14`, `:22`**: each is
   guarded by `.exists()` (which follows links and is true for a FIFO) and then a plain
   `read_to_string`. `discovery` is reached from `src/fs/mod.rs:540` and `src/tools/library.rs:164`
   (library roots, so third-party source trees); `versions` from `src/tools/semantic/index.rs:162`
   (an index build; `Cargo.lock`, `package-lock.json` in the project root).
5. **`.codescout/system-prompt.md`**: `src/agent/mod.rs:1972` is type-checked first
   (`symlink_metadata`, regular file only; a link or FIFO is never served) and then read with no
   size cap, into the `initialize` instructions; `src/tools/onboarding.rs:62` hashes the same file
   through `hash_file_or_empty` (`src/memory/hash.rs:31`, `std::fs::read`) with no type check at
   all, so a link there is followed.
6. **`src/memory/hash.rs:13`** (`hash_file`, `std::fs::read`), called from
   `src/memory/anchors.rs:126`, `:162`, `:213`, `:390` on paths named by memory anchors under the
   project root. Reachability not traced: whether an anchor can name a path that is a planted
   link, and which tool calls run the anchor check (`status` runs `check_all_memories`), was not
   followed.
7. **`src/server.rs:2034`** (`librarian_enabled_at_runtime`) and **`:2085`**
   (`peer_enabled_at_runtime`): `read_to_string` of `<project>/.codescout/project.toml`. In the
   ordinary start-up order the bounded loader read the same path first; a path swapped between the
   two reads would hold the second.
8. **The git pointer reads**, from a `.git` file and the `gitdir:` path it names. The path inside
   the pointer is chosen by whoever wrote the pointer, so the `HEAD` it leads to can be anywhere:
   `src/prompts/mod.rs:964` (after a `symlink_metadata` regular-file check, no size cap) and
   `:990` (`<gitdir>/HEAD`); `src/librarian/tools/worktree.rs:44` (`.git`, no type check) and
   `:46` (`<gitdir>/HEAD`); `src/util/path_security.rs:1137` (after the same regular-file check),
   `:1159` (`worktree_main_root`, `.git` read with no type check) and `:1091` (the `gitdir` files
   under `.git/worktrees/`); `src/librarian/tools/doctor.rs:3397` (the same `gitdir` files). A
   `.git` is created by git, not committed, which is why these rank below the rows above.
9. **`src/librarian/tools/doctor.rs:3317`** (`worktree_residue_sessions`): `read_to_string` of a
   worktree's `.codescout/cc_session_id`, the file the server start-up read was fixed for.
   Gitignored per-session residue, read by the doctor over worktree directories.
10. **`src/agent/write_guard.rs:75`** (`read_holder_record`): `read_to_string` of the
    `write.lock.holder` record, which follows links. Written by this process; reachable by a
    planted link only.

**Writers that do not call `check_write`** (so a protected `project.toml` can be written without
the guard in `src/config/protected_files.rs`):

- `src/tools/onboarding.rs` `patch_project_table`: `std::fs::write(config_path, …)` at the end of
  the function (`:112` is its read). It re-serialises the whole raw table, so `[security]` is
  carried over as read; the module doc of `protected_files.rs` already names `onboarding` as out of
  the guard's scope.
- `src/librarian/util.rs:85-94` `write_catalogued_file`: `refuse_write_through_symlink`, then
  `refuse_catalogued_path_outside_repo`, then `std::fs::write`. It calls neither `check_write` nor
  the bounded read. Its callers take `row.abs_path` from the catalog (`augmentation_sidecar.rs`,
  `catalog/augmentation.rs`, `catalog/rekey.rs`, `tools/doctor.rs:2154`, `tools/mv.rs:793`,
  `tools/update.rs:785` and `:1020`). **Can it reach a protected path? Verified: the indexer walk
  catalogues only `.md` files** (`src/librarian/indexer.rs:432` and `:1256`:
  `path.extension() != Some("md")` is skipped), so no row is minted for a `.toml` name, and the
  write door refuses a symlink leaf. **Not verified, from the code only:** a regular `.md` name
  that is a hard link to a protected `project.toml` would be catalogued (`is_file()` is true,
  `refuse_write_through_symlink` looks at symlinks) and its frontmatter rewritten without
  `check_write`; git cannot commit a hard link, so it needs local filesystem access. No other
  route that creates a row with a non-`.md` path was searched for beyond the callers listed, and
  none was tested.

## Evidence

The first version of this file quoted a grep at `a3bdc48fb`; the line numbers in `## Root cause` were
re-read at `44338ba87` and differ for the three that moved. A re-run is

```
git grep -n -E "read_to_string|std::fs::read\(" 44338ba87 -- src/library/auto_register.rs \
  src/library/discovery.rs src/library/versions.rs src/workspace.rs src/tools/onboarding.rs \
  src/server.rs src/agent/mod.rs src/agent/write_guard.rs src/memory/hash.rs
```

(and the git-pointer files named in row 8). At `44338ba87`: `src/agent/mod.rs:1972`,
`src/server.rs:2034` and `:2085`, `src/tools/onboarding.rs:62` (call) and `:112` (read),
`src/memory/hash.rs:13` and `:31`, `src/agent/write_guard.rs:75`, `src/workspace.rs:601/622/650/692/713`,
`src/library/auto_register.rs:80/242/407/412/534/640/641/644`.

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

Start with rows 1 to 4 of `## Root cause` (`auto_register.rs`, `patch_project_table`, the
`deps_from_*` manifest reads, `discovery.rs` / `versions.rs`): they are the ones a repository can
plant directly and that run on activation. Prove each red with a FIFO under
`util::fs::special_file_fixtures::within_deadline`, then the rest in order.

## References

- `docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md`
