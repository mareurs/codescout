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
unverified: Which scan dominates the 8.7 s is not measured; the 2-3 s target is an estimate.
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

Scans that each read every file (librarian explorer, 2026-10-30): `entry_indegree` (`src/librarian/tools/doctor.rs:4523`), `corpus_cited_tokens` (`:4342`), entry-validity scans (`:4945`, `:5104`, `:5232`, `:5351`), `scan_cited_prefix_with_no_definer` (`:5858`), `scan_premature_archive_citation` (`:6133`), `scan_unterminated_fence` (`:8516`), `scan_frontmatter_id_mismatches` (`:6577`), `scan_frontmatter_status_mismatches` (`:4061`); `check_row_behind_file` reads and hashes again (`:3859`); `params_backed_ledgers` at `:5429`, `:5549`, `:6262`, `:6479`. One raw pass over the corpus: 0.16 s (`sha256sum`), 0.5 s (`wc -l`). Which scan dominates is not measured.

## Evidence

`read_text` body spot-checked 2026-10-30.

## Hypotheses tried

None.

## Fix

A per-call `HashMap<PathBuf, Option<Arc<str>>>` in `DoctorScope`, and one parse per file shared by the scans. Corpus-wide metrics unchanged.

## Tests added

None yet.

## Workarounds

None.

## Resume

Add a tracing span per scan (or `strace -c -e openat` on a CLI doctor run) to find which scans dominate before caching.

## References

N/A
