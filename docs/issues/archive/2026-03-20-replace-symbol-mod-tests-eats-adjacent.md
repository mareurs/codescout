---
status: fixed
opened: 2026-03-20
closed: 2026-05-15
severity: medium
owner: marius
related: ["BUG-032", "BUG-037"]
tags: ["replace_symbol", "edit_code", "ast", "lsp", "stale-position"]
---

# BUG: `replace_symbol` on `mod tests` ate adjacent function body

## Summary

`replace_symbol` against a `mod tests { ... }` block could over-extend its range across an adjacent function and re-emit corrupted text. Comprehensively mitigated 2026-05-15 by the three-guard chain (stale-position detection + AST-authoritative `editing_end_line` + sibling-symbol drop guard).

## Symptom (Effect)

Range computation for the `mod tests` block included a sibling function definition; the replacement clobbered that sibling. Workaround chain over time progressively narrowed exposure until the three-guard chain made the happy path safe.

## Reproduction

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug030_repro.rs` (commit `a45f1bd7`). Variant 1a explicitly exercises the sibling-drop guard — preserve as defense-in-depth.

## Environment

- Date observed: 2026-03-20
- Tool: `replace_symbol` (now consolidated into `edit_code action="replace"`)

## Root cause

Symbol range computation pulled in adjacent function bodies when LSP positions were stale and the AST end-line was either missing or under-reported. Without the three-guard chain, the lenient end-line fallback could land mid-sibling.

## Evidence

Repro fixture exercises the regression: `tests/fixtures/edit-eval-rust/src/bug030_repro.rs`.

## Hypotheses tried

*N/A — migrated from compact form; the original entry tracked progressive mitigations rather than a hypothesis list.*

## Fix

Original mitigation (2026-03-20): `validate_symbol_position` guard detects stale LSP positions and surfaces a `RecoverableError`. Happy path works.

Comprehensive fix (2026-05-15) — three-guard chain:

1. `validate_symbol_position` (`src/symbol/query.rs:227-322`)
2. AST-authoritative `editing_end_line` (`src/symbol/edit.rs`)
3. Sibling-symbol drop guard

## Tests added

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug030_repro.rs` (commit `a45f1bd7`). Variant 1a explicitly exercises the sibling-drop guard.

## Workarounds

Pre-2026-05-15 escape hatch: if `replace_symbol` reported "symbol not found" after a big write, `/mcp` reconnect re-indexed.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-030** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-032 (`remove_symbol` orphaned impl block code), BUG-037 (adjacent impl-block over-capture).
