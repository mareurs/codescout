---
status: fixed
opened: 2026-10-20
closed: 2026-10-20
severity: medium
owner: marius
related: [docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md]
tags:
- cluster/unclassified
kind: bug
unverified: "The fix is on branch fix/bounded-repo-reads-2026-10-20 only; not merged to experiments. A hard link at the record path is still followed and untested as a fix (deliberately: see Resume). A link in a PARENT directory (.codescout itself) is still followed by both lock files. agent/build_check.rs cargo_lock_is_free has the same shape and is NOT fixed. The reachability of the FIFO hang through a real write-tool call (as opposed to the acquire function) was not driven by me. The refusal text a user sees was provoked by the round-4 reviewer through Agent::new and Agent::activate: failed to open write.lock for <root>: <root>/.codescout/write.lock is a link; codescout does not follow or replace it. Remove it so the lock file can be created. FreeBSD (EMLINK instead of ELOOP) untested."
---

# BUG: the write-lock holder record is written and cleared with a plain `fs::write`, which follows a link and opens a FIFO blocking

## Summary

`src/agent/write_guard.rs` writes `.codescout/write.lock.holder` (`write_holder_record`, on the
UNCONTENDED acquire path) and empties it on release (`Drop`) with `std::fs::write`. The path is
inside the repository. A link there to a FIFO blocks the first write-tool call in `open`, with the
in-process write mutex held; a link to a regular file elsewhere gets that file overwritten while the
lock is held and emptied on release. Found by the round-2 review of the bounded-reads branch, after
the read side of the same file (`read_holder_record`, row 10 of the related issue) had been bounded.

## Class fit

A write whose target is chosen by repository content and is not checked for being a link or a
special file before it is opened. It is the write-side twin of the unbounded reads in the related
issue, and is filed unclassified for the same reason: no existing class names a path-type check
missing on the operation it protects.

## Symptom (Effect)

Reviewer evidence, then re-read at the bytes: holder record linked to a FIFO makes an uncontended
`acquire` hang in `write_holder_record` (while the async mutex is held on a tokio worker, so the
first write call hangs and later ones time out waiting for the mutex); linked to a regular file
outside the root, the outside file holds `<epoch_ms>\t<holder>` during the hold and is emptied on
release.

## Reproduction

Tests in `agent::write_guard::holder_record_bounded_tests`, run against the unfixed `write` and
`Drop` (one mutant at a time):
- record path is a link to a FIFO (and a FIFO itself): `acquire` followed by the guard's drop does
  not return within 15 s;
- record path is a link to a regular file outside the root: after the hold the outside file is
  empty.

## Environment

Linux, tree at `eefbe1082` (branch `fix/bounded-repo-reads-2026-10-20`).

## Root cause

`std::fs::write` is `open(O_WRONLY | O_CREAT | O_TRUNC)` which follows links, truncates before any
check can look at the target, and blocks on a FIFO with no reader. Two call sites:
`write_holder_record` (`write_guard.rs`, called by `acquire` after the flock is granted) and
`Drop for WriteGuard`. The flock is taken on `write.lock`, a different file, so the lock itself is
unaffected; the damage is to the holder record's target and to liveness.

## Evidence

- `write_guard.rs` at `eefbe1082`: `Drop` ran `std::fs::write(&self.holder_path, "")`;
  `write_holder_record` ran `std::fs::write(path, format!("{}\t{}", now_ms(), holder))`; `acquire`
  calls the latter with `let _ =` on the uncontended path.
- Red runs (byte-exact one-line reverts, restored with `git checkout --`, empty `git diff HEAD`
  after each):
  - `write_holder_record` back to `std::fs::write(..)`: the FIFO test failed with `acquire with a
    FIFO holder record did not return within 15s`, and the outside-file test failed.
  - `Drop` back to `std::fs::write(&self.holder_path, "")`: the outside-file test failed with
    `left: ""`, `right: "OUTSIDE-BYTES"`, and the FIFO test failed with the same deadline panic.
