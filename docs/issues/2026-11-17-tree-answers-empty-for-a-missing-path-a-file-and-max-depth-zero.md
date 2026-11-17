---
id: '8c9a21b468d38b9f'
kind: bug
status: open
title: 'BUG: tree answers "(empty directory)" / "0 files" for a missing path, a file path and max_depth=0'
tags:
- cluster/unclassified
opened: 2026-11-17
---

# BUG: tree answers "(empty directory)" / "0 files" for a missing path, a file path and max_depth=0

## Summary
`tree` validated the `path` argument for security but never checked that it names an existing directory. The directory walker swallows the error of a root it cannot open and yields nothing for a file, so a mistyped path, a file path and `max_depth=0` all came back as an empty listing — in list mode `(empty directory)`, in glob mode `0 files`. Every caller that mistypes a directory is told it is empty.

## Class fit
Closest by name is the accepted-parameter-silently-dropped class (IC-15), which covers only the `max_depth=0` half. The main defect is a failure rendered as an answer: the walker's error for a missing root is discarded and the empty result is presented as a valid listing. The roster's `cluster/unclassified` entry already names this shape as a candidate class ("a failure rendered as an answer"), so this is filed there rather than forced into a class.

## Symptom (Effect)
```
tree(path="src/this/does/not/exist")        -> (empty directory)
tree(path="src/main.rs")                    -> (empty directory)
tree(path="src", max_depth=0)               -> (empty directory)
tree(path="src", limit=0)                   -> (empty directory)
tree(path="src/this/does/not/exist", glob="*.rs") -> 0 files
tree(path="src/main.rs", glob="*.rs")       -> 0 files
```
Control: `tree(path="src", depth=1)` lists 38 entries. `symbols` and `read_file` refuse the same missing path with `path not found: … Nearest existing directory: …`.

## Reproduction
Build `afac1bb72`, any project. Run the calls above. Measured 2026-11-17, 6/6 (the `limit=0` row is NOT fixed by this change, see Fix).

## Environment
Linux, release build `afac1bb72`, MCP stdio, branch `experiments-albert`.

## Root cause
`list_dir_impl` and `glob_impl` in `src/tools/tree.rs` call `validate_read_path` (security only) and hand the result to `ignore::WalkBuilder` with `.build().flatten()`. A missing root makes the walker's first item an error, which `.flatten()` drops; a file root yields only the depth-0 entry, which list mode filters with `.filter(|e| e.depth() > 0)` and glob mode skips as a non-file. With `max_depth=0` the walker yields only the depth-0 entry. Read from the code and measured by the calls above on 2026-11-17.

The `max_depth` schema declares `minimum: 1`; the server did not enforce its own schema.

An existing test, `tools::edit_file::tests::tree_nonexistent_path_errors`, was named for the intended contract but asserted the opposite (`entries.is_empty()`, comment "WalkBuilder returns empty for nonexistent paths"), so the gate was green for the defect.

## Evidence
Calls in Symptom. Failing first run of the new tests (before the fix), each panic message: `a missing path must not list as empty: … Object {"entries": Array []}`, same for the file path and for `max_depth=0`.

## Hypotheses tried
1. **Hypothesis:** `validate_read_path` rejects missing paths. **Test:** `tree` on a missing path returned `(empty directory)`. **Verdict:** rejected — it checks policy, not existence.
2. **Hypothesis:** only list mode is affected. **Test:** the same missing and file paths with `glob` set returned `0 files`. **Verdict:** rejected — both branches.

## Fix
`require_directory` (src/tools/tree.rs:296) is called right after `validate_read_path` in both branches. A missing path is a `RecoverableError` `path not found: <path>` whose hint names the parent directory and, through `describe_neighbours_of_missing`, the nearest existing directory; a file path is `not a directory: <path>` with a hint to `read_file`/`symbols`. `max_depth=0` is a `RecoverableError` in list mode (the schema minimum). `validate_read_path` and the semantics of `"."` are unchanged.

Not fixed here: `limit=0`, which is shared output-guard handling across tools (`symbols`, `semantic_search`) and needs its own change.

Fix commit: not committed yet.

## Tests added
`src/tools/tree.rs`: `a_missing_path_is_refused_naming_the_nearest_existing_directory` (:1085), `a_file_path_is_refused_as_not_a_directory` (:1112), `max_depth_zero_is_refused_like_the_schema_minimum` (:1139), and a guard that must keep passing, `an_existing_empty_directory_still_lists_as_empty` (:1158). Each of the first three fails before the change and passes after (both modes in the first two). `src/tools/edit_file/tests.rs`: `tree_nonexistent_path_errors` rewritten to assert the refusal it is named for.

## Workarounds
Check the path with `symbols(path)` or `read_file` before trusting an empty `tree` answer.

## Resume
Commit when asked; then write `## Fix provenance` with the SHA and `git patch-id --stable` of the commit, set `status: fixed` and `closed:`, and move to `docs/issues/archive/` with `doc(action="move")`. Open a separate bug for `limit=0`.

## References
`src/tools/tree.rs`, `src/fs/mod.rs` (`describe_neighbours_of_missing`), field finding A-005 (experiments-albert feedback).
