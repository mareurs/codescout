---
id: f9abe7185324eec2
kind: bug
status: fixed
title: grep rewrites a backslash in a file name to a slash, so the listed path names a file that does not exist
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-07
opened: 2026-11-04
unverified: 'Fixed, gated and seen live in grep, tree and the references completeness_warning (src/a\b.rs listed with its backslash, 2026-11-07), but gaps remain. (1) Not every one of the 13 sites of a6babe715 is proven reached by a test: the lsp warming error entry in list_overview.rs and the paths in the WalkClock cut notice in grep.rs probably are not, and no per-site mutation was run for them. (2) The references.rs warning site fixed in 665a7758e has no test of its own; it was seen live only. (3) The callers fixed in d05072221 are tracked in bb462b414e559152, with their own caveat.'
---

# BUG: grep rewrites a backslash in a file name to a slash, so the listed path names a file that does not exist

## Summary

`grep` (files mode, and the default content mode) reports every matching file through `to_forward_slash(entry.path())`. On Linux a backslash is an ordinary file-name character, so a file named `a\b.txt` is listed as `a/b.txt`: a different path that does not exist. A caller that feeds the listed path to `read_file` or `edit_file` addresses the wrong file. Observed incidentally in Task 4 of the harness-delivered-hints server slice; re-verified below.

## Class fit

`cluster/addressing-without-an-escape-hatch`. The path field is a name the caller feeds back as an address, and the display form turns a legal file-name character into the separator. A name containing a backslash can no longer be written down, so the file cannot be addressed from the tool's own output.

## Symptom (Effect)

Listed path differs from the real path:

```
grep(mode="files", pattern="needle", path=<dir>)  ->  <dir>/a/b.txt
grep(pattern="needle", path=<dir>)                ->  <dir>/a/b.txt (1)   1: needle
```

The file on disk is `<dir>/a\b.txt`.

## Reproduction

Run 2026-11-05 on branch `hints/server-slice` (source identical to the running server's at this site), Linux:

```
mkdir -p $SCRATCH/bs-check && printf 'needle\n' > "$SCRATCH/bs-check/a\\b.txt"
ls -b $SCRATCH/bs-check          # a\\b.txt   (one literal backslash, ls -b escapes it)
grep(mode="files", pattern="needle", path="$SCRATCH/bs-check")
grep(pattern="needle", path="$SCRATCH/bs-check")
```

Both greps printed `$SCRATCH/bs-check/a/b.txt`; no output named `a\b.txt`. `$SCRATCH` was the session scratchpad directory outside the project.

## Environment

Linux 7.2, codescout 0.15.0, branch `hints/server-slice`.

## Root cause

measured 2026-11-05: the repro above printed `a/b.txt` for a file whose name is `a\b.txt`. The mechanism is read from `src/tools/grep.rs`, not otherwise instrumented: the per-file count key (`counts.entry(to_forward_slash(entry.path()))`, ~L289), and the `"file": to_forward_slash(entry.path())` of each content match (~L352, ~L396, ~L421) pass the real path through `crate::util::fs::to_forward_slash`, which replaces every `\` with `/` regardless of platform. Intended for Windows separators, it also rewrites a backslash that is part of a Unix file name. Its own doc comment (`src/util/fs.rs`, `to_forward_slash`) records this as a caveat and bounds it to "string matching against the catalog — not file IO"; grep returns the rewritten string in its MCP response, outside that bound, so the caveat does not cover this use (read from the doc comment, not measured).

## Evidence

The two grep results quoted in Symptom, against `ls -b` showing the single literal backslash.

## Hypotheses tried

1. **Hypothesis** the rewrite is Windows-only. **Test** the repro on Linux. **Verdict** rejected: it happens on Linux.

## Fix

Two new functions in `src/util/fs.rs`:

- `display_path(&Path) -> String`: on Windows it is `to_forward_slash`; on every other platform it returns the path unchanged.
- `relative_display_path(&Path, &Path) -> String`: `strip_prefix` then `display_path`.

`to_forward_slash` is untouched, because the catalog stores the forward-slash form and 39 files depend on it. The listing tools now use the display form at 13 sites: `src/tools/grep.rs` (6: the per-file count key, three `"file"` fields, and the two paths in the `WalkClock` cut notice), `src/tools/tree.rs` (2: directory listing and glob), `src/tools/symbol/list_overview.rs` (3 `display_path` calls and 4 `relative_display_path` calls).

Not changed, on purpose: five other callers of `relative_forward_slash` (`src/tools/symbol/symbol_at.rs`, `references.rs`, `edit_code.rs`, `src/fs/mod.rs`, `src/symbol/edit.rs`). They report paths too and may have the same defect. Not checked; see Resume.

## Fix provenance

- **SHA:** `a6babe715` (`experiments`)
- **patch-id:** `530240666631e11b6523c2c4a5aeb5f03c2939b4`

Fix committed 2026-11-07, not pushed when this was written. Gate on the tree before two blank lines were added in `util/fs.rs`: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; LEAN 6172 and DEFAULT 9040 passed.

Follow-up commit `665a7758e` (patch-id not computed here): three of the five callers named under Fix now use `relative_display_path`: `symbol_at.rs` (definition `file`), `edit_code.rs` (rename `corruption_hints`) and `references.rs` (the example paths in `completeness_warning`). Red first: `symbol_at_definition_names_the_real_file` and `rename_corruption_hint_names_the_real_file` failed with `src/a/b.rs`. Mutant: both sites restored to `relative_forward_slash` failed both tests. Gate: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, LEAN 6174 and DEFAULT 9042 passed. The `references.rs` site has no test of its own.

## Tests added

`src/tools/backslash_name_tests.rs` (`cfg(all(test, unix))`), 7 tests. Each runs one tool over a directory whose only file is `a\b.*` and requires the result's JSON text to hold the real name and not `a/b.*`:

- `grep` in `files` mode, grouped content mode, context mode, and context mode with two blocks
- `tree` directory listing and glob
- `symbols` directory overview

Red first: all six original tests failed on the old code with `a/b.txt` in the result. Mutant: `display_path` changed back to `replace('\\', "/")` on non-Windows failed all 7. Covered by reading, not by per-site mutation: not every one of the 13 sites is reached. Two probably are not: the `lsp: "warming"` error-path entry near the end of `list_overview.rs`, and the paths in the `WalkClock` cut notice in `grep.rs`.

## Workarounds

Avoid backslashes in file names; or locate such a file with `tree` or `run_command ls`.

## Resume

Fixed in `a6babe715` and `665a7758e`; the two callers left out of those, `classify_reference_path` and `TextualMatch.file`, were fixed in `d05072221` under `bb462b414e559152`. Seen live after `a6babe715` on 2026-11-07 (`grep(mode="files")` and `tree` list `a\b.txt`). `symbols` on a directory, `symbol_at`, `references` and the rename hints are covered by tests only. See the `unverified` caveat in the frontmatter.

## References

- `src/tools/grep.rs` (`to_forward_slash(entry.path())`), `src/util/fs.rs` (`to_forward_slash`)
