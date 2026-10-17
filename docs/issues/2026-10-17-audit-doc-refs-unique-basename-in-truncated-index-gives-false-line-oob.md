---
kind: bug
status: fixed
title: audit_doc_refs file_line/file_symbol treat a single indexed copy as THE file when the basename index is truncated, so a twin in the un-indexed tail gives a false LineOob/SymbolMissing
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
closed: 2026-10-17
---

## Summary

`resolver::unique_basename_path` answers "which file?" for a bare-basename `path:line` or `path::symbol` citation by taking the index entry when it has exactly one path. When the basename index was truncated at its file cap (`ScanMeta.basename_index_truncated`, fixed in `2026-10-16-audit-doc-refs-basename-index-cap-reports-existing-files-missing.md`), "exactly one indexed copy" does not mean "exactly one copy": a twin can sit in the un-indexed tail. The audit then range-checks the line (or looks up the symbol) in the wrong file and reports `line_oob` / `symbol_missing` at `med` — a false claim about a file the doc may cite correctly. The sibling fix only covered the case where the basename is absent from the index.

## Class fit

A capped scan read as complete: the truncated index is a prefix of the tree, and a lookup against a prefix is answered as if it were the whole tree, here asserting a unique match instead of an absence. It differs from the sibling bug in what is falsely claimed: not "this file does not exist" (`missing`) but "this file is the only one, and your line/symbol is not in it" (`line_oob` / `symbol_missing`), and the finding is `med`, so it gates under `--fail-on med` unlike the sibling's `low`/`unknown` result.

## Symptom (Effect)

With a truncated index, a doc citing `x.py:50` where `a/x.py` (3 lines) is indexed and `b/x.py` (100 lines) is in the tail gets `line_oob` / `med`; against a complete index the same ref is `ambiguous_basename` / `med`.

## Reproduction

Observed 2026-10-17 on the branch that fixed the sibling bug, with a throwaway test (not kept) driving `call_with_index_cap(&ctx, args, 1)` over a repo of `a/x.py` (3 lines), `b/x.py` (100 lines), `docs/spec.md` containing `` `x.py:50` ``. The walk is name-sorted, so `a/x.py` is the one indexed file. Output of the first finding:

- cap 1: `"ref_kind":"file_line","verdict":"line_oob","severity":"med","severity_reason":"policy_default"`
- uncapped: `"verdict":"ambiguous_basename","severity":"med","severity_reason":"basename_ambiguous","notes":"basename matches 2 files: a/x.py, b/x.py"`

Re-verified at `afbe8abf` before fixing, by the new tests going red: `file_line` gave `LineOob`/`Med` (both resolver-level and end to end), `file_symbol` (mock LSP, empty symbols) gave `SymbolMissing`/`Med`, and `file_path` gave `ResolvedBasename` with the note `resolved by basename to pkg/bar.py` and no hint that the index was truncated.

Code: `src/librarian/tools/audit_doc_refs/resolver.rs`, `unique_basename_path` (`[only] => Some(ctx.repo_root.join(only))`), called from `resolve_file_line` and `resolve_file_symbol` before `try_basename_fallback`. `file_symbol` shares the call and the match arm; only `file_line` was run.

## Root cause

`unique_basename_path` has no access to the truncation fact (or does not consult `ctx.basename_index_truncated`), so a one-element index entry is trusted as unique on an index that is only a prefix.

## Fix

Fixed 2026-10-17. On a truncated index a single indexed copy is no longer treated as THE file:

- `file_line` / `file_symbol`: when the file came from a lone entry of a truncated index (`indexed_copy_only`), a NEGATIVE on it (`line_oob` / `symbol_missing`) becomes `Unknown`, `Low`, `SeverityReason::BasenameIndexTruncated`, with notes naming the copy examined, the truncation and the env override (`resolver::indexed_copy_unproven`). A positive on the copy stays `Resolved` (it can only be a false "ok", never a false "broken"). `Low` is never above the `Med` the old verdict carried.
- `file_path`: the verdict stays `ResolvedBasename` (the file exists, and the verdict is exempt from every gate), but on a truncated index the note says the copy is the only one *found*, not proven the only one. Making it `Unknown` would put every basename citation of a >50,000-file repo under `--fail-on low`; this is a judgement call, flagged in the commit report.
- A complete index is unchanged byte for byte (asserted); refs containing `/` never reach this path (asserted for `file_line`).

## Tests added

In `src/librarian/tools/audit_doc_refs/resolver.rs`:

- `resolver_line_miss_in_single_indexed_copy_of_truncated_index_is_unknown` (negative -> unknown, positive stays resolved, complete index keeps `line_oob`/med)
- `resolver_symbol_miss_in_single_indexed_copy_of_truncated_index_is_unknown` (mock LSP; complete index keeps `symbol_missing`)
- `resolver_single_copy_basename_hit_on_truncated_index_says_uniqueness_is_unproven` (complete-index note unchanged)
- `resolver_slashed_line_ref_is_unaffected_by_a_truncated_index`

In `src/librarian/tools/audit_doc_refs/mod.rs`:

- `single_indexed_copy_on_truncated_index_does_not_give_a_false_line_oob` (end to end through `call_with_index_cap`, with a complete-index control that yields `ambiguous_basename`)

Red first at HEAD before the fix: the line, symbol, note and end-to-end tests failed (`left: LineOob right: Unknown`, `left: SymbolMissing right: Unknown`, note `resolved by basename to pkg/bar.py` without the truncation caveat); the slashed-ref test passed as a baseline. Then each guarded site was mutated once and observed red: `file_line` flag forced false (line and end-to-end tests red), `file_symbol` flag forced false (symbol test red), `file_path` note branch disabled (note test red).

## Resume

None outstanding.

## References

- `src/librarian/tools/audit_doc_refs/resolver.rs` (`unique_basename_path`, `resolve_file_line`, `resolve_file_symbol`, `try_basename_fallback`)
- `docs/issues/2026-10-16-audit-doc-refs-basename-index-cap-reports-existing-files-missing.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
