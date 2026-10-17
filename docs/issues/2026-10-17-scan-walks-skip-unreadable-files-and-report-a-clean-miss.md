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

- `src/tools/symbol/scan_budget.rs`: `ScanCoverage { cut_short_at, unexamined, symlinks, walk_errors, no_parser, empty_walk, entries_visited }` with `is_complete()`, `merge()` (larger figure, not the sum: two walks over one tree see the same unreadable files) and `describe()`, which returns `None` when complete (ADR clause 2) and otherwise states only what was observed (ADR clause 3): "stopped after N same-language files with more left unexamined", "could not read or parse N same-language file(s)", "did not follow N symlink(s) that lead outside the project", "could not enter N path(s)", "found no same-language file to look in", and, for a refused walk, only "has no definition parser for <lang>, so it looked in no file". `ScanBudget::record_unexamined()` counts a candidate that could not be read or parsed; `ScanBudget::note_symlink()` the links above; `ScanBudget::entry()` is the one funnel for the walker's items and counts the errors it used to drop. The unit is the one the cap already uses, files; walker errors say "path(s)" because an unenterable directory's contents are unknown.
- `src/tools/symbol/references.rs`: `CorroborationScan { hits, coverage }`; `corroboration_incomplete_warning` (renamed from `..._cut_short_...`) says the zero is unconfirmed when `hits` is empty and the coverage is not complete. A hit still proves the point and needs no warning.
- `src/tools/symbol/call_graph/mod.rs`: `WorkspaceScan::Incomplete(ScanCoverage)` replaces `CutShort`; `lookup_pos` records the symbol and merges the coverage; `incomplete()`, `scan_incomplete_warning`, `attach_scan_incomplete`; the result fields are now `scan_incomplete_symbols` (bounded to 20) and `scan_incomplete_total`. Same `completeness_warning` channel, so the compact rendering (warning first) and the overflow summary carry it unchanged.
- Pruning: a shared `scan_walk` builds the walk for both callers and prunes `node_modules/`. First pass pruned the whole `path_in_excluded_dir` set; narrowed in the second review round, see below.

**Second review round (2026-10-17), all reproduced by reverting the site and observing the test red:**

