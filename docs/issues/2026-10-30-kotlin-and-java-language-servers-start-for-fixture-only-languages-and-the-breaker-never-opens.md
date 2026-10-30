---
id: '2d67b3ce932ef45f'
kind: bug
status: open
title: Kotlin and Java language servers start for fixture-only languages, and the circuit breaker never opens at their failure rate
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-10-30
severity: low
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

A negative cache that persists across processes, or skip languages whose only files are fixtures.

## Tests added

None yet.

## Workarounds

None.

## Resume

Re-verified 2026-10-30 (no code changed): `git ls-files '*.kt' '*.java' '*.kts'` lists 14 files, all under `tests/fixtures/{kotlin,java}-library`; `search_project_symbols` (`src/tools/symbol/symbols.rs`) takes its language set from an unfiltered `ignore` walk, so tracked fixtures select kotlin and java. Breaker constants are 5 failures / 60 s (`src/lsp/manager.rs`). Both fixes named above change which languages get an LSP answer or add persisted state, so the choice is a design decision, not made here.

## References

- `docs/issues/2026-10-24-java-lsp-prewarm-fails-on-every-codescout-activation.md` (Java only)
- `docs/issues/2026-10-30-project-wide-symbols-waits-out-the-8-s-rust-budget-then-runs-a-sequential-tree-sitter-pass.md`
