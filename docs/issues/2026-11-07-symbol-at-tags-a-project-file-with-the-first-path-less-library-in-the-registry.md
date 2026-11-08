---
kind: bug
status: fixed
title: 'BUG: symbol_at tags a project file''s definition with the first path-less library in the registry (lib:pandas, lib:codescout_embed)'
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
closed: 2026-11-07
opened: 2026-11-07
severity: low
unverified: 'Defect A (verbatim prefix) was run only under Wine 11.21, never on Windows: red on the pre-fix line, green with the fix. Wine is a re-implementation of the Windows path layer, so this is evidence of the mechanism, not proof of the Windows behaviour. Defect B (empty path) was seen fixed live on Linux. `symbols` was not run live.'
---

# BUG: symbol_at tags a project file's definition with the first path-less library in the registry (lib:pandas, lib:codescout_embed)

## Summary
`symbol_at` (definition and hover) marks the definition of a symbol that lives in the project's own
file with `source: "lib:<name>"`, where `<name>` is an unrelated library: the first registry entry
whose `path` is empty. The label is wrong, and it also triggers the library nudge and
`maybe_auto_index_library` for a library the file has nothing to do with.

## Class fit
The tag is a per-item attribute (which library owns THIS path) read off the container (the registry
as a whole), so the first path-less entry is stamped on every path. It is exactly right for the one
path that really lies under that entry and wrong for all others, which is the class's signature. It
differs from the founding member in that the container is a list searched with `find`, not a span
emitted by a parser.

## Symptom (Effect)
Two independent sessions, same call shape, different wrong labels: `symbol_at` on a definition in a Python
project file returned the right definition and signature, tagged `(lib:pandas)` in one project and
`(lib:codescout_embed)` in another.

## Reproduction
Observed on Windows with pyright 1.1.413. Call `symbol_at` on a definition inside a project file while the
project's `.codescout/libraries.json` holds `manifest_scan` entries with `"path": ""`.

## Environment
Windows (project root held as `\\?\C:\...`), Python project registered with `manifest_scan` libraries.

## Root cause
Two defects that combine.
- A: `tag_external_path` (`src/fs/mod.rs`) opened with `path.starts_with(project_root)`. `project_root` is the
  canonical `\\?\C:\...` spelling and the definition path comes from an LSP URI (`C:\...`); `Path::starts_with`
  compares components and `VerbatimDisk` != `Disk`, so a project file was not seen as inside the project.
  `crate::util::fs::strip_verbatim` exists for exactly this and `references` already uses it.
- B: `LibraryRegistry::is_library_path` (`src/library/registry.rs`) did
  `entries.find(|e| target.starts_with(comparable(&e.path)))`. An entry with an empty `path` (every
  `manifest_scan` entry without resolved sources) is a prefix of every path, so the first one won.

## Evidence
- One project's `.codescout/libraries.json`: first entry `pandas`, `"path": ""`, `manifest_scan`.
- This repository's `.codescout/libraries.json`: 59 entries; the entries with an empty path are
  `codescout_embed`, `qdrant_client`, `open`, in that order, and `codescout_embed` is the label observed.

## Hypotheses tried
None beyond the above.

## Fix
- A: `tag_external_path` compares `crate::util::fs::strip_verbatim(path)` against `strip_verbatim(project_root)`
  in its first check.
- B: `LibraryRegistry::is_library_path` skips entries whose `path` is empty.

## Fix provenance

- **SHA:** `b5c67ecdef6dc5b7e55d51b9e2e78e2a12cbe65d` (`experiments`)
- **patch-id:** `d41fa914791321e4d5f13802bdbeff107a668729`

The commit is a portability subset from PR 33 and also holds the Windows pid-alive probe and a subshell producer status. Another session made it.

## Tests added
- `a_library_with_no_path_owns_no_file` (`src/library/registry.rs`): a path-less entry registered first must
  not own a file.
- `tag_external_path_returns_project_for_a_uri_spelled_path_under_a_verbatim_root` (`src/tools/symbol/tests.rs`,
  Windows only): the URI spelling `C:\...` under a `\\?\C:\...` root is tagged `project`.

## Workarounds
Ignore the `source` field on `symbol_at` for files that are plainly the project's own.

## Resume

Gate green on Linux on 2026-11-07 (Python, fmt, clippy, LEAN and DEFAULT nextest). Live: after a rebuild at 21:30, `symbol_at` on the call of `display_path` at `src/tools/grep.rs:289` returns the definition `src/util/fs.rs:567` with no `lib:` tag, in a project whose `.codescout/libraries.json` has path-less entries (`codescout_embed` first).

`symbols` does not call `tag_external_path` (its callers are `symbol_at.rs` lines 164 and 282). The other callers of `is_library_path` are `src/fs/mod.rs` line 542, inside `tag_external_path`, and `src/tools/read_file.rs` line 1085. The second one gets the defect B fix through the registry.

Wine run 2026-11-08 (Wine 11.21, `cargo test --lib --no-default-features --target x86_64-pc-windows-gnu`, filter `tag_external_path_returns_project_for_a_uri`, at `b75ccae18`): with the fix both tests pass, 2 of 2. With the pre-fix line `if path.starts_with(project_root)` in a throwaway worktree, both fail with `left: "lib:src"`, `right: "project"`, which is the reported symptom. So under Wine the test discriminates, and it does so for the second test too, which on Linux passes without exercising the defect.

Still open: run the Windows-only test on a real Windows host or in CI. Wine re-implements the Windows path layer; it is not Windows.

## References
- `fed1c5731a623c30` (same verbatim-prefix defect in `references`)
