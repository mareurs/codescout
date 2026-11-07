---
kind: bug
status: fixed
title: references and the rename textual matches list a file whose name holds a backslash under a rewritten path
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-07
opened: 2026-11-07
unverified: 'Fixed and gated, but no test reaches the external branch of classify_reference_path or the lib: branch of format_library_path; both use the same display_path swap as the tested project branch. Not seen live in the main references result: after a rebuild on 2026-11-07, rust-analyzer returned no location from src/a\b.rs for a scratch project that declares it with #[path] (3 tries, one after 25 s), so only the mock test covers file_groups. The cause of the missing location was not investigated.'
---

# BUG: references and the rename textual matches list a file whose name holds a backslash under a rewritten path

## Summary

Two more producers of a response path rewrite each `\` in a file name to `/`, as `grep` did before `a6babe715` and `symbol_at`, the rename corruption hint and the references warning did before `665a7758e`. On Linux a backslash is an ordinary file-name character, so a file `src/a\b.rs` is listed as `src/a/b.rs`: a path that does not exist.

- `classify_reference_path` (`src/fs/mod.rs`) builds the `file` of every location in the main `references` result. Its project branch calls `relative_forward_slash`; its external branch calls `to_forward_slash`.
- `TextualMatch.file` (`src/symbol/edit.rs`, built through `relative_forward_slash`) is the `file` of each entry in the `textual_matches` of an `edit_code` rename.

## Class fit

`cluster/addressing-without-an-escape-hatch`, the same class as `2d8475ed5bc831e5`: the listed path is a name the caller feeds back as an address, and the display form turns a legal file-name character into the separator.

## Symptom (Effect)

`references` on a symbol used in `src/a\b.rs` lists `"file": "src/a/b.rs"`. A rename whose textual sweep finds the old name in `src/a\b.rs` lists the same wrong path in `textual_matches`.

## Reproduction

Not reproduced end to end. Established by reading, 2026-11-07:

- `src/tools/symbol/references.rs` lines 437-455: `file` comes from `classify_reference_path(&p, &root, &lib_roots)` and goes into `json!({ "file": file, ... })`.
- `src/fs/mod.rs`, `classify_reference_path`: the project branch is `relative_forward_slash(path, &project_root)` and the external branch is `to_forward_slash(path)`. Both replace `\` with `/` on every platform (`src/util/fs.rs`).
- `src/symbol/edit.rs` line 821: `rel_path = relative_forward_slash(path, project_root)` is stored in `TextualMatch.file`. The `textual_matches` array of a rename response carries that field; the output of `rename_corruption_hint_names_the_real_file` in `src/tools/backslash_name_tests.rs` shows `"textual_matches":[{"file":"src/lib.rs", ...}]`.

A regression test would follow the pattern of `src/tools/backslash_name_tests.rs`: `MockLspClient` with a reference location in `src/a\b.rs`, then `assert_names_the_real_file`.

## Environment

Linux 7.2, branch `experiments`, after `665a7758e`.

## Root cause

Read, not measured: `relative_forward_slash` and `to_forward_slash` replace `\` unconditionally. `display_path` and `relative_display_path` (added in `a6babe715`) keep a `\` on non-Windows hosts and are the fix pattern.

`classify_reference_path` has a second use that must not change: `references.rs` line 423 compares its classification (`project`, `lib:<name>`, `external`) and ignores the display string. Only the display half needs the new form. The `lib:` branch goes through `format_library_path`, which was not read.

## Evidence

The three code sites above, read on 2026-11-07.

## Hypotheses tried

None.

## Fix

Written and committed 2026-11-07 (`d05072221`):

- `classify_reference_path` (`src/fs/mod.rs`): the project branch uses `relative_display_path` and the external branch uses `display_path`. The classification half of the tuple is unchanged, so the comparison at `references.rs` line 423 behaves as before.
- `format_library_path` (`src/fs/mod.rs`): both of its `to_forward_slash` calls became `display_path`. It builds the `lib:` form shown by `references`, `symbols` and the directory overview, a path that has the same defect.
- `text_sweep` (`src/symbol/edit.rs`): `TextualMatch.file` uses `relative_display_path`.

Checked before the change: `TextualMatch.file` reaches only the `textual_matches` JSON and the `uncovered_sources` list of a rename response; no caller compares it with a catalog form.

## Fix provenance

- **SHA:** `d05072221` (`experiments`)
- **patch-id:** `348471ee3bd48ba49ac76abd2d610d9ee258a336`

Fix committed 2026-11-07, not pushed when this was written. Gate: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; LEAN 6176 and DEFAULT 9044 passed.

## Tests added

In `src/tools/backslash_name_tests.rs`, both with `MockLspClient`:

- `references_locations_name_the_real_file`: a reference in `src/a\b.rs` must be listed under that name in `file_groups`.
- `rename_textual_matches_name_the_real_file`: the textual sweep of a rename must list `src/a\b.rs` under that name.

Red first: both failed on the old code with `src/a/b.rs`. Mutant: `relative_forward_slash` restored in the project branch of `classify_reference_path` and in `text_sweep` failed both; reverted.

## Workarounds

Avoid backslashes in file names, or locate the file with `tree` (fixed in `a6babe715`).

## Resume

Fixed in `d05072221`. See the `unverified` caveat in the frontmatter for what no test reaches. Follow-up of `2d8475ed5bc831e5`.

## References

- `2d8475ed5bc831e5`, the first bug of this class, fixed in `a6babe715` and `665a7758e`.
- `src/fs/mod.rs` (`classify_reference_path`), `src/symbol/edit.rs`, `src/tools/symbol/references.rs`, `src/util/fs.rs`.
