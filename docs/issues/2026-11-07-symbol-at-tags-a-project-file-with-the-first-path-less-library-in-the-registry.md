---
kind: bug
status: open
title: 'BUG: symbol_at tags a project file''s definition with the first path-less library in the registry (lib:pandas, lib:codescout_embed)'
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
opened: 2026-11-07
severity: low
unverified: Both defects now have tests, but only targeted tests on one Windows GNU toolchain were run; the full gate (clippy, both nextest runs, Linux CI) has not. Defect A's test is Windows-only. Not re-checked on a live symbol_at after the fix.
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

## Tests added
- `a_library_with_no_path_owns_no_file` (`src/library/registry.rs`): a path-less entry registered first must
  not own a file.
- `tag_external_path_returns_project_for_a_uri_spelled_path_under_a_verbatim_root` (`src/tools/symbol/tests.rs`,
  Windows only): the URI spelling `C:\...` under a `\\?\C:\...` root is tagged `project`.

## Workarounds
Ignore the `source` field on `symbol_at` for files that are plainly the project's own.

## Resume
Status stays `open` until the gate is green. Then: run the gate, confirm on a live `symbol_at` that `source` is
`project`, and check whether `symbols` consumes the same helper.

## References
- `fed1c5731a623c30` (same verbatim-prefix defect in `references`)
