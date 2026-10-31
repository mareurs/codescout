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
unverified: The user-time shares are callgrind instruction counts, not seconds; the sha256 share in particular is overstated under valgrind. The saving of part 3 is measured as CPU time over three runs on a shared machine; wall time is not.
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

3. `ReadDenyList` (`src/util/path_security.rs`), held by each `ReadDoor` in a `OnceLock` (2026-10-31). Profiling the 6.2 s run showed 2,054,784 `readlink` calls (2,032,491 errors) behind only 7,431 `validate_read_path` calls: each call re-resolved the whole read deny list, about 276 `readlink` calls per file. The door now resolves the list once per door. `validate_read_path` itself is unchanged.

Measured 2026-10-31 (release builds, three alternating runs each, shared machine): `sys` 2.83 / 2.06 / 2.40 s before, 1.54 / 1.36 / 2.03 s after; `user` 4.45 / 4.20 / 4.44 s before, 4.23 / 4.14 / 4.37 s after. CPU time (user + sys) fell by about 0.8 s, about 12%. Wall time is too noisy to state on a shared machine (5.5-9.0 s after, 6.3-11.7 s before). The two reports differ only in live peer state that changed between the runs (a claim ended, one bug came under live edit, audit row counts).

Not done: `user` time, 4.2 s. Instruction shares from callgrind (74.3 G before the fixes, 43.9 G after; callgrind counts instructions, and its `sha2` soft path overstates the hash because valgrind hides SHA-NI): `link_scan::extract::extract` 31.8% (one pass over the corpus now, `scan_tokens` regex 16%), `scan_artifact_paths` 18.3% (sha256 12.8%), `scan_open_bug_cited_from_source_walking` 16.7%, `frontmatter::parse` 6.4% (serde_yml), `content_key` 4.3% (the memo's own hash), `ReadDoor::bytes` copy 3-4%, `gc::detect_move_candidates` 3.8%, `headings::parse` 2.9%.

## Tests added

`read_door::memo_tests` (the default door reads the file as it is now; an opted-in door serves the second read from memory, proved with a changed file; a missing file is not remembered; one door's memo does not reach another) and `link_scan::extract::memo_tests` (nothing remembered outside a scope; memoized answers equal plain ones; a changed text gets its own answer; dropping the scope frees the memo). The first two groups were not mutation-tested. For part 3: `read_door::deny_list_tests::a_door_resolves_the_deny_list_once_however_many_files_it_judges` (a thread-local counter on `ReadDenyList::resolve`; resolving per file instead reads 5, not 1: verified by mutation, reverted) and `path_security::tests::a_resolved_deny_list_refuses_what_validate_read_path_refuses`. The door's refusal itself stays covered by `deny_listed_target_inside_a_managed_root_reaches_no_read_site`.

## Workarounds

None.

## Resume

Part 3 is done; the next lever is `user` time (see the shares under Fix). Profile again on a symbol build (`CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`; `valgrind --tool=callgrind` takes about 7 min). Next measured shares: sha256 in `scan_artifact_paths`, `frontmatter::parse`, `preview::headings::parse`, libgit2. A debugger attach fails here (`ptrace_scope=1`); launching the binary under `gdb` and sending it SIGINT works.

## References

N/A
