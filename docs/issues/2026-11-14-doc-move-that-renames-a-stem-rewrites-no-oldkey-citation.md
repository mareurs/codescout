---
id: '67d5983be664de6a'
kind: bug
status: open
title: 'BUG: doc(move) that renames a file stem rewrites none of the [[oldkey]] citations in other documents'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not run. Not established: what the move response lists for a real [[oldkey]] citation (no call made), what link_scan or the entry_cite rows do after the rename, and whether Plan D has already ruled on a rewrite. The Plan D scope was not read.'
---

# BUG: doc(move) that renames a file stem rewrites none of the [[oldkey]] citations in other documents

## Summary
A key is the lowercase file stem (`key_of`, `src/librarian/identity/key.rs`). A `doc(move)` that changes the stem therefore changes the document's key. `move` rewrites no citation: other documents that cite `[[oldkey]]` keep the old text. The caller is told to re-point them by hand. Plan D needs a rewrite, or a decision not to need one, before it relies on keys.

## Class fit
cluster/unclassified: a rename that changes the name a document is cited by, with no step that carries the citations along. The move reports the work and leaves it to the caller.

## Symptom (Effect)
After `doc(move, new_rel_path=".../renamed.md")` for `.../original.md`, a document that holds `[[original]]` still holds it. The key `original` no longer names a file.

## Reproduction
Not run. Read path, `src/librarian/tools/mv.rs`, `call`:
- No code in `call` edits any file other than the moved file's own `id:` line (`repair_frontmatter_id`). A grep of `mv.rs` for rewrite, retarget and `[[` finds only comments and test fixtures.
- After the move, `citations_with_dated_flag` runs `git grep --untracked -l -F <old stem>` (and the dateless slug when the stem is dated) and returns the citing files as `inbound_path_citations`.
- `stem_preserved` is false when the stem changes, so `inbound_citations_cleared` is empty and `citation_stem_preserved` is false. The test `a_move_that_renames_the_stem_clears_nothing_and_says_so` pins that.
- For a key equal to the stem, `[[oldkey]]` contains the stem, so the scan should list its file. That is a substring scan, so it lists files and not the lines or the form.

## Environment
experiments at fe5370c2f.

## Root cause
`move` was designed before keys. It reports inbound citations (paths, slugs and ids) and leaves the edit to the caller, which is the workflow in `get_guide("tracker-conventions")` § Archive. Keys add a citation form whose target name moves with a rename.

## Evidence
- Plan C1 execution state, "Next steps": `doc(move)` stem rename leaves `[[oldkey]]` cites stale; a Plan D precondition.
- The move response reports the citing files, so the work is visible. It is not done.

## Hypotheses tried
None. Found by the Plan C1 whole-branch review (N-5).

## Fix
Not started. Options for Plan D: rewrite `[[oldkey]]` and `[[oldkey:TOKEN]]` citations in the files the scan finds when the key changes, report them separately from path and slug citations, or refuse a move that changes the key while citations exist. Whatever is chosen, the response should name the key change.

## Tests added
None.

## Workarounds
Keep the stem when moving. After a stem rename, grep for `[[oldkey` and edit each file, in the same commit as the move.

## Resume
Open. Not started. A precondition of Plan D.

## References
- `src/librarian/tools/mv.rs` (`call`, `citations_with_dated_flag`, `stem_preserved`)
- `src/librarian/identity/key.rs` (`key_of`)
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