- Controls that stay green on the old code: the ordinary-file test (record present and parseable
  during the hold, emptied and not removed on release, a longer stale record fully replaced).

## Hypotheses tried

None needed.

## Fix

**Holder record** (commit `3a4f46cd8`, patch-id `c03f96bd380394f09898915f401d5b0376878b8c`):
`open_holder_record(path)` opens write + create with `O_NOFOLLOW | O_NONBLOCK` on unix (a link at the
leaf is `ELOOP`, a FIFO nobody reads is `ENXIO`), `fstat`s the opened descriptor and refuses
anything but a regular file, and only then truncates (`set_len(0)`; `O_TRUNC` on open would empty
a regular target before the check). Both `write_holder_record` and `Drop` go through it. A refusal
is best effort exactly as before (`let _ =`); the flock release in `Drop` runs either way. Other
platforms keep the plain open with truncate. Bytes written for an ordinary file are unchanged.

**Lock file** (commit `357d0c968`, patch-id `0dbe98632c15ed80e664206cedc93165a3965a20`; follow-up `9e070eeaa`, patch-id `3a2c798ca625d7151059b68eae1e7528ac41082d`, which applies the `ELOOP` message mapping only on unix because the identity `map_err` it left on Windows failed `clippy::map_identity`):
`open_lock_file` has the same shape and was fixed the same way. Probes by the reviewer on the old
code: a link to a FIFO did not hang (a read+write open of a FIFO does not block on Linux, so no
reader is needed to avoid the hang); a link to an existing outside file was flocked, not modified;
but a link to a path that DOES NOT EXIST created an empty file there, outside the root, and
`.codescout/` is committed content (127 tracked files in this repository), so a planted link is
realistic. It now opens `O_NOFOLLOW | O_NONBLOCK`, `fstat`s the descriptor and refuses anything but
a regular file, with an error naming the path only (no content). **Policy: a link is never unlinked
or replaced** (codescout does not delete user data); the user removes it. The `flock` is not
affected by `O_NONBLOCK`.

**What a refusal does, read at the bytes (`src/agent/mod.rs`, `src/server.rs`):** all three callers
call `open_lock_file(..).with_context(|| format!("failed to open write.lock for {}", ..))?`, so a
refused open FAILS the call; nothing runs without the lock. They are:
- `agent/mod.rs:943`, inside `Agent::new` (`:910`), which `server::run` (`server.rs:2928`) calls at
  `server.rs:2955` with the project falling back to `std::env::current_dir()` (`server.rs:2942`):
  **a link at `.codescout/write.lock` in the directory the server is launched from makes the MCP
  SERVER FAIL TO START**, not merely an activation fail;
- `agent/mod.rs:1095`, in `load_project_resources` (`:1076`), reached from `ensure_resident`
  (`:1135`, and another caller at `:1052`): the first pinned call to such a project fails;
- `agent/mod.rs:1729`, sub-project activation.

The exact text a user sees (provoked by the round-4 reviewer through `Agent::new` and
`Agent::activate`): `failed to open write.lock for <root>: <root>/.codescout/write.lock is a link;
codescout does not follow or replace it. Remove it so the lock file can be created`.

**This is a deliberate behaviour change.** The old plain open followed a link to a regular file
without complaint (reviewer probe: it opened fine), so anyone who linked `write.lock` on purpose
now cannot start or activate. Nothing in this repository or on this machine creates such a link:
no `symlink(` outside tests in `src/`, `scripts/` only `ln -sf` for LSP binaries, no document
describes a linked `write.lock`, and `write.lock` is gitignored (`.gitignore:120`; the holder at
`:125`), so a planted link has to come through a path that bypasses the ignore (a force-add, an
archive). The cost of the policy is that a planted link is a denial of service by the repository,
not a silent loss of exclusion; the user removes the link.

