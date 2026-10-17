---
kind: bug
status: fixed
title: references' corroboration scan and call_graph's Phase B walk skip a file they cannot read and still report a clean miss
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-17
severity: low
closed: 2026-10-17
---

## Summary

Both fallback walks (`corroborate_with_cap` in `src/tools/symbol/references.rs` and `CachedResolver::lookup_pos_via_ts_workspace` in `src/tools/symbol/call_graph/mod.rs`) admit a file against the file budget and only then try to read and parse it. A read failure (not valid UTF-8, permissions), a parse failure, or a directory the walker cannot enter drops that file with nothing recorded. The walk then ends as `NotFound` / an empty hit list, which the callers treat as a trustworthy negative. Found while reviewing the fix for the two scan-cap bugs, which closed the *cap* path to this false negative and left this one.

## Class fit

The class's own shape: a search that did not cover its population reports "not present" rather than "not reached". The difference from the scan-cap members is that the cap is not involved at all, so `ScanBudget::cut_short_at` cannot see it: the files are admitted, then skipped one by one on an `else { continue }`.

## Symptom (Effect)

A definition (call_graph) or a use (references) that lives only in a file `std::fs::read_to_string` rejects is never seen. call_graph treats the symbol as unplaceable and drops its edges with no warning; references returns a bare zero with no `completeness_warning`.

Measured 2026-10-17 with throwaway probes (deleted after the run), a project containing `latin1.rs` = `// caf\xe9\nfn ghost() {}\n` and `b.rs`:

```
PROBE4 found=None not_found={"ghost"} cut_short={}
```

and for the references side, `latin1.rs` = `// caf\xe9\nfn caller() { target(); }\n` next to `def.rs`:

```
PROBE4R CorroborationScan { hits: [], cut_short_at: None }
```

Both are the bare negative; neither names the unread file.

## Reproduction

`src/tools/symbol/references.rs`, `corroborate_with_cap`: `let Ok(src) = std::fs::read_to_string(path) else { continue; };` after `budget.admit()`. `src/tools/symbol/call_graph/mod.rs`, `lookup_pos_via_ts_workspace`: the same `let Ok(source) = ... else { continue; }`, then `let Ok(syms) = extract_symbols_from_source(...) else { continue; }`. Both loops iterate `walker.flatten()`, which drops walker errors (an unreadable directory) silently as well.

## Root cause

The budget counts files *admitted*, not files *examined*. `ScanBudget` (`src/tools/symbol/scan_budget.rs`) records only a refusal by the cap, and its docs now say so; nothing records a skip after admission. Measured 2026-10-17 by the probes above.

## Fix

Re-verified at HEAD `e277135e` before changing anything (2026-10-17): the probes still printed `found=None not_found={"ghost"} cut_short={}` and `hits: [], cut_short_at: None`, and the new tests FAILED against the unmodified walks: `an_unreadable_candidate_file_makes_a_miss_incomplete` (`call_graph/mod.rs:1389`), `call_graph_names_the_files_it_could_not_read_when_it_cannot_place_a_caller` (`:1500`), `references_zero_with_an_unreadable_candidate_file_is_warned` (`tests.rs:6205`). Two more silent skips turned up while reading the loops, both the same shape and both reproduced red: a same-language file whose language has NO tree-sitter grammar (a C or Ruby seed) was skipped before being counted, so the walk ended on a clean `NotFound` having parsed nothing (`a_seed_language_with_no_grammar_reports_the_files_it_could_not_parse`, `:1408`); and `.flatten()` dropped every walker error, so a directory the walk could not enter was invisible (`an_unenterable_directory_makes_a_miss_incomplete`, `:1477`; `references_zero_with_an_unenterable_directory_is_warned`, `tests.rs:6236`; both confirmed against a real `chmod 000` directory, so the fixture is not skipped).

**Coverage, not a flag.** The cap's disclosure was one `Option<usize>`; the three new ways to not look are other dimensions of the same fact, so they share one record.

- `src/tools/symbol/scan_budget.rs`: `ScanCoverage { cut_short_at, unexamined, walk_errors }` with `is_complete()`, `merge()` (larger figure, not the sum: two walks over one tree see the same unreadable files) and `describe()`, which returns `None` when complete (ADR clause 2) and otherwise states only what was observed (ADR clause 3): "stopped after N same-language files with more left unexamined", "could not read or parse N same-language file(s)", "could not enter N path(s)". `ScanBudget::record_unexamined()` counts a candidate that could not be read, parsed or has no grammar; `ScanBudget::entry()` is the one funnel for the walker's items and counts the errors it used to drop. The unit is the one the cap already uses, files; walker errors say "path(s)" because an unenterable directory's contents are unknown. A shared `scan_walk` builds the walk for both callers.
- `src/tools/symbol/references.rs`: `CorroborationScan { hits, coverage }`; `corroboration_incomplete_warning` (renamed from `..._cut_short_...`) says the zero is unconfirmed when `hits` is empty and the coverage is not complete. A hit still proves the point and needs no warning.
- `src/tools/symbol/call_graph/mod.rs`: `WorkspaceScan::Incomplete(ScanCoverage)` replaces `CutShort`; `lookup_pos` records the symbol and merges the coverage; `incomplete()`, `scan_incomplete_warning`, `attach_scan_incomplete`; the result fields are now `scan_incomplete_symbols` (bounded to 20) and `scan_incomplete_total`. Same `completeness_warning` channel, so the compact rendering (warning first) and the overflow summary carry it unchanged.
- Pruning: the `node_modules/`/`build/` prune that only `references` had now lives in `scan_walk` and applies to call_graph's Phase B as well (see the tests below). It was a few lines and testable, as the earlier Resume predicted.

