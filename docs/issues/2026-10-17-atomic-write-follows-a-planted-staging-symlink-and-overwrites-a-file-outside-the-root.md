---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md]
tags:
- cluster/unclassified
unverified: 'Windows behaviour of CREATE_NEW against a planted reparse point was not run; no end-to-end plant was run against edit_file, edit_markdown or memory writes (read from callers). The fix SHAs recorded here are those of the review branch and may not survive a rebase: use the patch-ids.'
kind: bug
---

# BUG: `atomic_write` writes its staging file through a planted `<name>.tmp` symlink, so a write to `x.txt` overwrites a file outside the project root

## Summary

`atomic_write` (`src/util/fs.rs`) stages content at `<name>.tmp` next to the target with
`std::fs::write`, which follows a symlink, then renames the staging path onto the target. If
`<root>/x.txt.tmp` is a symlink to `/outside/victim`, a write to `<root>/x.txt` overwrites the victim and
leaves a symlink where `x.txt` should be. The write allowlist classifies `<root>/x.txt` as inside the root
correctly; the escape is below it.

## Class fit

No existing class names a write that follows a link in a path the caller never named: the staging path
is derived inside the writer. It is not the allowlist class (`write-allowlist-canonicalizes-only-the-parent...`),
since the classifier answers correctly for the path it is given. Filed unclassified for lack of a second
instance of "an internally derived sibling path is followed through a link".

## Symptom (Effect)

A file outside the project root is overwritten with the content of an in-root write, the in-root target
becomes a symlink to it, and the tool reports success. `git` can carry a symlink, so a repository can ship
`x.txt.tmp -> /path/to/target` and wait for any codescout write to `x.txt`.

## Reproduction

Measured 2026-10-17 on branch `worktree-agent-a5bf8d6383eb7a8b1` by calling the real
`crate::util::fs::atomic_write` from a throwaway unit test (since removed), all under a `tempdir()` in the
scratch tree: `root/` and `outside/victim.txt` containing `ORIGINAL`, `symlink(outside/victim.txt,
root/x.txt.tmp)`, then `atomic_write(root/x.txt, "WRITTEN-VIA-ATOMIC-WRITE")`. Output:
`victim="WRITTEN-VIA-ATOMIC-WRITE" x.txt_is_symlink=true`. The reviewer reproduced the same with a verbatim
replica (`scratchpad/review-a5bf/staging.rs`) before this.

## Environment

Linux, `299395e5` plus the branch's path-security changes; `src/util/fs.rs` is unchanged by them.

## Root cause

`atomic_write` -> `std::fs::write(&tmp, content)` with `tmp = staging_path(path)`. `fs::write` is
`File::create`, which follows a symlink at `tmp`. The following `fs::rename(&tmp, path)` then moves the
link itself onto `path`. Nothing checks that `tmp` is not a link, and `O_EXCL`/`O_NOFOLLOW` are not used.

## Evidence

`atomic_write` callers (from `references`): `src/tools/edit_file/mod.rs` (3 sites), `src/tools/symbol/edit_code.rs`,
`src/tools/markdown/edit_markdown.rs`, `src/symbol/edit.rs`, `src/library/registry.rs`, `src/memory/anchors.rs`,
`src/memory/mod.rs`, `src/operator_rules/mod.rs`, and `write_utf8` in `src/util/fs.rs`. `create_file` and the
edit tools reach it through those; this entry ran the function directly, not each tool.

## Hypotheses tried

None needed for the function-level result. Whether each tool reaches the sink with the planted name intact
was read from the callers, not run.

## Fix

FIXED on the review branch `worktree-agent-a26646b8b970ff11a` (`b493599d` fix and tests, `7b1d719a` a test,
`417c4d3e` private staging, mode and name-closure tests, doc corrections, `133ac224` umask-proof and skip-loud tests,
real-closure staging tests, fchmod and mode-read-race documentation). patch-ids (`git show <sha> | git patch-id --stable`):
`b493599d` -> `4c9425528fab3aae53dac1ce079719843750e93c`, `7b1d719a` -> `e9aaed1e8092c028f27e12bc8edde9a86bceb9f3`,
`417c4d3e` -> `06f4bdffd22d16002c71813738fc714978f04462`, `133ac224` -> `ff43af64c8b312ef9c1893a287490be649a11925`.

`src/util/fs.rs`:

