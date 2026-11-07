---
kind: bug
status: open
title: references and the rename textual matches list a file whose name holds a backslash under a rewritten path
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-11-07
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

Not started. Direction: use `relative_display_path` in the project branch and `display_path` in the external branch of `classify_reference_path`; check `format_library_path` for the same rewrite; use `relative_display_path` in `src/symbol/edit.rs`. Check first that no caller of `classify_reference_path` or `TextualMatch.file` compares the string with a catalog form.

## Tests added

N/A: bug only filed.

## Workarounds

Avoid backslashes in file names, or locate the file with `tree` (fixed in `a6babe715`).

## Resume

Open. Next step: write the `references` test with `MockLspClient` and watch it fail with `src/a/b.rs`, then apply the fix above.

## References

- `2d8475ed5bc831e5`, the first bug of this class, fixed in `a6babe715` and `665a7758e`.
- `src/fs/mod.rs` (`classify_reference_path`), `src/symbol/edit.rs`, `src/tools/symbol/references.rs`, `src/util/fs.rs`.