**Leaf only.** `O_NOFOLLOW` covers the last path component. A link in a PARENT directory
(`.codescout` itself a link to a directory elsewhere) is still followed, and both `write.lock` and
`write.lock.holder` are created there; the damage is limited to those two fixed names. The rustdoc
said "never through a link" before this was corrected.

## Tests added

`agent::write_guard::holder_record_bounded_tests`:
- `a_holder_record_that_is_a_fifo_or_a_link_to_one_does_not_hang_acquire_or_release` (under
  `within_deadline`),
- `a_holder_record_that_is_a_link_to_an_outside_file_leaves_it_untouched` (during the hold and
  after release; a dangling link target is not created),
- `an_ordinary_holder_record_is_written_during_the_hold_and_cleared_on_release`,
- `a_fifo_with_a_reader_is_refused_by_the_type_check_and_receives_nothing` (a FIFO that HAS a
  reader opens for writing, so only the `fstat` refuses it; asserts our message, not the later
  `ftruncate` failure, which has the same `ErrorKind` on Linux),
- lock file: `a_dangling_link_at_the_lock_file_is_refused_and_creates_nothing`,
  `a_link_to_an_outside_file_at_the_lock_file_is_refused_and_left_unmodified`,
  `a_fifo_or_a_link_to_one_at_the_lock_file_is_refused_without_a_hang`,
  `an_ordinary_lock_file_is_created_locked_and_reused_not_truncated`.

Red evidence (one byte-exact one-line mutant at a time, `git checkout --`, empty `git diff HEAD`
after each):
- `write_holder_record` back to `std::fs::write(..)`: FIFO test hung to the 15 s deadline, outside
  file test failed.
- `Drop` back to `std::fs::write(&self.holder_path, "")`: outside-file test failed
  (`left: ""`, `right: "OUTSIDE-BYTES"`), FIFO test hit the deadline.
- `open_holder_record`'s `fstat` check neutralised (`if false && ..`): only the FIFO-with-a-reader
  test failed (the other tests stay green because `ftruncate` fails on a FIFO anyway).
- `open_lock_file` without `O_NOFOLLOW`: the dangling-link and outside-file tests failed.
- `open_lock_file`'s `fstat` check neutralised: the FIFO test failed.
- Not mutated by me: the `ELOOP` message mapping. The round-4 reviewer ran mutants on it (the
  refusal kind changed to `Other`, in both the `fstat` branch and the `ELOOP` branch) and the tests
  killed them.
- Survives all tests, recorded as defence in depth: dropping `O_NONBLOCK` in `open_lock_file`. It
  is unobservable on Linux (a read+write open of a FIFO never blocks; a device leaf needs `mknod`
  or a link, and a link is refused by `ELOOP`).
- Platform note, untested: FreeBSD returns `EMLINK`, not `ELOOP`, for `O_NOFOLLOW` on a link, so
  the friendly message would not show there; the link is still refused, with the raw OS error.

## Workarounds

Do not link `.codescout/write.lock.holder`.

## Resume

Open, recorded and deliberately not fixed:

- **Hard link at the holder record** is followed (same inode). Reviewer probe: the other file read
  `<ts>\tholder-x` during the hold and `""` after release. Recommendation: do NOT refuse on
  `st_nlink > 1`; `cp -al`, `rsync --link-dest` and snapshot copies legitimately give every file a
  link count of 2, so a refusal would leave the record permanently anonymous there, while git never
  creates hard links in a checkout. If it is ever addressed: break the link (unlink, then recreate
  with `O_CREAT | O_EXCL | O_NOFOLLOW` under the lock).
- **`src/agent/build_check.rs:395-405` `cargo_lock_is_free`** opens `target/debug/.cargo-lock` read +
  write + create and follows links: the same class. `target/` is normally untracked, so it was left
  alone.
- A link in a parent directory (`.codescout`) is still followed (see Fix).

## References

- `docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md` (row 10, the read side)