- *No-parser seeds (supersedes design choice 1 of the first pass, which I had flagged and which the reviewer rejected).* Counting every file of a language with no definition parser as "could not read or parse" named an I/O cause for files that were never opened (ADR clause 3), reported N = every file of the language, appeared on every result with one unplaced node, and each unplaced symbol still cost a full uncapped tree walk. Now `lookup_pos_via_ts_workspace` asks `ast::parser::supports_symbol_extraction(seed_language)` BEFORE walking (the ADR's pre-flight predicate) and returns `Incomplete` with `ScanCoverage::no_parser`, which `describe()` renders once as "has no definition parser for <lang>, so it looked in no file": no file count, no I/O wording, `entries_visited == 0`. The predicate covers languages with no grammar (C, C++, Ruby, C#) and the ones with a grammar but no extractor (HTML, CSS, SCSS, Less, finding 10), and a test ties it to `extract_symbols_from_source` so the two cannot drift. `references`' corroboration is a text match that needs no parser, so it has no such pre-flight.
- *Symlinks.* A symlinked candidate file vanished (`file_type().is_file()` is false for a link) and a symlinked directory was not entered. The walk still does not follow links (the repo's walkers do not: `follow_links(false)` in `librarian/tools/gather.rs`; `librarian/indexer.rs` treats an inward link as a duplicate and an outward one specially). `ScanBudget::note_symlink` counts a link to a candidate file or to a directory whose target is OUTSIDE the project (`coverage.symlinks`, "did not follow N symlink(s) that lead outside the project"); an inward link costs nothing because the walk reaches the target by its own path, and a dangling link has nothing to examine.
- *Empty walk.* The seed's own file is a candidate, so a call_graph walk that admitted none is not complete (`coverage_requiring_a_candidate`, "found no same-language file to look in"). That check would have caught the original vocabulary mismatch. Not applied to `references`, where zero other candidates is a legitimate answer for a file-local symbol.
- *Prune narrowed (finding 7).* The first pass pruned `path_in_excluded_dir`'s set (`target/`, `vendor/`, `build/`, `out/`, `dist/`, `node_modules/`, ...) invisibly: a pruned directory is not in the coverage, so a definition living only in `build/ci.go` (go-ethereum), Go's `go/build` or `cmd/dist` became a clean miss where Phase B used to find it. Chosen: restrict the prune to names that are never source (`node_modules` only; dot-directories are already skipped by the walker, and the build directories that matter in practice are gitignored, which the walk honours), rather than disclose a pruned-directory count, which would put a warning on every miss in any repo with a `target/`. A tree that burns the budget in an unpruned build directory says so through the cap's disclosure, which is the visible failure; an invisible prune is the invisible one.
- *Compiled twins (finding 9, verified red by mutation).* The family widening made `foo.js` (written by `tsc` next to `foo.ts`, not gitignored) a candidate that contains the very identifier it was compiled from, so every zero-reference `.ts` symbol found a "use" in its own build output and got a false "index may still be warming" warning, and the twins spent the file budget. `scan_budget::is_compiled_twin` excludes a `.js`/`.jsx`/`.mjs`/`.cjs` with a `.ts`/`.tsx` sibling of the same stem from both walks; a lone `.js` stays a candidate.
- *Traversal counter.* `ScanCoverage::entries_visited` (not a gap) counts walker entries pulled, so the cap's `break` is observable: the first pass's cap tests survived `break` -> `continue`.

**Design choices made, flagged for the human** (each is one line to change):

1. Files of another language family, files under pruned directories and compiled twins are never counted (they were never candidates).
2. A parse failure and a read failure share one skip site, because tree-sitter's `parse` returns `None` only on a timeout or cancellation, which no fixture can provoke; the non-UTF-8 test is what exercises that site. A parse failure of a file of the seed's family counts.
3. The prune is `node_modules` only (see above); the LSP half of `references` keeps the broader `path_in_excluded_dir` list, so the two halves of one tool still disagree about `build/`. That asymmetry is deliberate: a reference filter reduces noise, a scan must not hide a definition.
4. A no-parser seed still puts one "looked in no file" warning on every result that has an unplaced node (library callees included). It is true and says what is wrong once; if it proves too loud for C/C++ projects, the alternative is to suppress the symbol list for refused walks.

Fix commits: recorded under Resume.

## Tests added

First pass (reds observed against the unmodified walks, 2026-10-17): `an_unreadable_candidate_file_makes_a_miss_incomplete`, `call_graph_names_the_files_it_could_not_read_when_it_cannot_place_a_caller` (end to end through `CallGraph::call`, JSON and compact text), `an_unenterable_directory_makes_a_miss_incomplete` (unix), `the_walk_does_not_spend_its_budget_on_vendored_directories`, `the_walk_prunes_below_the_project_root_not_above_it` in `call_graph/mod.rs`; `references_zero_with_an_unreadable_candidate_file_is_warned` and `references_zero_with_an_unenterable_directory_is_warned` in `tests.rs`; the `ScanBudget` / `ScanCoverage` unit tests in `scan_budget.rs`. ADR clause 2 stays covered by `a_workspace_walk_that_ends_exactly_on_its_cap_is_a_real_miss`, `references_zero_from_a_completed_corroboration_scan_stays_bare`, `call_graph_stays_bare_when_the_walk_for_an_unplaced_caller_finished`.

Second review round (new, 2026-10-17):

- `call_graph/mod.rs`: `a_seed_language_with_no_definition_parser_is_refused_before_walking` (cpp: `no_parser == Some("cpp")`, `entries_visited == 0`, `unexamined == 0`, the single sentence, none of the I/O wording), `a_seed_language_with_a_grammar_but_no_extractor_is_refused_too` (html, css, scss), `a_walk_that_admitted_no_candidate_at_all_is_incomplete`, `a_symlinked_candidate_outside_the_project_is_counted_not_followed`, `only_a_symlink_that_leads_outside_the_project_is_a_gap_for_the_walk`, `a_compiled_twin_of_a_typescript_file_is_not_a_candidate`, `a_definition_living_in_a_build_directory_is_found`, and the rewritten `the_cap_stops_the_walk_instead_of_only_reporting_it` (now asserts the exact number of walker entries pulled, 4: root plus three candidates).
- `tests.rs`: `corroborate_scan_stops_reading_at_its_file_cap` (entries pulled, 4..=5), `corroborate_scan_does_not_cross_into_another_language_family` (now asserts the walk is complete and `describe()` is `None` with `.md`/`.py`/`.ts` files present), `corroborate_scan_counts_a_symlinked_candidate_outside_the_project`, `corroborate_scan_ignores_a_compiled_twin_of_the_definition_file`, `corroborate_scan_reads_a_use_living_in_a_build_directory`.
- `scan_budget.rs`: `a_refused_walk_names_the_missing_parser_and_no_io_cause`, `pulling_entries_alone_does_not_make_a_walk_incomplete`, `a_walk_that_admitted_nothing_is_incomplete_only_for_a_caller_that_needs_a_candidate`, `a_js_file_next_to_a_ts_source_is_a_compiled_twin_and_a_lone_js_file_is_not`, `only_a_symlink_that_leads_outside_the_project_is_a_gap`, `the_locked_dir_guard_restores_the_mode_when_the_test_unwinds`.
- `ast/parser.rs`: `supports_symbol_extraction_matches_the_extractor` (every language in a sample table: predicate and extractor agree).
- The `chmod 000` tests now share `scan_budget::test_support::lock`, which restores the mode in a `Drop` guard (so a failing assertion cannot leave an unenterable directory behind) and prints a `SKIPPED ...` line on stderr when the process can read the directory anyway (root), instead of returning silently.

Mutations, one per guarded site, each reverted and observed red (first round 2026-10-17, second round the same day; every site below turned exactly the named tests red):

- read/parse `record_unexamined` removed (call_graph) -> `an_unreadable_candidate_file_makes_a_miss_incomplete`, the call_graph e2e test; (references) -> `references_zero_with_an_unreadable_candidate_file_is_warned`.
- `ScanBudget::entry` stops counting errors -> unit test and both unenterable-directory tests.
- `ScanCoverage::is_complete` ignoring `unexamined` -> unit test, the unreadable test.
- coverage merge removed from `lookup_pos` -> the call_graph e2e tests and the resolver-level cap test.
- no-parser pre-flight disabled -> the cpp refusal test and the html/css/scss refusal test; an extra language (`html`) added to `supports_symbol_extraction` -> the predicate-vs-extractor test.
- `break` -> `continue` after a refused file, in each walk -> `the_cap_stops_the_walk_instead_of_only_reporting_it` / `corroborate_scan_stops_reading_at_its_file_cap` (both survived this in the first pass, 452 passed); `entries_visited` counter removed -> the same two tests.
- `note_symlink` call removed, in each walk -> the matching symlink test; inward-link guard (`target.starts_with(root)`) dropped -> the scan_budget unit test, the call_graph inward-link test and the references symlink test.
- `is_compiled_twin` made false -> its unit test, the call_graph twin test, the references twin test.
- `coverage_requiring_a_candidate` ignoring an empty walk -> its unit test and the call_graph empty-walk test.
- `PRUNED_DIRS` swapped for `build` -> the two build-directory tests and both node_modules tests.
- references: other-language files counted as unexamined -> `corroborate_scan_does_not_cross_into_another_language_family` (it survived this in the first pass, 448 passed).

## Resume

**Fix commits:** `d5c5694b` (first pass, patch-id `a6f2116cc2322da460627237ee10de2225b3099e`); second review round recorded below. Branch `worktree-agent-ab12363de3a0d43f1` (cut from `experiments` 299395e5).

N/A once fixed. Open for the human: design choice 4 (a refused no-parser walk still warns once per result with an unplaced node).

## References

- `src/tools/symbol/scan_budget.rs`
- `src/tools/symbol/references.rs` (`corroborate_with_cap`)
- `src/tools/symbol/call_graph/mod.rs` (`lookup_pos_via_ts_workspace`)
- `docs/issues/2026-10-16-references-corroboration-scan-cap-yields-a-false-zero.md`
- `docs/issues/2026-10-16-call-graph-workspace-scan-cap-returns-a-false-no-callers.md`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
