---
id: c052a7c89e3b4a4d
kind: bug
status: fixed
title: concurrent_producers_have_untorn_distinct_records fails when two producers contend for the 100 ms lock budget
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-10
opened: 2026-11-10
---

# BUG: `concurrent_producers_have_untorn_distinct_records` fails when two producers contend for the 100 ms lock budget

## Summary

`tests/observation_store.rs::concurrent_producers_have_untorn_distinct_records` demands that all 24 appends of two tight-loop producers succeed. `secure::lock` in `src/observations/store.rs` gives each append a bounded wait of 100 ms and then returns `Err("unavailable")`. When one append holds the lock for about 45 ms (three fsyncs on btrfs), the other producer loses at least two lock races in a row and times out. The test then panics at `tests/observation_store.rs:296`.

## Class fit

No existing class fits. IC-5 (reproduction environment differs from the gating environment) does not apply, because the failure reproduces in the gate and in a local run alike. The test assumes an unbounded wait while the store promises a bounded one, so `cluster/unclassified` is the honest tag.

## Symptom (Effect)

- Producer thread panics: `called Result::unwrap() on an Err value: "unavailable"` at `tests/observation_store.rs:296:40`.
- The test thread then panics at line 302 on the `join().unwrap()`.
- `./scripts/gate.sh` LEAN lane (2026-11-10): 6,382 of 6,383 passed, this test the only failure, exit 100.

## Reproduction

`scripts/with-slot.sh cargo test --no-default-features --test observation_store concurrent_producers` with `TMPDIR=/home/marius/.cache/cc-tmp`, which is on btrfs. Failed in 5 of 6 runs on 2026-11-10: the gate LEAN lane, a re-run, a working tree with default features, a clean worktree at HEAD `01dcbaf5e` with no edits, and a run under `strace`. Passed once, in the gate DEFAULT lane (nextest, tmpfs `TMPDIR`).

## Environment

- Linux 7.2.8-zen1, `/home` btrfs at 90% use, `/tmp` tmpfs, gate pool ext4 on zram.
- Rust toolchain of the pool slot, run with the test binary built from HEAD `01dcbaf5e`.

## Root cause

Established from an `strace -f -tt -T` of the test binary (flock, fsync, openat, statx):

- `secure::lock` polls `flock(LOCK_EX|LOCK_NB)` every 2 ms and gives up after 100 ms.
- `append` holds that lock across `file.sync_all()`, `dir.sync_all()` and `write_control`. The slowest fsync in the trace was 15.6 ms.
- The producer loop calls `append` again within microseconds of the release, so a waiter that polls every 2 ms wins the lock only by chance.
- Measured in the failing run: thread 950467 waited 101.2 ms and gave up. Thread 950468 waited 86.4 ms and only just acquired. 87 flock calls returned EAGAIN; the two threads made 49 and 43 flock calls.
- On tmpfs an append takes well under 1 ms, so the waiter always gets in, which is why the nextest DEFAULT lane passed.

The 100 ms bound is the store's design: a producer must not block the observed tool call for long, and a contention timeout loses one observation. Nothing in the repository states the bound's intent, and I did not change it.

## Evidence

- `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/4f8c9678-b417-474d-b9af-50956d939669/scratchpad/obs.strace` and `lockwaits.py` (session scratchpad, not kept).
- Clean-worktree run: `@bg_0000000a` of that session, exit 101 in both feature sets.

## Hypotheses tried

- A slow btrfs alone: weakened, because the gate LEAN lane failed with `TMPDIR` on tmpfs too (load made the timing worse).
- A defect from the 2026-11-10 prompt edits: refuted, the test fails on a clean HEAD worktree.

## Fix

Test-side only. `concurrent_producers_have_untorn_distinct_records` now counts the appends the store acknowledged. It accepts `Err("unavailable")` (a lock timeout, which writes nothing) and fails on any other error. It asserts that the number of stored records equals the number of acknowledged appends, that there are no diagnostics, and that the ids are distinct. This keeps the real invariant (no torn, lost or phantom record) and drops the assumption that two tight-loop producers never time out.

The store is unchanged, so the 100 ms bounded wait stays as designed. Whether the wait should be fair under contention (the 2 ms poll starves a waiter while the holder re-locks at once) is a product question that nothing in the repository decides, and this fix does not answer it.

## Tests added

- `a_held_quota_lock_times_out_as_unavailable_and_leaves_no_trace` in `tests/observation_store.rs`. An external `flock` on `quota.lock` makes `append` return `unavailable` after at least 100 ms, with no record stored. After the lock is released, the next append stores sequence 0, so the timeout left no gap.
- Mutation probe: the lock budget cut from 100 ms to 10 ms makes this test fail (`tests/observation_store.rs:327`). The restored `store.rs` was byte-identical.
- Verification: both tests 10 of 10 on btrfs `TMPDIR` in both feature sets; whole file 17 passed; full `./scripts/gate.sh` exit 0 (LEAN 6,384, DEFAULT 9,287, Python 1,827).

## Workarounds

Run the test with `TMPDIR` on tmpfs.

## Fix provenance

- **SHA:** `7a8a07b09` (`experiments`)
- **patch-id:** `3f0f7898dcd970b24015c530b2418497e915c2d1`

## Resume

Closed. The fix is committed on `experiments` (see Fix provenance), the full gate was green before the commit, and the file is archived. One question stays open and is recorded only in the Fix section: should the 100 ms lock wait in `secure::lock` be fair under contention? Nothing in the repository decides it. Reopen this bug if a contended `append` is seen to lose observations in real use.

## References

- `src/observations/store.rs`: `secure::lock`, `ObservationStore::append`.
- `tests/observation_store.rs`: `concurrent_producers_have_untorn_distinct_records`.
