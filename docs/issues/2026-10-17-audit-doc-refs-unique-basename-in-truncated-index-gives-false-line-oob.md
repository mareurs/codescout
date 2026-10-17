---
kind: bug
status: open
title: audit_doc_refs file_line/file_symbol treat a single indexed copy as THE file when the basename index is truncated, so a twin in the un-indexed tail gives a false LineOob/SymbolMissing
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
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

Code: `src/librarian/tools/audit_doc_refs/resolver.rs`, `unique_basename_path` (`[only] => Some(ctx.repo_root.join(only))`), called from `resolve_file_line` and `resolve_file_symbol` before `try_basename_fallback`. `file_symbol` shares the call and the match arm; only `file_line` was run.

## Root cause

`unique_basename_path` has no access to the truncation fact (or does not consult `ctx.basename_index_truncated`), so a one-element index entry is trusted as unique on an index that is only a prefix.

## Fix

Not designed. Candidate: in `unique_basename_path`, return `None` when `ctx.basename_index_truncated` (so the caller falls through to `try_basename_fallback`), and have `try_basename_fallback` report a single indexed copy on a truncated index as `ResolvedBasename` with a note that uniqueness is unproven, or as `Unknown`/`BasenameIndexTruncated`. Decide which, with a test per ref kind.

## Tests added

None — filing only (a throwaway probe test was run and removed).

## Resume

1. Decide the verdict for a single indexed copy on a truncated index (resolved-with-caveat vs unknown) for `file_path` as well: `resolve_file_path` returns `ResolvedBasename` with the note "resolved by basename to <path>" and says nothing of uniqueness, which is the same unproven claim at `low`.
2. Fix `unique_basename_path`, then add the cap-driven e2e test through `call_with_index_cap`.

## References

- `src/librarian/tools/audit_doc_refs/resolver.rs` (`unique_basename_path`, `resolve_file_line`, `resolve_file_symbol`, `try_basename_fallback`)
- `docs/issues/2026-10-16-audit-doc-refs-basename-index-cap-reports-existing-files-missing.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