- `create_staging_file` opens the staging file with `OpenOptions::new().write(true).create_new(true)`,
  i.e. `O_CREAT|O_EXCL` (`CREATE_NEW` on Windows). That fails with `AlreadyExists` if ANYTHING is at the
  name (regular file, directory, symlink, dangling symlink) and never follows a link at the final
  component, so `O_NOFOLLOW` is not added: it would be redundant. On `AlreadyExists` it tries the next
  name, at most `STAGING_ATTEMPTS` = 16, then returns an `AlreadyExists` error naming the staging file.
  It never removes or writes a colliding entry. Any other error (missing parent, EACCES) returns at once.
- `staging_path(path, nonce)` is now `<file name>.<nonce>.tmp` beside the target, with the nonce a
  `uuid::Uuid::new_v4().simple()` (128 random bits). The file name is cut to 100 bytes at a char boundary
  so a name near `NAME_MAX` still fits. It returns `None` for a path with no file name, and
  `atomic_write` then returns `InvalidInput` immediately (it used to fail later in `fs::write`/`rename`).
- The mode copy is done on the open handle (`File::set_permissions`, `fchmod`), not by path.
- `atomic_write` is a thin wrapper over `atomic_write_with(path, content, next_name, write)`, which takes
  the name sequence and the write step as parameters. A test needs them: with random names a planted
  `x.txt.tmp` is never opened, so the tests that plant it pass against ANY implementation that merely
  renames its staging file (measured: mutating `create_new(true)` to `create(true).truncate(true)` leaves
  all three planted-`x.txt.tmp` tests green; only the two seam tests that offer the planted entry as the
  first candidate die).
- Cleanup: a write failure or rename failure removes the staging file, which is always one this call
  created.
- The guarantee is AT THE FINAL COMPONENT of the staging path only. A symlinked parent directory redirects the
  create and the rename alike (measured by the reviewer); for the write tools the classifier resolves the parent,
  while a caller that bypasses it (`memory/mod.rs` `ensure_gitignored`) gets no parent protection from
  `atomic_write`.
- Leftovers: a kill between the create and the rename leaves `<name prefix>.<32 hex>.tmp` that nothing reclaims,
  and a stale legacy `<name>.tmp` is never touched again. `.tmp` is not gitignored here (`git check-ignore` exit
  1), so leftovers show in `git status`; not embedded (`retrieval/sync.rs`), not catalogued (`indexer.rs` takes
  `.md` only). No reaper built.
- Staging is `0600` while the content is written when the target exists, and takes the target's mode afterwards;
  a new file keeps `0666 & !umask`. **The final mode is unchanged in every case where `fchmod` succeeds** (existing
  incl. setuid, new, symlink target, dangling), pinned by `the_final_mode_is_unchanged_for_every_kind_of_target`.
  A failing `fchmod` is ignored, as before, and now leaves an existing target at `0600 & !umask` instead of
  `0666 & !umask`: it fails closed (a `0644`/`0755` file narrows to `0600`, the write still succeeds). Plausible
  hosts: sshfs with an sftp-server started `-P setstat,fsetstat`, an LSM (SELinux) denying `setattr`, sticky/setgid
  rules on macOS/FreeBSD; not run on any of them. Documented in `atomic_write`'s doc, behaviour not changed.
  Reading the mode BEFORE the create widens the documented same-target race; measured by the reviewer against the
  old order: a target created `0600` during our write ends `0644` (old: `0600`); a `chmod 0644 -> 0755` during the
  write is lost (old: `0755`); a target deleted during the write keeps `0600` (old: `0644`). Also documented there.
  Before, a `0600` target's new content sat in a `0644` staging file for the length of the write. Red first:
  `the_staging_file_of_a_private_target_is_private_while_it_is_written` saw mode `0o044` in group/other.
  (Commit `417c4d3e`.) **Superseded 2026-10-27:** `atomic_write` no longer carries setuid or setgid over (`9289dc7cf`), so "unchanged in every case ... incl. setuid" above and a setuid result for a link to a setuid file both describe the state before that fix; see
  `archive/2026-10-17-atomic-write-replace-keeps-setuid-and-takes-the-pointed-at-files-mode.md`.
- Not changed: no fsync (the code never had one), the rename-over-target semantics.

**A target that is itself a symlink is replaced, not written through** (decision, pinned by test): the
rename swaps the directory entry, so the pointed-at file keeps its bytes. The mode is still read with
`metadata` (follows the link), so the new regular file takes the pointed-at file's permission bits; for a
dangling link the default applies.

Same STAGING pattern, fixed: `src/tools/run_command/attribution.rs::materialize_into` staged each embedded script at
`<name>.<pid>.tmp` in the shared temp dir with `std::fs::write` (predictable name, link followed). It now
calls `atomic_write`. Measured red first: with `<name>.<pid>.tmp` planted as a symlink the script body was
written into the symlink's target. **Only the staging file is fixed there.** The directory itself
(`$TMPDIR/codescout-attribution-<hash>`, a computable name that is reused without an owner check, and from which
`python3` runs with its directory importable) is a separate and measured problem, filed as
`docs/issues/2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md` and NOT fixed.

