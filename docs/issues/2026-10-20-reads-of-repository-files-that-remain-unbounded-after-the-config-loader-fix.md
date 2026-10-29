---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-29
opened: 2026-10-20
owner: marius
related:
- docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md
- docs/issues/2026-10-20-the-write-lock-holder-record-is-written-through-a-link.md
severity: low
unverified: 'Two questions were never reproduced and are not fixed: (1) writers that skip `check_write`, including the hard-link `.md` -> `project.toml` case, are untested; (2) the swap between a path-type check and the open at the system-prompt and `.git` pointer sites was not exercised. File each as its own bug if one is reproduced. All fix commits named in the file were re-verified as ancestors of `experiments` on 2026-10-29; the fixes were not re-run.'
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
| 9, 8 (doctor) | `librarian/tools/doctor.rs` `worktree_residue_sessions` (`cc_session_id`) and the `gitdir` scan (now `registered_worktree_roots`) | Yes: both run over `.worktrees/` entries git does not vouch for (FIFO, red) | `6297c6718` (`ba540aeae0b3d940754113edc8a00dda9a44cd6f`); the read-door allow-list entries for these and the `tools/worktree.rs` reads were removed in `217ed7b8f` (a first gate run failed `no_librarian_production_read_bypasses_the_read_door` on their liveness floor, and on the new test modules being `#[cfg(all(test, unix))]`, which that scan does not recognise as test code: `#[cfg(test)] #[cfg(unix)]` in the two librarian files) | `MAX_SESSION_ID_FILE_BYTES` = 4096 (as `server.rs`), `read_git_pointer` for the gitdir files |

**Second pass (after the Opus review of `fbcff95af`)** added the rows the first pass left and one it missed:

| Row | Site | Reachable | Fixed in | Cap (all `cap-class: NOT_A_CAP`) |
|---|---|---|---|---|
| 12 (new) | `library/registry.rs` `LibraryRegistry::load` (`.codescout/libraries.json`, every activation: `agent/mod.rs` x3, dashboard) | Yes: `exists()` then a plain read, so a committed link to a FIFO held every activation (red); a link to `/dev/zero` was not driven | `08685b2ed` (`6f5e5ccecbcf506a59fd5b24ae1c9e7ea6d2123d`) | `MAX_LIBRARY_REGISTRY_BYTES` = 16 MiB (own limit since `decae71fc` (patch-id `24585e1324b70b40954682b597445b223cbb9133`), see the third pass; first pass used the 1 MiB config limit). A refused or over-cap file is an EMPTY registry, as an absent one is (all callers used `unwrap_or_default`; a directory used to be `Err`, now empty, same result at the callers). Invalid JSON, empty and invalid UTF-8 are still errors; the error quotes nothing |
| 3 | `workspace.rs` `deps_from_cargo/npm/pyproject/requirements/gradle` (onboarding) | Yes: sibling manifests of a discovered project are whatever the repository holds; FIFO held them (red). `read_file_capped` was not used (`File::open` blocks on a FIFO) | `14864cbbc` (`f0669b33e40b6683ac452451e97f63e8d48e2f85`) | `MAX_CONFIG_FILE_BYTES`; refused = skip the manifest; the `settings.gradle.kts` -> `settings.gradle` fall-through is kept and tested |
| 11 (new) | `memory/anchors.rs` `read_anchor_file` | Reachable only as a size problem: the path check already refuses a FIFO or link (that test is a control); a FIFO swapped after the check is the race | `852fce089` (`9d327d8de91512d8d6b6e2ac940f1dc29e4805b1`) | `MAX_CONFIG_FILE_BYTES`; over-cap or swapped-in special file is the same `RecoverableError` as a link, so `check_all_memories` lists the topic under `unreadable` and checks the rest |
| 10 | `agent/write_guard.rs` `read_holder_record` (read side) | Not only "written by this process": the record sits at `.codescout/write.lock.holder` inside the repository, so a planted link (to a FIFO or a device) or a very large record is reachable. Its WRITE side followed links too; that is a separate bug, `docs/issues/2026-10-20-the-write-lock-holder-record-is-written-through-a-link.md`, fixed in `3a4f46cd8` (patch-id `c03f96bd380394f09898915f401d5b0376878b8c`) | `b5a3d7b68` (`ffecdd9dfdbaaacd8dbd70af4b2db2c1fdc27515`) | new `MAX_HOLDER_RECORD_BYTES` = 64 KiB. Every refusal is `None`, which the doc on that function already defines as "no usable owner information", never "the lock is free"; no design decision was needed. `why_no_holder_record` reads only metadata and is unchanged |
| 7 | `server.rs` `librarian_enabled_at_runtime`, `peer_enabled_at_runtime` | Only through a `project.toml` swapped after the bounded loader read it; defence in depth (FIFO, red) | `8dee39068` (`2ff4285bf4845ebe97d84967a68731614b6b6597`) | `MAX_CONFIG_FILE_BYTES`, one shared seam `project_toml_enabled`; a refusal leaves the caller's default, as a failed read did |

