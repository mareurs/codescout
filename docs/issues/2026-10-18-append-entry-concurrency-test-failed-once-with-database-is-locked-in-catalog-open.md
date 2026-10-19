---
kind: bug
status: open
title: 'BUG: append_entry_serializes_across_independent_connections_to_same_file failed once with "applying schema: database is locked" inside Catalog::open, in a full cargo test --lib run'
tags:
- cluster/unclassified
- flaky-test
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-two-subagent-notice-guide-hint-tests-flake-under-build-load.md
severity: low
---

# BUG: append_entry_serializes_across_independent_connections_to_same_file failed once with "applying schema: database is locked" inside Catalog::open, in a full cargo test --lib run

## Summary

In one whole `cargo test --lib` run on 2026-10-18 (7406 passed, 3 failed, 426 s), `librarian::catalog::augmentation::tests::append_entry_serializes_across_independent_connections_to_same_file` failed: one of its two threads got `applying schema: database is locked` from `Catalog::open` (not from `append_entry`). The same test passed in isolation, and in three other whole-suite runs of the same tree (852 s, 376 s and 435 s wall clock; the failing run took 426 s). Not reproduced in a stress loop (below). Whether this is test flake or a window in which two real processes opening the same catalog fail is not established.

## Class fit

No class on the roster was walked one by one, so the sanctioned `cluster/unclassified` is used. What the case shows: a concurrency test whose failure is in the setup step it does not mean to test (opening a connection), so a green or red result says little about the property in its name.

## Symptom (Effect)

```
thread '<unnamed>' panicked at src/librarian/catalog/augmentation.rs:5127:49:
called `Result::unwrap()` on an `Err` value: applying schema

Caused by:
    0: database is locked
    1: Error code 5: database is locked
thread 'librarian::catalog::augmentation::tests::append_entry_serializes_across_independent_connections_to_same_file' panicked at src/librarian/catalog/augmentation.rs:5141:29:
called `Result::unwrap()` on an `Err` value: Any { .. }
```

Line 5127 is the `Catalog::open(&path2).unwrap()` inside the second spawned thread; 5141 is the `h2.join().unwrap()`.

## Reproduction

None reliable. `cargo test --lib` (whole suite) on a loaded machine: failed in 1 of 4 whole-suite runs of this tree. Isolated: passes. Stress, measured 2026-10-18: the test binary built from this tree, 16 concurrent workers x 60 runs each, one test per process (`--exact ... --test-threads 1`), 960 runs, 0 failures.

## Environment

Linux, branch `fix/small-items-2026-10-18` (base `experiments` 7a09623bd), `scripts/with-slot.sh cargo test --lib`, other cargo and agent sessions running on the same machine.

## Root cause

Unknown: not measured. What is established is where it fails (`Catalog::open`, schema application, SQLite error code 5) and that 0 of 960 isolated stress runs failed, so the window needs something the isolated runs lack (the rest of the suite running in the same process, or heavier machine load).

**Second sighting, 2026-10-18** (a whole `cargo test --lib -- --nocapture` run on the same tree, 1363 s under heavy machine load, 7408 passed, 3 failed): `librarian::catalog::augmentation::tests::allocate_entry_id_serializes_across_independent_connections`, the sibling concurrency test in the same file, failed with the same signature: `augmentation.rs:5451:46`, `called Result::unwrap() on an Err value: applying schema ... database is locked ... Error code 5`, then `5459:38` the `join().unwrap()`. Both tests open one fresh catalog file from two threads at once. The other two failures in that run were the known `two-subagent-notice` pair.

What `Catalog::open` does (read, `src/librarian/catalog/mod.rs` ~648): `Connection::open`, then ONE batch `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;`, then `execute_batch(SCHEMA_SQL)` (the `applying schema` context). So a 5000 ms busy timeout is in force when the schema statement fails; whether it expired under that load, or SQLite did not consult the busy handler for this statement, was not measured.

## Evidence

The log excerpt above, from `lib.log` of the 2026-10-18 final verification run. A similar `database is locked` flake on a different test is archived as `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` (wontfix, not recurred); this is a different test and a different lane, so it is filed separately, not reopened there.

## Hypotheses tried

1. **Hypothesis:** the test fails on its own under concurrent load. **Test:** 960 isolated runs, 16 at a time. **Verdict:** rejected for that load; the failing run had the whole suite in-process.

## Fix

None yet.

## Tests added

N/A: open, unreproduced.

## Workarounds

Re-run the test alone; it passes.

## Resume

1. Run the whole `--lib` suite in a loop on a quiet machine to see whether it recurs without external load. 2. `busy_timeout` is already set (5000 ms) before the schema statement, so the open question is why `database is locked` surfaces anyway: measure how long a failing open waits (a 5 s wait means the timeout expired; an immediate failure means the busy handler was not consulted), then decide between a longer timeout, retrying the open, or serialising the first schema application.

## References

- `src/librarian/catalog/augmentation.rs` (the test, around line 5096)
- `docs/issues/2026-10-17-two-subagent-notice-guide-hint-tests-flake-under-build-load.md`
