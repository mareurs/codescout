---
status: investigating
opened: 2026-10-20
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md]
tags:
- cluster/unclassified
kind: bug
unverified: Rows 3 (src/workspace.rs deps_from_*), 7 (src/server.rs *_enabled_at_runtime) and 10 (src/agent/write_guard.rs read_holder_record) were NOT touched in branch fix/bounded-repo-reads-2026-10-20 (they belong to other work) and are still plain reads; the new row 11 (src/memory/anchors.rs read_anchor_file, regular-file check but no size cap) is unfixed. The hard-link .md -> project.toml question is untested. For the fixed rows, reachability by a FIFO is proven by tests run red against the plain read (see Re-verification); reachability of a link to /dev/zero is reasoned from std path semantics and the type check, never driven (not run on this machine). Fixes are on the branch only; not yet merged to experiments.
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

## Re-verification at HEAD and fixes (2026-10-20, branch `fix/bounded-repo-reads-2026-10-20`)

Every row of this file named below was re-read at `25fa6c362` (line numbers had drifted; the
functions are named instead). Reachability was decided by a test that puts a FIFO (and a link to
one) where the file belongs, calls the real call site under `within_deadline`, and was run RED
against the plain read (a byte-exact revert of the production read, then `git checkout --` and an
empty `git diff HEAD`): every FIFO test below reported `did not return within 15s: it is blocked
reading a special file`. A link to `/dev/zero` was NOT driven (it fills memory); `/dev/null` stands
for "a device" in the controls and reads empty even on the plain read, so those tests document the
refusal and are not red tests.

| Row | Site | Reachable | Fixed in | Cap (all `cap-class: NOT_A_CAP`) |
|---|---|---|---|---|
| 1 | `library/auto_register.rs` `collect_*_deps` (Cargo.toml, package.json, pyproject.toml, requirements.txt, go.mod, build.gradle(.kts), pom.xml) | Yes: FIFO or link to one at any manifest name held the call (red) | `077a02953` (patch-id `62635a8c97f2413e61a17a3d71dda76910052030`) | `MAX_CONFIG_FILE_BYTES` (1 MiB). Largest manifest of those names under `~/work` is under 200 KB (`find -size +200k` found only lock files) |
| 4a | `library/versions.rs` `resolve_dependency_versions` (Cargo.lock, package-lock.json) | Yes (FIFO, red) | `efe568f93` (`29b3101d8dc37217bf73841c8dd5a5be003040f6`) | new `MAX_LOCKFILE_BYTES` = 64 MiB: largest `package-lock.json` on this machine 757,914 B, largest `Cargo.lock` 407,737 B (this repo 143,847 B), so 1 MiB would be close to refusing real projects |
| 4b | `library/discovery.rs` `try_manifest` / `try_go_mod` (walk up from a library file) | Yes (FIFO on the walk, red) | `7002712be` (`0987964f5a6d3f618f4e82e960a31eff26059c80`) | `MAX_CONFIG_FILE_BYTES` |
| 2 | `tools/onboarding.rs` `patch_project_table` | Only through a path swapped after the load: both callers (`record_refresh_request`, `stamp_witnessed_refresh`) call `ProjectConfig::load_or_default` first, and that refuses a FIFO. Fixed as defence in depth; test drives the function directly (FIFO, red) | `36fa0e01e` (`5a7cd87dd926e61cc1e4a8050860476f3807de82`) | `MAX_CONFIG_FILE_BYTES`; an `Io` error keeps its original `io::Error`; a refusal names the path and quotes nothing |
| 5a | `agent/mod.rs` system-prompt read (now `read_system_prompt_file`) | A FIFO or link is NOT reachable (the `symlink_metadata` check already refuses both; only a swap between the check and the open could). The reachable defect was the missing size cap: red run served the file's 256 KiB of NULs into the `initialize` instructions | `d932a4a47` (`8bcd7883734b812f61e3bbd0dfb77a6b9a3a9aa4`) | new `MAX_SYSTEM_PROMPT_FILE_BYTES` = 256 KiB (largest on this machine 7,834 B); over-cap or refused falls back to the `project.toml` `system_prompt` like an absent file; an unreadable regular file is still `None` as before |
| 5b, 6 | `memory/hash.rs` `hash_file` / `hash_file_or_empty`, hence `hash_system_prompt` and the anchor checks | Yes. `check_path_staleness` (run by `check_all_memories`, called from `tools/config/mod.rs:691`) hashes a sidecar path after only `exists()`, so a FIFO there held `status` (red); `hash_system_prompt` had no type check at all (red). A sidecar is repository content, and `project_root.join("/dev/zero")` is `/dev/zero` (std semantics, not driven) | `2d62f0302` (`750b6a92e5fd40b019f082b8a70fd3da14036639`) | new `MAX_HASHED_FILE_BYTES` = 64 MiB. `check_path_staleness` reports a refused path as `Changed` instead of failing the whole check; `seed_anchors` skips an over-cap file instead of failing the memory write; `merge_anchors` / `refresh_hashes` already dropped what could not be hashed; `hash_file_or_empty` hashes a refused path as empty |
| 8 | `.git` pointer and `<gitdir>/HEAD`: `prompts/mod.rs` `detect_worktree_info`, `librarian/tools/worktree.rs` `read_branch`, `util/path_security.rs` `is_linked_worktree`, `worktree_main_root`, `list_git_worktrees` | Yes. `worktree_main_root` has no type check and runs from `usage/mod.rs:167`, `agent/mod.rs` `load_discover_settings` and the activation response, so a `.git` FIFO held them (red); a `HEAD` FIFO under a pointer-named gitdir held `detect_worktree_info` / `read_branch` (red); a `gitdir` FIFO under `.git/worktrees/` held `list_git_worktrees` (red); an oversized `.git` was read whole (red) | `d535f9a5d` (`198f6fec9d0846d2254c7d0ede01d5a273d93893`) | new `util::fs::read_git_pointer` with `MAX_GIT_POINTER_BYTES` = 64 KiB; `None` for absent, refused, oversized or non-UTF-8, as the `.ok()` sites already treated a failed read |
| 9, 8 (doctor) | `librarian/tools/doctor.rs` `worktree_residue_sessions` (`cc_session_id`) and the `gitdir` scan (now `registered_worktree_roots`) | Yes: both run over `.worktrees/` entries git does not vouch for (FIFO, red) | `6297c6718` (`ba540aeae0b3d940754113edc8a00dda9a44cd6f`) | `MAX_SESSION_ID_FILE_BYTES` = 4096 (as `server.rs`), `read_git_pointer` for the gitdir files |

