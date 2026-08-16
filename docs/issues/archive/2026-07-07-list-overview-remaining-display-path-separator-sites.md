---
id: '8e7c6b1b041eebd7'
kind: bug
status: fixed
title: 'BUG: two more list_overview.rs display-string sites use Path::display() instead of forward-slash normalization'
tags:
- windows
- cross-platform
- test-portability
- symbols-tool
- follow-up
closed: 2026-07-07
opened: 2026-07-07
owner: marius
related:
- docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md
severity: low
---


# BUG: two more list_overview.rs display-string sites use Path::display() instead of forward-slash normalization (same class as BUG 3bf933638427fd21)

## Summary

Same bug class as
[`2026-07-07-windows-glob-overview-path-separator-test-mismatch.md`](2026-07-07-windows-glob-overview-path-separator-test-mismatch.md)
(id `3bf933638427fd21`), fixed in that file for the glob-overview branch.
Two sibling call sites in the same file build path-derived display strings
the same platform-dependent way, but currently have no test pinning a
forward-slash literal against them, so they don't fail CI today — they're
latent, not (yet) observed failures.

## Symptom (Effect)

None observed yet — no failing test. On Windows, the affected fields would
render `\`-separated instead of `/`-separated, same shape as the fixed bug.

## Reproduction

Not yet reproducible — no test exercises these two sites with a
literal-separator assertion. Best lead: write a Windows-path-literal test
analogous to `symbols_overview_glob_marks_grammarless_language_warming_instead_of_dropping_file`
targeting the `directory_map` overview mode (site 1) and the non-glob
directory symbol-mode listing (site 2).

## Environment

Windows only, same as the sibling bug.

## Root cause

Two more `.display().to_string()` call sites in
`src/tools/symbol/list_overview.rs`, both building path-shaped strings
surfaced in tool output:

1. `list_overview.rs:137-141` — `count_files_by_subdir()`'s per-subdirectory
   display string, used by the `directory_map` overview mode (medium-size
   directories, `LIST_SYMBOLS_RECURSE_MEDIUM` threshold).
2. `list_overview.rs:526-531` — `dir_files` display string, non-glob
   directory overview's symbol-mode file listing (small directories,
   `LIST_SYMBOLS_RECURSE_SMALL` threshold).

Same fix pattern applies: swap `.display().to_string()` for
`crate::util::fs::to_forward_slash(...)`.

## Evidence

N/A — not yet reproduced; see Root cause for the exact `path:line` citations
found while fixing the sibling bug.

## Hypotheses tried

N/A — not yet investigated beyond locating the call sites.

## Fix

Implemented. Both sites now route through the new `crate::util::fs::relative_forward_slash(path, root)` helper (extracted during the broader Windows path-separator audit, commit `edb44a9b`): `list_overview.rs:137-141` (`count_files_by_subdir`'s subdir display strings) and `list_overview.rs:522-528` (`dir_files` display string in the non-glob directory symbol-mode listing). Fixed as part of the same session that ran the full-codebase audit (see `docs/trackers/bug-fix-session-log.md` F-27 for provenance).
## Tests added

No new test added - same precedent as the sibling glob-overview fix: the normalization primitive (`relative_forward_slash`/`to_forward_slash`) already has dedicated cross-platform-safe unit tests in `src/util/fs.rs`, and the full `cargo test --lib` suite (2952 passed / 0 failed / 6 ignored) stayed green before and after.
## Workarounds

None needed — no observed failure yet.

## Resume

Apply `crate::util::fs::to_forward_slash()` at `list_overview.rs:137-141`
and `526-531` (same import already added at the top of the file by the
sibling fix). Add a regression test per site before or alongside the fix.

## References

- [`2026-07-07-windows-glob-overview-path-separator-test-mismatch.md`](2026-07-07-windows-glob-overview-path-separator-test-mismatch.md) (`3bf933638427fd21`) — sibling bug, same file, same class, already fixed.
- `docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md` — original bug class.
