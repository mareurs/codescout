---
id: '2d67b3ce932ef45f'
kind: bug
status: mitigated
title: Kotlin and Java language servers start for fixture-only languages, and the circuit breaker never opens at their failure rate
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-31
opened: 2026-10-30
severity: low
unverified: 'The skip and the failure note in `search_project_symbols` are wiring that no unit test reaches: the policy in `LspLegHealth` is tested, the call site is not. No live check after a rebuild. The 5-in-60-s breaker, the explicit-path `symbols` calls and the prewarm path still retry a failing start.'
---

# BUG: Kotlin and Java language servers start for fixture-only languages, and the circuit breaker never opens at their failure rate

## Summary

`git ls-files` finds only 12 `.kt`/`.java` files, all under `tests/fixtures/`, but a project-wide `symbols` call starts the Kotlin mux each time. The circuit breaker opens only after 5 failures within 60 s (`src/lsp/manager.rs:915-935`, `:1630-1650`), so failures spaced further apart never trip it.

## Class fit

`cluster/selector-narrower-than-its-population`: the breaker selects failures inside a 60 s window, while the population of repeated failures is spread over minutes and hours. It differs from other members because the cost is repeated start-up time, not a wrong result.

## Symptom (Effect)

`.codescout/debug.log` 2026-10-30: `mux startup failed for kotlin` ~2 s each, 4 times in 3 minutes. usage.db `lsp_events` since 2026-10-23: kotlin failed 10 times; java failed 74 times, avg handshake 16.9 s. Today it runs alongside the 8 s Rust budget and costs no wall time; once that is fixed, it becomes the floor (~2 s).

## Reproduction

Project-wide `symbols(name=X)` on codescout; grep debug.log for `mux startup failed`.

## Environment

Linux; codescout declares languages bash, java, javascript, kotlin, markdown, python, rust, typescript.

## Root cause

- `src/lsp/manager.rs:915-935`, `:1630-1650`: 5-in-60-s breaker.
- measured 2026-10-30: debug.log and `lsp_events` above.

## Evidence

See Symptom.

## Hypotheses tried

None.

## Fix

Landed on experiments in 9e22606cd (`src/tools/symbol/symbols.rs`). `LspLegHealth` remembers a `get_or_start` failure per (language, root), and `search_project_symbols` skips that language for `LANG_START_FAILURE_COOLDOWN` (10 minutes). The tree-sitter pass still covers the language's files. An LSP error after a successful start does not open the cooldown. An answer clears it.

This is a per-process cooldown. It was chosen over a persisted negative cache, and over a fixture-path rule: the repo has no fixture predicate to reuse, and a path rule would guess at other repos' layouts. The 5-in-60-s breaker is unchanged.

## Tests added

Five tests in `project_search_cost_tests` (`symbols.rs`):

- never-failed is not in the cooldown
- the cooldown outlasts the 60 s breaker window (asserts at 90 s)
- a second failure does not extend it
- per-key isolation
- an answer ends it

Mutation-checked: a 30 s cooldown fails the 90 s test, and removing the `!in_start_cooldown` guard fails the does-not-extend test. The call-site wiring has no test.

Gate: green (FMT/CLIPPY/LEAN/DEFAULT/PYTHON_LIGHT = 0) on the tree before the last two rebases onto experiments. On the final tree I ran only clippy and the 28 `symbols` tests.

The first gate run had four red tests in `tools::run_command`: `a_directory_swapped_in_after_first_use_is_not_run`, `a_red_attaches_wip_authors_on_the_main_arm`, `a_red_attaches_wip_authors_on_the_buffer_only_arm` and `a_red_naming_many_dirty_files_carries_a_bounded_wip_authors`. All four passed alone and in the next two full gate runs. They look load-sensitive. That is not diagnosed.

## Workarounds

None.

## Resume

Re-verified 2026-10-30 (no code changed): `git ls-files '*.kt' '*.java' '*.kts'` lists 14 files, all under `tests/fixtures/{kotlin,java}-library`; `search_project_symbols` (`src/tools/symbol/symbols.rs`) takes its language set from an unfiltered `ignore` walk, so tracked fixtures select kotlin and java. Breaker constants are 5 failures / 60 s (`src/lsp/manager.rs`). Both fixes named above change which languages get an LSP answer or add persisted state, so the choice is a design decision, not made here.

## References

- `docs/issues/2026-10-24-java-lsp-prewarm-fails-on-every-codescout-activation.md` (Java only)
- `docs/issues/2026-10-30-project-wide-symbols-waits-out-the-8-s-rust-budget-then-runs-a-sequential-tree-sitter-pass.md`