Follow-ups in the same pass: `hash_file` now streams through the new `util::fs::stream_regular` (same open, bounds and refusals as `read_regular_bytes`, no 64 MiB buffer; digests unchanged for ordinary files) and `read_git_pointer` is silent for a directory (the `.git` of every ordinary checkout, read per usage row), both in `96306d8ec` (`d304884be81a658cb735d8ac608df07a2ce1df7e`); the duplicated 4096-byte session-id cap is one `util::fs::MAX_SESSION_ID_FILE_BYTES` (`96306d8ec`, `8dee39068`, `b50a0e4d3` (`5c82158935d58a075ab79a69fa173320906a5c1c`)); new teeth for `patch_project_table` over-cap and oversized `HEAD` in `8db52293b` (`9904ef770f0029454055e5cf81f4ba1127dd8db5`).

Known, intended behaviour: an anchored file over 64 MiB makes `check_path_staleness` report it permanently `Changed`, while `merge_anchors` / `refresh_hashes` drop its anchor silently.

**Third pass (Opus round-2 review of `eefbe1082`)**

- **Write side of row 10**: new issue `docs/issues/2026-10-20-the-write-lock-holder-record-is-written-through-a-link.md` (HIGH in the review, pre-existing). `write_holder_record` and `Drop` followed a link and blocked on a FIFO; both now go through `open_holder_record` (`O_NOFOLLOW | O_NONBLOCK`, fstat, truncate after the check). Commit `3a4f46cd8` (patch-id `c03f96bd380394f09898915f401d5b0376878b8c`). Red: one mutant per call site (`write_holder_record` back to `fs::write`; `Drop` back to `fs::write(.., "")`), each failing the FIFO deadline test and the outside-file test.
- **`stream_regular` bounds** (`748de1323` (patch-id `8b44ecda1cf28a29c3033883fb1cf63916ef2952`)): `/proc/self/status` (length 0, about 1.5 KB) under a 16 byte limit must be `Err(TooLarge { max: 16 })` with at most 17 bytes seen. Red: `take(u64::MAX)` fails it (`seen=1578`); the final `if total > max_bytes` neutralised (`if false`) fails it (`Ok(17)`: a truncated prefix as success) and the `hash_file_capped` test (private seam, because at the real 64 MiB limit this small file is simply hashed). The `take(u64::MAX)` mutant is caught only by the `stream_regular` test, not by the hash one.
- **`librarian_enabled_at_runtime`** (`85d2315ef` (patch-id `cea2446f85be9879755c6b8306a4ea64e95b986d`)) takes the `LIBRARIAN_ENABLED` value as `env_override` (the production call reads the variable and passes it, so behaviour is unchanged); a test covers FIFO, link, ordinary file, the env layers, at-limit and over-limit. Red: the plain read in `project_toml_enabled` fails the new test (FIFO leg: deadline panic).
- **`read_git_pointer` log test** (`748de1323` (patch-id `8b44ecda1cf28a29c3033883fb1cf63916ef2952`)) now runs the FIFO read under `within_deadline` with an explicit `Dispatch`. Red, two mutants because the first failure masks the second: a plain `read_to_string` fails earlier on the directory assertion (`a directory logs nothing`, 148 bytes logged); a plain read that keeps the directory exemption fails on `read_git_pointer on a FIFO did not return within 15s`.
- **`libraries.json` limit** (`decae71fc` (patch-id `24585e1324b70b40954682b597445b223cbb9133`)): `MAX_LIBRARY_REGISTRY_BYTES` = 16 MiB, tested at the limit, one over, and between 1 MiB and 16 MiB. Red: pointing it back at `MAX_CONFIG_FILE_BYTES` fails the between case (`left: 0, right: 1`).
- **Hash digest sizes** (`748de1323` (patch-id `8b44ecda1cf28a29c3033883fb1cf63916ef2952`)): the digest-equals-sha256 test covers 1, 65535, 65536, 65537, 131072, 131073 and 300000 bytes. No mutant was run for this one (it widens a control; it is not red on any change I made).
- **Open decision, not implemented**: an over-cap, invalid or otherwise refused `libraries.json` loads as an EMPTY registry, and `auto_register_deps` then SAVES over it (`save` is an `atomic_write`, which also replaces a link at that path). For invalid JSON that was already so; the refusal cases are new. Reasoned from `agent/mod.rs` (`load(..).unwrap_or_default()` at activation) and `auto_register.rs`, not driven. A do-not-save flag on a refused load is a separate decision.