**Design choices made, flagged for the human** (each is one line to change):

1. A same-language file with no grammar counts as unexamined for call_graph. Consequence: for a C/C++/Ruby/PHP seed, every unplaceable symbol now carries a warning naming N unparseable files, where it used to read as a clean miss. That is true (Phase B can never find anything there), but it is new noise for those languages if Phase B is considered a no-op for them. Alternative: skip the walk entirely for grammar-less seeds and say so once.
2. Files of another language family and files under pruned directories are never counted (they were never candidates).
3. A parse failure and a read failure share one skip site, because tree-sitter's `parse` returns `None` only on a timeout or cancellation, which no fixture can provoke; the non-UTF-8 test is what exercises that site. A parse failure of a file of the seed's family counts.
4. Pruned directories include a legitimate source directory named `build`, `out`, `dist` or `vendor`; that is `path_in_excluded_dir`'s existing rule, now also applied to the two walks (it already applied to the LSP half of `references`).

Fix commits: recorded under Resume.

## Tests added

- `src/tools/symbol/call_graph/mod.rs`: `an_unreadable_candidate_file_makes_a_miss_incomplete`, `a_seed_language_with_no_grammar_reports_the_files_it_could_not_parse`, `an_unenterable_directory_makes_a_miss_incomplete` (unix, skipped when the process can read a `chmod 000` directory), `call_graph_names_the_files_it_could_not_read_when_it_cannot_place_a_caller` (end to end through `CallGraph::call`, JSON and compact text), `the_walk_does_not_spend_its_budget_on_vendored_directories` (RED `mod.rs:1428`), `the_walk_prunes_below_the_project_root_not_above_it` (guard, green before and after), `the_cap_stops_the_walk_instead_of_only_reporting_it` (new: see the mutation note).
- `src/tools/symbol/tests.rs`: `references_zero_with_an_unreadable_candidate_file_is_warned`, `references_zero_with_an_unenterable_directory_is_warned`, `corroborate_scan_stops_reading_at_its_file_cap` (new, same reason).
- `src/tools/symbol/scan_budget.rs`: `every_kind_of_unexamined_candidate_makes_the_walk_incomplete_on_its_own`, `describe_is_silent_when_complete_and_names_each_gap_otherwise`, `merging_takes_the_larger_figure_not_the_sum`, plus the three boundary tests, now through `coverage()`.
- ADR clause 2 stays covered by the existing silence tests: `a_workspace_walk_that_ends_exactly_on_its_cap_is_a_real_miss`, `references_zero_from_a_completed_corroboration_scan_stays_bare`, `call_graph_stays_bare_when_the_walk_for_an_unplaced_caller_finished`.

Mutations, one per guarded site (observed 2026-10-17, each turned the named tests red and, where stated, only those):

- `call_graph` no-grammar `record_unexamined` removed -> the no-grammar test.
- `call_graph` read/parse `record_unexamined` removed -> `an_unreadable_candidate_file_makes_a_miss_incomplete` and the call_graph e2e test.
- `references` read `record_unexamined` removed -> `references_zero_with_an_unreadable_candidate_file_is_warned`.
- `ScanBudget::entry` no longer counts the error -> the unit test and both unenterable-directory tests (shared site, so both callers).
- `scan_walk`'s prune replaced by `true` -> both vendored-directory tests (shared site).
- `ScanCoverage::is_complete` ignoring `unexamined` -> the unit test, the unreadable test and the no-grammar test.
- `lookup_pos` no longer merging the coverage -> the call_graph e2e tests (including the cited probe test of `call_graph.workspace_files_scan`) and the resolver-level cap test.
- The cap's `break` removed in either walk (the disclosure lives inside `ScanBudget::admit`, so the old cited test stayed green): a gap in the earlier fix, found now and closed by `the_cap_stops_the_walk_instead_of_only_reporting_it` and `corroborate_scan_stops_reading_at_its_file_cap`, each red under that mutation.

## Resume

**Fix commit:** `d5c5694b` on branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5); patch-id `a6f2116cc2322da460627237ee10de2225b3099e`.

N/A once fixed. The design choices listed under Fix (especially 1, the no-grammar seeds) are for the human to confirm.

## References

- `src/tools/symbol/scan_budget.rs`
- `src/tools/symbol/references.rs` (`corroborate_with_cap`)
- `src/tools/symbol/call_graph/mod.rs` (`lookup_pos_via_ts_workspace`)
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