**Not fixed in this branch, and why**

- **Row 3** `src/workspace.rs` `deps_from_*`, **row 7** `src/server.rs` `librarian_enabled_at_runtime` / `peer_enabled_at_runtime`, **row 10** `src/agent/write_guard.rs` `read_holder_record`: outside the site list this branch was given; untouched and still plain reads. Row 7 is reachable only through a path swapped after the bounded loader read; row 10 only by a planted link.
- **Row 11 (new), `src/memory/anchors.rs` `read_anchor_file`**: type-checked with `symlink_metadata` (a link or FIFO is refused) and then `std::fs::read_to_string` with no size cap, the same shape as row 5a. Found while reading `check_all_memories`; not fixed here and not tested.
- The system-prompt and `.git` pointer reads still check the type on the path and open afterwards: a link swapped in between the two is followed. The bounded read narrows what that can do (type and size are checked on the opened descriptor) but does not remove the swap.
- The hard-link `.md` -> `project.toml` question: untouched; nothing new learned.

## Fix

Applied for rows 1, 2, 4, 5, 6, 8 and 9 as described in the table above, with the shared
`util::fs::read_regular_text` / `read_regular_bytes` helper (plus `read_git_pointer`), a cap named
and annotated per site, and a refused, oversized or non-UTF-8 file treated as the absent file where
the caller already skipped on absence. Error and debug messages carry the reason and the path,
never the file's content. Rows 3, 7 and 10 and the new row 11 are not fixed (see above).

## Tests added

One module per site, each with a FIFO test (and a link to a FIFO) run under `within_deadline`, and
a control that proves ordinary files unchanged (regular, link to a regular file, missing, empty,
invalid UTF-8, directory, exactly-at-cap where cheap, over-cap via a sparse file whose first bytes
are valid so only the length check can refuse it):

- `library::auto_register::bounded_manifest_tests` (6 tests)
- `library::versions::bounded_lockfile_tests` (4)
- `library::discovery::bounded_manifest_tests` (3)
- `memory::hash::bounded_hash_tests` (5), `memory::anchors::bounded_anchor_tests` (5)
- `tools::onboarding::bounded_read_tests` (3: `patch_project_table`, `hash_system_prompt`)
- `agent::system_prompt_read_tests` (4, one through `project_status`)
- `prompts::git_pointer_read_tests` (4), `util::path_security::git_pointer_read_tests` (4),
  `librarian::tools::worktree::read_branch_bounded_tests` (3), `util::fs` `read_git_pointer` test (1)
- `librarian::tools::doctor::worktree_residue_bounded_tests` (4)

Red evidence: each module's production read was reverted to the plain read, the module run, and the
revert undone with `git checkout --` (`git diff HEAD` empty afterwards). FIFO tests failed with the
deadline panic; over-cap tests failed on the assertion. Tests that stay green on the plain read are
the controls and the `/dev/null` device checks.

## Workarounds

Do not point repository files at special files; look at `.codescout/` and the manifests of an
untrusted project before activating it.

## Resume

Rows 3 (`workspace.rs`), 7 (`server.rs`), 10 (`write_guard.rs`) and the new row 11
(`memory/anchors.rs` `read_anchor_file`) remain. Use the same shape: a seam that takes the path,
`read_regular_text` with a named `cap-class: NOT_A_CAP` constant, a FIFO test under
`util::fs::special_file_fixtures::within_deadline` proved red, then the writers that skip
`check_write` question. Merge the branch before archiving this file (record patch-ids of the SHAs
above, which are positional).

## References

- `docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md`