**Fourth pass (Opus round-3 review of `11ccc999b`)**

- **Flaky log test, honest restatement.** `read_git_pointer_is_silent_for_a_directory_and_logs_a_fifo` (added in `748de1323` (patch-id `8b44ecda1cf28a29c3033883fb1cf63916ef2952`)) was order-dependent: under `cargo test --lib util::fs` it failed on its last assertion (`a FIFO is logged: ""`) in the reviewer's 3 of 3 runs, and in my own re-run without the fix 3 of 5. My earlier gate runs passed it by ordering luck, and my earlier red claims for it were about the mutant, not about its stability. Mechanism (tracing-core callsite interest): with exactly one registered dispatcher the interest of the `debug!` is decided by whichever thread first reaches it; the sibling tests reach it from threads with no dispatcher and cache "never". Fix `7d57bcdd7` (patch-id `e4fdb99c91417918644d9ee05a6c327ff92b7247`): the test keeps a second live dispatcher for its whole duration, which makes the interest "sometimes". After the fix: 7 consecutive runs of `cargo test --lib util::fs` (68 tests each) and 3 of `cargo test --lib fs::` (279 each), all green. Re-proved red with the plain-read mutant that keeps the directory exemption: `read_git_pointer on a FIFO did not return within 15s` in 3 of 3 runs.
- **`open_lock_file`** fixed in `357d0c968` (patch-id `0dbe98632c15ed80e664206cedc93165a3965a20`); see `docs/issues/2026-10-20-the-write-lock-holder-record-is-written-through-a-link.md` for the probes, the policy (a link is refused, never unlinked), and what a refusal does at activation: all three callers fail the activation with `failed to open write.lock for <root>`; nothing runs without the lock.
- **Pinned**: the `fstat` check of `open_holder_record` had a surviving mutant; the FIFO-with-a-reader test now kills it.
- **Recorded, not fixed**: a hard link at the holder record is followed (do not refuse on `st_nlink > 1`: `cp -al`, rsync `--link-dest` and snapshot copies make a count of 2 normal); `src/agent/build_check.rs:395-405` `cargo_lock_is_free` opens `target/debug/.cargo-lock` read+write+create through links (`target/` is normally untracked); a link in a PARENT directory (`.codescout` itself) is followed by both lock files.\n\n**Fifth pass (Opus round-4 review of `c436d7758`)**\n\n- `open_lock_file`'s `ELOOP` mapping was applied inside a `map_err` closure whose only statement was `cfg(unix)`; on non-unix it became `|e| { e }`, which `clippy::map_identity` rejects on the windows-gnu lane (`scripts/with-slot.sh scripts/build-windows.sh clippy --all-targets -- -D warnings`: `src/agent/write_guard.rs:336:35`). Fixed in `9e070eeaa` (patch-id `3a2c798ca625d7151059b68eae1e7528ac41082d`) by making the mapping a `cfg(unix)` `let`. Control: with the old code that command reproduces the error at 336:35; with the fix it is gone. The lane still fails on errors that predate the branch, none in a file this branch changed: `src/lsp/manager.rs:3174-3177` (E0425 x4), `src/lsp/manager.rs:109` (field never read), `src/lsp/client.rs:38` and `:43` (dead code), `src/memory/write_transaction.rs:161:17` (unused `mut`). Because the lane stops there, the `cfg(test)` target of the lib is not fully linted on Windows, so the new tests' non-unix compile was not checked.\n- The blast radius of a refused `write.lock` is stated exactly in the write-lock issue and in the rustdoc: the MCP server fails to START when the launch directory holds such a link (`Agent::new`, `agent/mod.rs:943`, called from `server.rs:2955`), and the old open followed such a link, so it is a deliberate behaviour change.\n- `cargo_lock_is_free` is `src/agent/build_check.rs:395-405` (the earlier citations lacked `agent/`).\n- The comment on the second dispatcher in the log test is reworded in `c3f9a1d98` (patch-id `b9a034bac34d86d634a1b103b461849a4e47dac9`): a dispatcher dropped at once also passed in the reviewer's probe, so it is not claimed to be the only fix.

