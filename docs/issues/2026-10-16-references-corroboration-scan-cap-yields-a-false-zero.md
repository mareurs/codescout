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

- `src/tools/symbol/scan_budget.rs` (new): `ScanBudget`, shared with the `call_graph` fix. `cut_short_at()` is `Some(cap)` only when a candidate file was actually left unexamined, so a walk ending exactly on the cap, or stopping early on the hit cap, is not flagged.
- `src/tools/symbol/references.rs`: `corroborate_zero_references` now returns `CorroborationScan { hits, cut_short_at }` via `corroborate_with_cap` (cap injectable); `corroboration_cut_short_warning` builds the text, and `References::call` sets `completeness_warning` from it when the hit list is empty and the walk was cut short. A finished empty scan stays bare (ADR 2026-08-27 clause 2).
- `src/tools/core/cap_probe.rs`: `references.corroborate_files_scan` moved from `NoMarker` to `Probed`.

Fix commit: SHA `a1c101a3` on branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5); patch-id `a28d394e0e8e81323224874c5a2ad735ef090d4c`.

## Tests added

In `src/tools/symbol/tests.rs`:

- `references_zero_from_a_corroboration_scan_that_hit_its_cap_is_warned` (the cited probe test; red before the fix, green after)
- `references_zero_from_a_completed_corroboration_scan_stays_bare`
- `corroborate_scan_reports_cut_short_only_when_a_candidate_was_left_unexamined` (cap 3 over 3 candidates vs cap 2)
- `corroborate_zero_references_finds_callers_via_text_scan` (existing; adapted to the new return type)

Plus the three `ScanBudget` boundary tests in `src/tools/symbol/scan_budget.rs`.

## Resume

N/A once fixed.

## References

- `src/tools/symbol/references.rs:162-200,457-477`
- `src/tools/core/cap_probe.rs` row `references.corroborate_files_scan`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
