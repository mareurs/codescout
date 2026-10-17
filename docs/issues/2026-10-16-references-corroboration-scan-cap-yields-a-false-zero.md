---
kind: bug
status: fixed
title: references' zero-result corroboration stops scanning at 5,000 source files and a stopped scan reads as "no other references"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: medium
closed: 2026-10-17
---

## Summary

When `references` finds no locations outside the definition file, `corroborate_zero_references` scans the project's same-language source files for the identifier as a safeguard against a stale LSP graph (a zero is "the signal an agent uses to delete supposedly-dead code"). The scan stops after `MAX_FILES_SCAN` (5,000) files and returns a bare `Vec<PathBuf>`. An empty vector is produced both by "scanned everything, found nothing" and by "stopped at the cap, never reached the file that uses it", and the caller treats the two identically: it adds no warning. Filed from the `PROBE_ROWS` classification of `references.corroborate_files_scan` as `Coverage::NoMarker`.

## Class fit

The class's own sentence — "a zero from a capped scan reads as 'not present' rather than 'not reached'" — is this bug's whole shape: the safeguard exists to refuse a false zero and itself manufactures one past the cap. It differs from the ceiling members that truncate a returned list: here nothing returned is partial; the *absence* of a warning is the partial answer, and it is a safety warning that goes missing.

## Symptom (Effect)

On a project with more than 5,000 same-language files, `references` on a symbol used only in files the walk reaches after the 5,000th returns the bare zero, with no `completeness_warning`, where a smaller project would have produced one. Inferred from source, not reproduced (needs a >5,000-file fixture).

## Reproduction

`src/tools/symbol/references.rs:162-200`: `scanned` is incremented per same-language file (`:193`) and the loop `break`s at `scanned >= MAX_FILES_SCAN` (`:180`); the function returns `hits`. The only caller (`:467-469`) sets `completeness_warning` only when `!others.is_empty()`. Read at `experiments` f5f1c6f7 — inferred from source, not measured at runtime.

## Root cause

Bound and result share one return value: the `Vec` carries hits and nothing about whether the walk completed. The doc comment says the bounds mirror call_graph's Phase B (`call_graph.workspace_files_scan` has the same defect — see its issue).

## Fix

Verified real at runtime before changing the code (2026-10-17): `references_zero_from_a_corroboration_scan_that_hit_its_cap_is_warned` (5,001 same-language files, none naming the symbol, LSP answering the declaration only) FAILED against the pre-fix code with no `completeness_warning`.

The cap stays (performance guard); the fix is disclosure.

- `src/tools/symbol/scan_budget.rs` (new): `ScanBudget`, shared with the `call_graph` fix. `cut_short_at()` is `Some(cap)` only when a candidate file was actually refused, so a walk ending exactly on the cap, or stopping early on the hit cap, is not flagged. It does not see an admitted file that then fails to read (`docs/issues/2026-10-17-scan-walks-skip-unreadable-files-and-report-a-clean-miss.md`).
- `src/tools/symbol/references.rs`: `corroborate_zero_references` returns `CorroborationScan { hits, cut_short_at }` via `corroborate_with_cap` (cap injectable); `corroboration_cut_short_warning` builds the text; `References::call` sets `completeness_warning` from it when the hit list is empty and the walk was cut short. A finished empty scan stays bare (ADR 2026-08-27 clause 2).
- Review fix, overflow: `format_compact` puts `completeness_warning` FIRST (it was appended). The summary of a buffered result is `truncate_compact(format_compact(val), 2000, 3000)`, which cuts at the last newline in range, so an appended warning is the line a large result loses. This applies to references because "no reference outside the definition file" does not mean a small result (250 uses inside the file).
- Review fix, vendored trees: the walk prunes `path_in_excluded_dir` directories (`target/`, `node_modules/`, `vendor/`, `build/`, `out/`, `dist/`, ...) via `filter_entry`, judged on the path BELOW the project root (the helper tests every component, so the absolute path would exclude a whole project living under a directory named `build`). It matches what the LSP half of `references` already drops. Trade-off to know: a symbol whose ONLY other use is under such a directory (including a legitimate source directory named `build` or `out`) no longer corroborates; the LSP results from there are dropped by the same rule today.
- `src/tools/core/cap_probe.rs`: `references.corroborate_files_scan` moved from `NoMarker` to `Probed`.

