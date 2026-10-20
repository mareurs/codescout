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
unverified: The fix is on branch fix/bounded-repo-reads-2026-10-20 only; not merged to experiments. A hard link at the record path to a file elsewhere is still followed (same inode, not detectable from here) and is untested. write.lock itself (open_lock_file) has the same shape and is NOT fixed. The reachability of the FIFO hang through a real tool call (as opposed to the acquire function) was not driven.
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

`open_holder_record(path)` opens write + create with `O_NOFOLLOW | O_NONBLOCK` on unix (a link at
the leaf is `ELOOP`, a FIFO nobody reads is `ENXIO`), `fstat`s the opened descriptor and refuses
anything but a regular file, and only then truncates (`set_len(0)`; `O_TRUNC` on open would empty
a regular target before the check). Both `write_holder_record` and `Drop` go through it. A refusal
is best effort exactly as before (`let _ =`); the flock release in `Drop` runs either way. Other
platforms keep the plain open with truncate. Bytes written for an ordinary file are unchanged.
Commit `3a4f46cd8`.

## Tests added

`agent::write_guard::holder_record_bounded_tests`:
- `a_holder_record_that_is_a_fifo_or_a_link_to_one_does_not_hang_acquire_or_release` (under
  `within_deadline`),
- `a_holder_record_that_is_a_link_to_an_outside_file_leaves_it_untouched` (during the hold and
  after release; a dangling link target is not created),
- `an_ordinary_holder_record_is_written_during_the_hold_and_cleared_on_release`.

## Workarounds

Do not link `.codescout/write.lock.holder`.

## Resume

Open: `open_lock_file` opens `.codescout/write.lock` with `read + write + create` and follows a
link too (a FIFO there with a reader or an outside file is flocked, not truncated); not changed
here. A hard link at the record path to a file elsewhere is followed and not detectable by the
open flags; a `st_nlink != 1` refusal would close it and was not added.

## References

- `docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md` (row 10, the read side)
