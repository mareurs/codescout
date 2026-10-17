---
kind: bug
status: fixed
title: audit_doc_refs stops its basename index at 50,000 files and reports the un-indexed tail's refs as Missing
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
---

## Summary

`audit_doc_refs` resolves a bare-basename citation through an index built by `build_basename_index`. The build stops after `MAX_INDEXED_FILES` (50,000) files. A ref to a file in the un-indexed tail then gets `Verdict::Missing` — a claim that the file does not exist — and nothing in the audit output says the index was incomplete. The function's own doc comment states this as intended behaviour ("basename misses just fall through to `Verdict::Missing` for the un-indexed tail"). Filed from the `PROBE_ROWS` classification of `audit_doc_refs.basename_index` as `Coverage::NoMarker`.

## Class fit

A capped scan read as "not present" rather than "not reached": a truncated index produces a false `Missing`, a statement of absence about a file that exists. It differs from the walk-based members in its blast radius: the cap is high (50,000 files, an order above the 5,000 of the references/call_graph scans), so only very large monorepos reach it, and the result is a lint finding rather than a deletion signal.

## Symptom (Effect)

On a repo with more than 50,000 non-ignored files, a doc citing a file beyond the cap by basename only is reported as missing (counted in `n_refs_broken`).

Reproduced 2026-10-17 at the test level by making the cap injectable: three existing files cited by bare basename, cap of 1, and at least two came back `missing`. No 50,000-file fixture was built.

## Reproduction

Verified 2026-10-17 on `experiments` 299395e5, which has moved on from the lines first cited: `build_basename_index` still `break`s at `count >= MAX_INDEXED_FILES` and returned a bare `HashMap`; `resolver::try_basename_fallback` returned `None` on an index miss, and its callers (`resolve_file_path`, `resolve_file_line`, `resolve_link`, `resolve_file_symbol`) turned that into `Missing` / `FileMissing`. Note the index is consulted by four ref kinds, not only `file_path`.

## Root cause

The index type was a bare `HashMap`; there was no slot for "built from a truncated walk", so "not in the index" and "does not exist" were the same value. The doc comment on `build_basename_index` described the false `Missing` as graceful degradation, which is the defect restated as a design.

## Fix

- `build_basename_index(repo_root, max_files)` returns `(index, truncated)`. `truncated` is true only when the walk reached a *file* it did not index; a walk that ends exactly at the cap, or whose trailing entries are directories, is complete.
- `ResolveCtx.basename_index_truncated` carries it. The verdict lives in ONE place, `try_basename_fallback`, so all four ref kinds inherit it: a bare basename (no `/`, after the `./` strip) that misses a truncated index becomes `Verdict::Unknown`, `Severity::Low`, new `SeverityReason::BasenameIndexTruncated` (`basename_index_truncated`), with notes naming the scope examined and the env override. A ref with a `/` never reaches the index and keeps `Missing`; a hit in the indexed portion still resolves; a complete index keeps `Missing` (silence on a trustworthy negative, ADR 2026-08-27).
- The cap is `MAX_INDEXED_FILES_DEFAULT` (50,000, annotated `RESULT_CAP audit_doc_refs.basename_index`), overridable with `LIBRARIAN_AUDIT_MAX_INDEXED_FILES`, read only in `call`. `call_with_index_cap` is the seam a test drives, so no `set_var` (UB in a parallel test binary).
- `scan_meta.basename_index_truncated: bool` is the marker in the response, carried on `Degradation` but deliberately not folded into `is_degraded()` (that flag reports the LSP).
- The persisted tracker records it too: `ScanMeta.basename_index_truncated` (`#[serde(default)]`), set in `upsert_tracker` and rendered in the tracker header by `render_template.j2`. This matters because the merger never opens an issue from an `unknown` verdict, so on a truncated scan a newly broken bare-basename ref is absent from the tracker and the header flag is the only signal.
- A LINK ref was the only `missing`/`high` kind on this path (path/line/symbol refs were already capped to `low` as inferred paths), so a truncated-index link miss is `Unknown` at `med` (never above what the `Missing` would have carried after archive/memory drops), keeping it visible under `--fail-on med` per the band convention in `severity::default_severity`. This is a trade-off: `--fail-on high` no longer fails on it.
- The walk is name-sorted (`sort_by_file_name`), so the indexed prefix of a capped walk is identical on every machine.
- `cap_probe.rs` row `audit_doc_refs.basename_index` moved `NoMarker` -> `Probed` (marker `$.scan_meta.basename_index_truncated`, mutation `Killed`).

## Tests added

In `src/librarian/tools/audit_doc_refs/mod.rs`:

- `existing_file_beyond_index_cap_is_not_reported_missing` (the cited test for the probe row; end to end, with an uncapped control)
- `genuinely_absent_ref_stays_missing_when_index_is_complete`
- `basename_index_flags_truncation_only_when_a_file_was_left_out`
- `basename_index_trailing_directories_do_not_count_as_truncation`

In `src/librarian/tools/audit_doc_refs/resolver.rs`:

- `resolver_basename_miss_on_truncated_index_is_unknown_not_missing` (FilePath, Link, FileLine, FileSymbol)
- `resolver_slash_ref_stays_missing_on_truncated_index`
- `resolver_basename_hit_on_truncated_index_still_resolves`

Added in the review round: `truncated_link_ref_is_unknown_med_and_gates_at_med_but_not_high`, `genuinely_absent_link_stays_missing_high_when_index_is_complete`, `truncation_reaches_the_persisted_tracker_and_its_rendering`, `resolver_truncated_link_severity_never_exceeds_the_missing_it_replaces`.

Mutation-verified 2026-10-17: with the truncated arm of `try_basename_fallback` disabled, the resolver test and the end-to-end test red; with the marker key dropped from `build_response`, the end-to-end test reds.

## Resume

Fixed in the same merge: `docs/issues/2026-10-17-audit-doc-refs-unique-basename-in-truncated-index-gives-false-line-oob.md`. A basename with ONE indexed copy and a twin in the un-indexed tail was taken as unique, so `file_line`/`file_symbol` could report a false `line_oob`/`symbol_missing` (`med`) — a false claim, not only a uniqueness claim. That bug now yields `Unknown` instead.

Checked and NOT a bug: `build_basename_index` skips walk errors (`flatten`) without marking truncation, but `collect_markdown_files` walks the same tree with the same defaults and `?`-propagates walk errors first. A mode-000 subdirectory aborts the whole audit (`IO error ... Permission denied`) before the index is built, so no silent un-indexed tail results. The coupling is documented on `build_basename_index`.

## References

- `src/librarian/tools/audit_doc_refs/mod.rs` (`build_basename_index`, `call_with_index_cap`, `build_response`)
- `src/librarian/tools/audit_doc_refs/resolver.rs` (`try_basename_fallback`)
- `src/tools/core/cap_probe.rs` row `audit_doc_refs.basename_index`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