Fix commits: `a1c101a3` (first fix) and the review-fix commit recorded under Resume, both on branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5). patch-id of `a1c101a3`: `a28d394e0e8e81323224874c5a2ad735ef090d4c`.

## Tests added

In `src/tools/symbol/tests.rs`:

- `references_zero_from_a_corroboration_scan_that_hit_its_cap_is_warned` (the cited probe test; red before the fix, green after)
- `references_zero_from_a_completed_corroboration_scan_stays_bare`
- `references_warning_survives_the_overflow_summary_of_a_large_result` (through `call_content`: 250 in-file references, buffered, summary truncated). RED observed 2026-10-17 by appending the warning again: `panicked at tests.rs:6087`.
- `corroborate_scan_does_not_spend_its_budget_on_vendored_directories`. RED observed against the unpruned walk: `panicked at tests.rs:6114`, then green with the prune.
- `corroborate_scan_filters_below_the_project_root_not_above_it` (a project under a `build/` parent still gets scanned; guards the root-relative judgement)
- `corroborate_scan_reports_cut_short_only_when_a_candidate_was_left_unexamined` (cap 3 over 3 candidates vs cap 2)
- `corroborate_zero_references_finds_callers_via_text_scan` (existing; adapted to the new return type)

Plus the three `ScanBudget` boundary tests in `src/tools/symbol/scan_budget.rs`.

## Resume

**Review-fix commit:** `824ae416`, patch-id `bd21bcb112d3842380e892c51c053165e026c77c`, same branch.

N/A once fixed. Residual and open decisions, none implemented:

**How often does this warning fire (ADR clause 2 noise)?** Measured 2026-10-17.

- This repo (476 tracked `.rs`, 161 `.py`, 8 `.ts`): never, the walk finishes.
- `/home/marius/work/claude/codex` (5,086 tracked `.rs` files, over the cap, so the walk can never finish): of 45,464 unique `fn` names, every 40th with length >= 4 was sampled (1,136); `git grep -w -l` finds 779 (68.6%) named by exactly ONE file and 357 by two or more. A name found in one file is a symbol `references` reports with 0 references outside the definition file and the text scan cannot corroborate, so in a repo over the cap roughly two thirds of file-local functions return a zero that now carries the warning. Proxy only: it counts text, not LSP results, and trait-impl methods share names.

It is noise in practice in repos over the cap: the warning is true (the walk did stop) but it fires on every file-local symbol and so stops being read, which is what ADR clause 2 warns about. It is inert below the cap.

Options for the human to decide: (1) raise the cap, since the scan is a whole-word text match and not a parse (reading all 5,086 `codex` `.rs` files, 72 MB, took 0.85 s warm, `cat` only, so a 50,000-file cap is a few seconds worst case); (2) keep the cap and scope the warning to symbols that are not obviously file-local; (3) leave as is.

Follow-up, 2026-10-17: a non-UTF-8 candidate file, an unenterable directory and the TypeScript/TSX/JavaScript/JSX family are handled in the same branch (`docs/issues/2026-10-17-scan-walks-skip-unreadable-files-and-report-a-clean-miss.md`, `docs/issues/2026-10-17-call-graph-phase-b-compares-the-lsp-language-id-to-the-ast-language-so-a-tsx-or-jsx-seed-walks-no-files.md`). The warning now covers every way the walk fails to look, through `CorroborationScan::coverage`; `corroboration_cut_short_warning` is `corroboration_incomplete_warning`, and the vendored-directory prune moved into the shared `scan_walk`.

## References

- `src/tools/symbol/references.rs:162-200,457-477`
- `src/tools/core/cap_probe.rs` row `references.corroborate_files_scan`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
