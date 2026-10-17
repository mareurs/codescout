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

- `src/tools/symbol/scan_budget.rs` (new): `ScanBudget::admit` is where a walk is refused one more candidate file, and it records that. `cut_short_at()` is `Some(cap)` only when a candidate was actually refused, so a walk that ends exactly on its cap is not flagged (ADR 2026-08-27 clause 2). Shared with the `references` fix. It does NOT see an admitted file that then fails to read or parse (see `docs/issues/2026-10-17-scan-walks-skip-unreadable-files-and-report-a-clean-miss.md`); its docs say so.
- `src/tools/symbol/call_graph/mod.rs`: the walk returns `WorkspaceScan::{Found, NotFound, CutShort}`; `lookup_pos` records a cut-short symbol in `scan_cut_short`; `CachedResolver::cut_short()` returns those symbols with the cap the walk was GIVEN (`workspace_scan_cap`); `CallGraph::call` calls `attach_scan_cut_short`, which adds `completeness_warning`, `scan_cut_short_symbols` (bounded to 20) and `scan_cut_short_total`. `MAX_WORKSPACE_FILES_SCAN` moved to module level.
- Review fix (same file): `format_compact` puts the warning FIRST. On the overflow path the caller reads `truncate_compact(format_compact(val), 2000, 3000)` (`src/tools/core/types.rs`), which cuts at the last newline in range, so the line appended last was exactly the line a large result lost; the workspace notice is prefixed for the same reason. The symbol list is bounded because it counts toward the size that decides buffering.
- `src/tools/core/cap_probe.rs`: `call_graph.workspace_files_scan` moved from `NoMarker` to `Probed`, citing the end-to-end test below with the production marker `stopped after 5000`.

Fix commits: `a1c101a3` (first fix) and the review-fix commit recorded under Resume, both on branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5). patch-id of `a1c101a3`: `a28d394e0e8e81323224874c5a2ad735ef090d4c`.

## Tests added

In `src/tools/symbol/call_graph/mod.rs` (`mod tests`):

- `call_graph_reports_a_caller_it_could_not_place_because_the_walk_hit_its_cap` (the cited probe test, end to end through `CallGraph::call` over 5,001 files; asserts `completeness_warning`, `scan_cut_short_symbols`, `scan_cut_short_total` and the compact text). RED observed 2026-10-17 by deleting the `attach_scan_cut_short` call: `panicked at mod.rs:1228`.
- `call_graph_stays_bare_when_the_walk_for_an_unplaced_caller_finished` (ADR clause 2, end to end)
- `call_graph_warning_survives_the_overflow_summary_of_a_large_result` (through `call_content`: 121 edges, buffered, summary truncated, warning still in what the caller reads). RED observed by moving the warning back to the end of `format_compact`: `panicked at mod.rs:1287` (and, independently, by deleting the emission).
- `a_workspace_walk_stopped_by_its_cap_is_reported_not_passed_off_as_no_such_symbol` (resolver level; asserts the cap the walk was given). RED observed by making `cut_short()` return the production constant: `panicked at mod.rs:1067`.
- `a_workspace_walk_that_ends_exactly_on_its_cap_is_a_real_miss`, `a_definition_found_within_the_cap_is_not_flagged`, `format_compact_carries_the_completeness_warning`

In `src/tools/symbol/scan_budget.rs`: three boundary tests.

## Resume

**Review-fix commit:** `824ae416`, patch-id `bd21bcb112d3842380e892c51c053165e026c77c`, same branch.

N/A once fixed. Residual and open decisions, none implemented:

**How often does this warning fire (ADR clause 2 noise)?** Measured 2026-10-17.

- This repo has 476 tracked `.rs`, 161 `.py`, 8 `.ts` files, all far below 5,000: the walk always finishes here and the warning never fires.
- `/home/marius/work/claude/codex` has 5,086 tracked `.rs` files, so a Rust walk there can never finish. `hermes-agent` (4,586 `.py`) is just under the cap.
- call_graph path: NOT measured at runtime. It needs rust-analyzer on a >5,000-file tree. Structurally, with `direction=callees` and `max_depth >= 2`, every interior node that `workspace/symbol` does not place by exact name goes to the Phase B walk, and in such a tree that walk ends `CutShort`. `resolve_via_lsp` (`src/tools/symbol/call_edges/resolver.rs`) keeps std/dependency callees, which no file under the root defines. Expect the warning on most deep callee queries in a repo over the cap; unmeasured.

Options for the human to decide: (1) skip Phase B for callees whose `to.uri` is outside the project root (no walk can find them); (2) raise the cap, since the references-side scan is a whole-word text match, not a parse (reading all 5,086 `codex` `.rs` files, 72 MB, took 0.85 s warm, `cat` only); (3) leave as is and accept the warning in huge repos, where it is true (the walk did stop) but uninformative.

Not fixed here: the Phase B walk does not apply the vendored-directory prune that the `references` walk now has, so a non-gitignored `node_modules/` spends this walk's budget too. Filed separately: `docs/issues/2026-10-17-call-graph-phase-b-compares-the-lsp-language-id-to-the-ast-language-so-a-tsx-or-jsx-seed-walks-no-files.md` (a `.tsx` or `.jsx` seed examines no file at all) and `docs/issues/2026-10-17-scan-walks-skip-unreadable-files-and-report-a-clean-miss.md`.

## References

- `src/tools/symbol/call_graph/mod.rs:86-102,148-198`
- `src/tools/core/cap_probe.rs` row `call_graph.workspace_files_scan`
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