The existing ENOSPC test planted `<target>.tmp -> /dev/full` to make `atomic_write` itself open it, which is
the vulnerability, so it can no longer be the way in. `atomic_write_removes_its_tmp_file_when_the_write_itself_fails`
now does a real exclusive create and injects the write step, which writes the bytes to `/dev/full` (the
kernel's real ENOSPC), and additionally asserts the staging file existed when the write ran.

Checked, same pattern absent or not reachable from a write tool:

- `src/memory/write_transaction.rs` stages with `tempfile::NamedTempFile::new_in` (random name, `O_EXCL`);
  `tests/memory_write_history.rs::an_existing_legacy_tmp_symlink_is_never_followed` covers it.
- `atomic_write` callers: `tools/edit_file/mod.rs` (3 sites), `tools/symbol/edit_code.rs`,
  `tools/markdown/edit_markdown.rs`, `symbol/edit.rs`, `library/registry.rs`, `memory/anchors.rs`,
  `memory/mod.rs`, `operator_rules/mod.rs`, `write_utf8` (-> `create_file`, `guide_ledger::persist`,
  `rendezvous::publish` at `tools/rendezvous.rs:136`).
  All gain the fix; none stages on its own.
- `src/librarian/tools/mv.rs`, `src/logging.rs` and `src/platform/*::rename_overwrite` rename user or log
  files, they do not stage content.
- `scripts/measure/run.py::_write_atomic` unlinks its `<name>.tmp` first and then creates it with `O_EXCL`
  (`_write_new`), so it does not follow a planted link; `_write_exclusive` uses a dot-named pid temp the same
  way. `scripts/measure/label.py:359-365` (`relabel_pick.json.tmp`, `open(tmp, "w")` then `os.replace`) DOES follow
  one. They are measurement scripts, not reachable from an MCP write tool, and were NOT changed.

## Tests added

`src/util/fs.rs` (tests module). Red = failing at `65fc6d6a` (before the fix, run 2026-10-17).

**What the tests prove and do not.** The three planted-`x.txt.tmp` tests (symlink, dangling symlink, regular
file) are regression checks against the OLD fixed name only: with random names a planted `x.txt.tmp` is never
opened, so they pass against any implementation that merely renames its staging file (measured: mutating
`create_new(true)` to `create(true).truncate(true)` leaves all three green). The exclusive-create guarantee is
carried by the seam tests, which offer the planted entries as the FIRST candidate names
(`every_kind_of_planted_entry_at_a_candidate_name_is_refused_and_left_alone`,
`staging_gives_up_after_a_bounded_number_of_collisions_and_changes_nothing`). The seam is `atomic_write_with`; the
real write closure is not driven with an injected fault. The real NAME closure is covered without a seam, in three
parts, none of which alone is enough: `the_real_staging_file_is_created_on_the_targets_filesystem` catches staging
on a different device (target on `/dev/shm` or `/var/tmp`, staging in `temp_dir()` fails the rename with EXDEV);
`the_real_staging_file_is_beside_its_target_not_in_its_parent` catches staging in an ancestor on the same
filesystem (the target's parent is read-only, so a create one level up fails; it skips loudly as root);
`concurrent_writers_to_one_target_never_collide_on_a_staging_name` catches a reused nonce (8 writers, 512 KiB
each: a constant name makes concurrent creates collide and exhaust their 16 attempts). The free function
`staging_path` is tested directly by `real_staging_names_are_unpredictable_and_not_the_legacy_name`; that test
does not exercise the closure inside `atomic_write`. The cross-device test SKIPS, printing each candidate's
finding and re-checking it, on a host where neither `/dev/shm` nor `/var/tmp` is on a different device than
`temp_dir()` (a container without its own `/dev/shm`); where one exists it asserts. A seam-free fault for the
real write closure needs an `RLIMIT_FSIZE` child or a thread-only seccomp filter; both are heavy and fragile on
CI, so they were not adopted and the seam test pins it.

- `a_planted_staging_symlink_is_not_followed_and_the_victim_is_untouched` RED (`victim` held `WRITTEN`).
- `a_planted_dangling_staging_symlink_does_not_create_the_file_it_points_at` RED (the outside file was created).
- `a_planted_regular_staging_file_is_not_clobbered` RED (the planted file was renamed away, `NotFound`).
- `a_new_file_and_an_overwrite_both_work_and_leave_no_staging_file`, `a_target_that_is_a_symlink_is_replaced_not_written_through`
  (also a dangling leaf): green before and after, they pin the normal path and the leaf decision.
- Added with the seam (cannot run before it exists): `every_kind_of_planted_entry_at_a_candidate_name_is_refused_and_left_alone`,
  `staging_gives_up_after_a_bounded_number_of_collisions_and_changes_nothing`,
  `real_staging_names_are_unpredictable_and_not_the_legacy_name`, `a_file_name_near_the_name_max_still_writes`,
  `a_missing_parent_directory_is_an_error_not_a_retry_loop`, `a_path_with_no_file_name_is_refused_up_front`,
  `a_failed_rename_leaves_no_staging_file_and_the_target_untouched`,
  `concurrent_writers_to_same_stem_files_never_cross_or_leak` (unix).
- Edited, not weakened: `staging_paths_are_distinct_for_files_that_share_a_stem` and
  `the_staging_path_stays_beside_its_target` now pass one constant nonce (with a random one every name is
  distinct and the first could not fail); `atomic_write_removes_its_tmp_file_when_the_write_itself_fails`, see Fix.
- Mode work (`417c4d3e`): `the_staging_file_of_a_private_target_is_private_while_it_is_written` (RED before,
  group/other bits `0o044`), `the_final_mode_is_unchanged_for_every_kind_of_target` (existing `0600/0640/0755/04755`,
  new file, symlink target takes the pointed-at mode, dangling; green before and after),
  `the_real_staging_file_is_created_on_the_targets_filesystem` (green before and after). The window test needs a
  umask that leaves group/other read open; under any other umask (077, 066, 067, 076) it re-runs itself in a
  child under `umask 022` and fails only if that child fails or cannot establish 022, so a host's umask never
  reds it. Run under `umask 077` once: all `util::fs::tests` passed, and the child path was exercised (a mutant
  disabling private staging was killed by the parent and by the child).

`src/tools/run_command/attribution.rs`: `materialize_does_not_follow_a_planted_staging_symlink` (RED before the fix).

Tool level, leaf symlink (see the allowlist issue): `create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`
(`src/tools/edit_file/tests.rs`), `edit_code_replace_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`
(`src/tools/symbol/tests.rs`).

Mutation probe (`scripts/mutation-probe.sh`, isolated, tests `util::fs::tests` and `leaf_symlink`), all KILLED:
`create_new(true)` -> `create(true).truncate(true)` (2 tests: the seam ones); `AlreadyExists => continue` ->
`return Err` (2); `0..STAGING_ATTEMPTS` -> `0..1` (2); nonce constant (2); `STAGING_STEM_MAX_BYTES` 100 -> 250 (1);
char-boundary loop disabled (1); `set_permissions` removed (1, the exec-bit test); write-failure cleanup
removed (1, the ENOSPC test); rename-failure cleanup removed (1); `rename` -> `copy` (7, including both
tool-level leaf tests).
Second round, all KILLED: private staging disabled (window test); `metadata` -> `symlink_metadata` for the mode
(the final-mode test, via the symlink row); mode application removed (2: exec bit and final-mode); real name
closure staged in `temp_dir()` (cross-filesystem test); staging mode `0600` -> `0666` (window test).
Third round (`133ac224`), all KILLED: real closure staged in the grandparent on the same filesystem
(`the_real_staging_file_is_beside_its_target_not_in_its_parent`); real closure with a constant nonce
(`concurrent_writers_to_one_target_never_collide_on_a_staging_name`); private staging disabled, run under
`umask 077` (the window test, in the parent and in its umask-022 child).

## Workarounds

None needed after the fix. Before it: do not open a repository that ships `*.tmp` symlinks with write tools enabled.

## Resume

Fixed on the branch; awaiting review and merge. Windows (`CREATE_NEW` refusing a planted reparse point)
was not run: only `cargo test` on Linux. Related findings from the review, filed and NOT fixed:
`2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md`. The zombie
`2026-08-14-atomic-write-rename-not-detected-by-file-watchers.md` cites `atomic_write`'s old shape and line range;
it carries a one-line note, status unchanged. After it reaches `experiments`, archive this file
(`doc(action="move")`), keeping the SHAs and patch-ids above. Not run: an end-to-end plant against
`edit_file`, `edit_markdown`, a memory write. `edit_file`, `edit_markdown`, anchors and `.gitignore` writes
reach `atomic_write` (read from the callers); memory TOPIC writes do not (they use `NamedTempFile::persist` in
`src/memory/write_transaction.rs`, which is exclusive and random, covered by
`tests/memory_write_history.rs`). `create_file` and `edit_code` were run only for the leaf-symlink case.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`