**Not fixed in this branch, and why**

- The system-prompt and `.git` pointer reads still check the type on the path and open afterwards: a link swapped in between the two is followed. The bounded read narrows what that can do (type and size are checked on the opened descriptor) but does not remove the swap.
- The hard-link `.md` -> `project.toml` question: untouched; nothing new learned.

## Fix

Applied for rows 1 to 12 as described in the tables above, with the shared
`util::fs::read_regular_text` / `read_regular_bytes` helper (plus `read_git_pointer`), a cap named
and annotated per site, and a refused, oversized or non-UTF-8 file treated as the absent file where
the caller already skipped on absence. Error and debug messages carry the reason and the path,
never the file's content. Rows 3, 7, 10, 11 and 12 were added in the second pass (see above).

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

Second-pass modules: `library::registry::bounded_load_tests` (4), `workspace::bounded_manifest_tests`
(5), `memory::anchors::bounded_anchor_tests` (+2: sidecar over-cap, FIFO sidecar control),
`agent::write_guard::holder_record_bounded_tests` (2), `server::project_toml_enabled_tests` (2), plus
`util::fs` (`stream_regular`, directory-silent log capture), `memory::hash` (multi-chunk digest),
`tools::onboarding` (`patch_project_table` over-cap), `prompts` (oversized `HEAD`) and
`util::path_security` (directory `.git`).

Red evidence: each module's production read was reverted to the plain read, the module run, and the
revert undone with `git checkout --` (`git diff HEAD` empty afterwards). FIFO tests failed with the
deadline panic; over-cap tests failed on the assertion. For the second pass all mutants were applied
at once (one line each: the read in `registry.rs`, `workspace.rs`, `anchors.rs`, `write_guard.rs`,
`server.rs`, `doctor.rs`; `read_regular_text(.., u64::MAX)` in `patch_project_table` and in
`read_git_pointer`; the directory test of `read_git_pointer` reverted; the `sink(..)` call of
`stream_regular` removed) and the failing test names read per module: 24 failures, covering the FIFO
and over-cap tests of rows 3, 7, 10, 11, 12, the doctor over-cap test, the `patch_project_table`
over-cap test, both oversized-pointer tests, the directory-log test and the multi-chunk digests.
Correction to the first pass: the doctor "oversized" case wrote a valid id followed by a NUL tail,
which `is_session_id` rejects on its own, so a plain read kept it green and it had no teeth; it is
now the id padded with spaces (trimmed away) plus an at-the-limit case, and fails on the plain read.
Tests that stay green on the plain read are the controls and the `/dev/null` device checks; the
length bound inside `stream_regular` (never more than `max + 1` bytes pulled from a file that
lies about its length) has no test of its own, the shared `read_bounded` has.

## Workarounds

Do not point repository files at special files; look at `.codescout/` and the manifests of an
untrusted project before activating it.

## Resume

2026-10-26: every fix commit named in the tables (rows 1 to 12, 19 SHAs) is verified an ancestor of `experiments`, and the helpers exist in `src/util/fs.rs`. The branch `fix/bounded-repo-reads-2026-10-20` no longer exists locally, so nothing is left to merge. Two questions remain open and neither is verified as a defect:

- Writers that skip `check_write`, including the hard-link `.md` -> `project.toml` case (untested).
- The swap between a path-type check and the open at the system-prompt and `.git` pointer sites.

File each as its own bug once reproduced, then close this file with `unverified: TRACKED <id>`.

2026-10-29: marked `fixed` on the operator's decision, with the two questions kept in the `unverified:` field. A re-check the same day found all 33 commit ids in this file (the fix commits, their base commits and review rounds) to be ancestors of `experiments`. The fix tests were not re-run.

## References

- `docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md`
