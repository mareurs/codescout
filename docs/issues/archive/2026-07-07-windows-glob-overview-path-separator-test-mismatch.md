---
id: '3bf933638427fd21'
kind: bug
status: fixed
title: 'BUG: symbols glob overview builds "file" JSON field via Path::display(), breaking on Windows path separators'
owners: []
tags:
- windows
- cross-platform
- test-portability
- symbols-tool
topic: null
time_scope: null
closed: '2026-07-07'
opened: '2026-07-07'
owner: marius
related:
- docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md
severity: low
---


# BUG: symbols glob overview builds "file" JSON field via Path::display(), breaking on Windows path separators

## Summary

`src/tools/symbol/list_overview.rs`'s glob-overview branch (all three LSP-readiness
sub-cases: ready-with-symbols, warming-with-tree-sitter-fallback, warming-no-grammar)
built the `"file"` field of each result entry with `rel.display().to_string()`.
`Path::display()` renders OS-native separators, so on Windows the field held
`src\legacy.c` instead of `src/legacy.c`. Any code (tests, downstream tooling)
comparing that field against a forward-slash literal fails on Windows only.

## Symptom (Effect)

```
tools::symbol::tests::symbols_overview_glob_marks_grammarless_language_warming_instead_of_dropping_file ... FAILED
```
on Windows CI/dev machines. Test asserts:
```rust
.find(|f| f["file"].as_str() == Some("src/legacy.c"))
```
which never matches when the response field is `src\legacy.c`.

## Reproduction

On Windows (or by inspection — this class of bug does not reproduce on
Linux/macOS since `Path::display()` already emits `/` there):
```bash
cargo test --lib tools::symbol::tests::symbols_overview_glob_marks_grammarless_language_warming_instead_of_dropping_file
```
Commit at time of report: `33eca3e2` (branch `experiments`).

## Environment

Windows only (any Rust toolchain). Linux/macOS unaffected — `Path::display()`
is a no-op w.r.t. separators there, which is exactly why this shipped
unnoticed; same failure mode as the archived
`ci-windows-test-portability-rot` tracker.

## Root cause

Three call sites in the glob-overview branch of `list_overview()` used
`rel.display().to_string()` to build the JSON `"file"` field:

- `src/tools/symbol/list_overview.rs:268` (LSP-ready, has symbols)
- `src/tools/symbol/list_overview.rs:295` (LSP warming, tree-sitter fallback)
- `src/tools/symbol/list_overview.rs:310` (LSP warming, no tree-sitter grammar)

`Path::display()` is platform-native (backslash on Windows). The codebase
already has a canonical fix for exactly this class of bug —
`crate::util::fs::to_forward_slash()` — used throughout `src/librarian/`
for the same reason (catalog `abs_path` / LIKE-pattern matching must be
separator-stable across platforms). These three sites simply hadn't been
migrated to it.

Two sibling call sites in the same file build `"file"`-shaped display
strings the same (buggy) way, but are not covered by any test that
pins a forward-slash literal, so they don't currently fail CI:
- `src/tools/symbol/list_overview.rs:137-141` (`count_files_by_subdir`'s
  subdir display strings, used in the directory_map overview mode)
- `src/tools/symbol/list_overview.rs:526-531` (`dir_files` display
  strings, non-glob directory overview symbol-mode listing)

Tracked as a follow-up rather than fixed here — see References.

## Evidence

- `src/tools/symbol/list_overview.rs:106-119` (`crate::util::fs::to_forward_slash`
  doc comment): "Always replaces `\` with `/`, on every platform... Used at
  the boundary between filesystem paths and string representations stored
  in the catalog DB or returned in MCP responses." — this is exactly the
  boundary the three glob-overview sites cross.
- `src/util/fs.rs` tests `to_forward_slash_converts_backslashes_on_any_platform`,
  `to_forward_slash_is_idempotent` confirm the helper is host-OS-independent
  (operates on the string form, not OS path parsing).

## Hypotheses tried

1. **Hypothesis:** `rel.display().to_string()` at `list_overview.rs:310` (the
   line named in the inherited diagnosis) renders backslashes on Windows,
   breaking the literal-forward-slash test assertion.
   **Test:** Read the test (`src/tools/symbol/tests.rs:5008-5070`) and the
   three call sites directly; confirmed all three build the same `"file"`
   field the same way in parallel branches of the same `if is_glob` block.
   **Verdict:** confirmed.
   **Evidence link:** see Root cause above.

## Fix

Replaced `rel.display().to_string()` with `crate::util::fs::to_forward_slash(rel)`
at all three glob-overview call sites (`list_overview.rs:268,295,310`), plus
the corresponding `use` import. No behavior change on Linux/macOS (helper is
a no-op there — full `cargo test --lib` run stayed green, 2950 passed / 0
failed / 6 ignored, before and after). Implemented in the working tree on
`experiments` (commit `33eca3e2` base), not yet committed.

## Tests added

No new test added. The existing test
`tools::symbol::tests::symbols_overview_glob_marks_grammarless_language_warming_instead_of_dropping_file`
(`src/tools/symbol/tests.rs:5008`) is the regression test — it already
asserts the forward-slash literal and is what would fail on Windows without
this fix. The normalization primitive itself (`to_forward_slash`) already
has dedicated cross-platform-safe unit tests in `src/util/fs.rs:255-274`
that exercise literal backslash input on any host OS.

## Workarounds

None needed once the fix lands. Prior to the fix, Windows users would see
this specific test fail under `cargo test --lib`; the underlying `symbols`
tool call itself still worked (file was correctly present in the overview),
only the separator form was wrong.

## Resume

N/A — fixed. Follow-up: the two sibling `.display()` sites noted in Root
cause (`list_overview.rs:137-141`, `526-531`) share the same bug class but
aren't covered by a failing test; consider migrating them to
`to_forward_slash` too in a future pass.

## References

- `docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md` — same
  bug class (Windows path-separator test non-portability), prior occurrence.
- `src/util/fs.rs` — `to_forward_slash` / `RepoPath`, the established fix
  pattern for this bug class.
- `docs/trackers/bug-fix-session-log.md` (`2dd9d90bc83f9f49`) F-27 — records
  that a prior session's claim to have already logged this bug (and a
  second, memory-tool bug) did not actually land anywhere in the repo.
