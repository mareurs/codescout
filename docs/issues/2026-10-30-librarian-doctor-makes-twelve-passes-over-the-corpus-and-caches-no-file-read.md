---
id: abd6326741730e7d
kind: bug
status: open
title: librarian doctor makes about 12 full passes over the corpus and caches no file read (avg 8.7 s)
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-30
severity: medium
unverified: The remaining 6.2 s was not profiled after the fixes. sha256 in scan_artifact_paths (7.5% of instructions), frontmatter::parse (5.4%), preview::headings::parse (6%) and libgit2 commit-list parsing (4.2%) are the next measured shares; the 2-3 s target is an estimate.
---

# BUG: librarian doctor makes about 12 full passes over the corpus and caches no file read (avg 8.7 s)

## Summary

`DoctorScope::read_text` (`src/librarian/tools/doctor/scope.rs:268-270`) reads through the door on every call with no cache. About 12 scans each run `SELECT … FROM artifact` over every row and read each file again, and `params_backed_ledgers` runs 4 times.

## Class fit

`cluster/unclassified`: no roster class names repeated I/O. What the case shows: independent scans, each correct alone, that share an input but not a read of it.

## Symptom (Effect)

usage.db: `librarian action=doctor` 57 calls, avg 8,692 ms, max 21,560 ms, 52 over 5 s. It is a read-only scan by default.

## Reproduction

`librarian(action="doctor")` on codescout, HEAD `b27329082`.

## Environment

Catalog: 6,646 artifacts, 6,605 files on disk totalling 105 MB.

## Root cause

Measured 2026-10-30 (release build, `codescout doctor --json`, catalog of 6,617 distinct `.md` files across projects, not just this repo):

- **File opens:** `strace -e openat` counted 93,883 opens of the 6,617 files, up to 19 per file, with about 2.1 million `readlink` calls (2.0 million errors) and 525,000 `statx` calls behind them. That was sys 3.4 s of an 11.3 s run.
- **CPU, the larger part (user 7.9 s):** callgrind over the whole run (74.3 G instructions, symbol build): `link_scan::extract::extract` was **53%** inclusive, called by `scan_undefined_entries` (20%), `scan_cited_prefix_with_no_definer` (19.7%) and `entry_indegree` (18.3%) among others; `entry_sections` 13%; `sha256` in `scan_artifact_paths` 7.5%; `frontmatter::parse` 5.4%; `preview::headings::parse` 6%. The same text was extracted three to five times.

## Evidence

`read_text` body spot-checked 2026-10-30.

## Hypotheses tried

None.

## Fix

Part done 2026-10-30; the issue stays open for the rest.

1. `ReadDoor::cache_reads` (`src/librarian/read_door.rs`): an opt-in memo of successful reads, off by default; the door's open-then-verify read stays uncached for every verb that writes. `doctor`'s report (`call`, after the `--fix` early return) turns it on.
2. `ExtractMemoScope` (`src/librarian/tools/link_scan/extract.rs`): a thread-local memo of `extract` and `entry_sections` keyed by a hash of the TEXT (plus length), so a changed file can never get a stale answer. `doctor`'s scans import the memoized forms under the plain names.

Measured (release builds, same tree, `doctor --json`): 11.1-11.4 s before; 9.76 s with part 1 alone (sys 3.4 s to 2.0 s, user unchanged); **6.2 s** (6.29 / 6.23 s steady, 8.4 s first run) with both. The report is byte-identical to the old binary's on the same tree (243 violations).

Not done: one parse per file shared by the scans (frontmatter, headings), the per-row sha256 in `scan_artifact_paths`, libgit2 commit-list parsing.

## Tests added

`read_door::memo_tests` (the default door reads the file as it is now; an opted-in door serves the second read from memory, proved with a changed file; a missing file is not remembered; one door's memo does not reach another) and `link_scan::extract::memo_tests` (nothing remembered outside a scope; memoized answers equal plain ones; a changed text gets its own answer; dropping the scope frees the memo). Not mutation-tested.

## Workarounds

None.

## Resume

Profile the 6.2 s again on a symbol build (`CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`; `valgrind --tool=callgrind` takes about 7 min). Next measured shares: sha256 in `scan_artifact_paths`, `frontmatter::parse`, `preview::headings::parse`, libgit2. A debugger attach fails here (`ptrace_scope=1`); launching the binary under `gdb` and sending it SIGINT works.

## References

N/A
