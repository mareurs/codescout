---
kind: bug
status: fixed
title: call_graph's workspace definition scan stops at 5,000 files and a stopped scan reads as "no such symbol"
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
---

## Summary

`CachedResolver::lookup_pos_via_ts_workspace` is call_graph's Phase B fallback: when the LSP cannot place a symbol, it walks the workspace parsing same-language files for a definition. It stops after `MAX_WORKSPACE_FILES_SCAN` (5,000) parsed files and returns `None`. `None` means both "no file defines it" and "the walk stopped before reaching the file that does", and the caller records either as `not_found` for the rest of the resolver's lifetime. Filed from the `PROBE_ROWS` classification of `call_graph.workspace_files_scan` as `Coverage::NoMarker`.

## Class fit

A zero from a capped scan read as "not present" rather than "not reached" — the class's own wording. It differs from `references.corroborate_files_scan` (`docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`), which loses a warning; this one loses a *position*, so a symbol that exists is treated as unresolvable and its edges are dropped from the graph.

## Symptom (Effect)

On a project with more than 5,000 same-language files, with the LSP unable to place the symbol, a definition reached after the 5,000th parsed file is not found and `call_graph` proceeds as though the symbol were unresolvable, with no note that the scan was cut short. Inferred from source — not reproduced (needs a >5,000-file fixture with the LSP down); how the dropped node is rendered to the caller was not traced.

## Reproduction

`src/tools/symbol/call_graph/mod.rs:158-198`: `scanned` counts parsed files (`:184`), the loop breaks at `scanned >= MAX_WORKSPACE_FILES_SCAN` (`:172`), the function falls through to `None`. The caller (`:94-102`) inserts the symbol into `not_found` on `None`. Read at `experiments` f5f1c6f7 — inferred, not measured at runtime.

## Root cause

An `Option` return that carries "found" and nothing about whether the search was exhaustive; the cap is documented in a comment ("a monorepo stops short instead of stalling") and nowhere in the data.

## Fix

Verified real at the code before changing it (2026-10-17): `lookup_pos_via_ts_workspace` returned a bare `Option`, and `lookup_pos` cached `None` into `not_found` whether the walk finished or stopped at the cap. One correction to the filing: the *seed* symbol is pre-seeded into `positions` and never reaches Phase B, so the effect is a non-seed node (a caller or callee discovered mid-BFS) silently losing its edges, not a false "no callers" for the symbol asked about.

The cap stays (it is a performance guard against the MCP ceiling); the fix is disclosure.

- `src/tools/symbol/scan_budget.rs` (new): `ScanBudget::admit` is the one place a candidate file is refused, and it records that. `cut_short_at()` is `Some(cap)` only when a candidate was actually left unexamined, so a walk that ends exactly on its cap is not flagged (ADR 2026-08-27 clause 2). Shared with the `references` fix.
- `src/tools/symbol/call_graph/mod.rs`: the walk returns `WorkspaceScan::{Found, NotFound, CutShort}`; `lookup_pos` records a cut-short symbol in a new `scan_cut_short` set; `CallGraph::call` adds `completeness_warning` and `scan_cut_short_symbols` to the result, and `format_compact` renders the warning (the tool is `OutputForm::Text`, so that is what the caller reads). `MAX_WORKSPACE_FILES_SCAN` moved to module level and the resolver carries it as `workspace_scan_cap` so the boundary is testable.
- `src/tools/core/cap_probe.rs`: `call_graph.workspace_files_scan` moved from `NoMarker` to `Probed`.

Fix commit: SHA `a1c101a3` on branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5); patch-id `a28d394e0e8e81323224874c5a2ad735ef090d4c`.

## Tests added

In `src/tools/symbol/call_graph/mod.rs` (`mod tests`):

- `a_workspace_walk_stopped_by_its_cap_is_reported_not_passed_off_as_no_such_symbol` (the cited probe test; mutation-killed by removing the `scan_cut_short` insert)
- `a_workspace_walk_that_ends_exactly_on_its_cap_is_a_real_miss`
- `a_definition_found_within_the_cap_is_not_flagged`
- `format_compact_carries_the_completeness_warning`

In `src/tools/symbol/scan_budget.rs`: three unit tests on the boundary (`a_walk_that_ends_exactly_on_its_cap_is_not_cut_short`, `the_first_refused_candidate_marks_the_walk_cut_short`, `a_zero_budget_refuses_the_first_candidate`).

Not tested end to end: `CallGraph::call`'s assembly of `completeness_warning` from the per-direction resolvers (needs an LSP mock with call hierarchy and a >5,000-file or injectable-cap fixture through the tool surface); the pieces it composes are each covered.

## Resume

N/A once fixed. Residual: `call_graph`'s tool-level path has no test that drives a cut-short walk through `CallGraph::call` itself.

## References

- `src/tools/symbol/call_graph/mod.rs:86-102,148-198`
- `src/tools/core/cap_probe.rs` row `call_graph.workspace_files_scan`
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
