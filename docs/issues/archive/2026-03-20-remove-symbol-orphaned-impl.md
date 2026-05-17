---
status: fixed
opened: 2026-03-20
closed: 2026-05-15
severity: medium
owner: marius
related: ["BUG-030", "BUG-037"]
tags: ["remove_symbol", "edit_code", "ast", "stale-position"]
---

# BUG: `remove_symbol` left orphaned `impl` block code after enum removal

## Summary

`remove_symbol` on an enum could leave the surrounding `impl Trait for Type` block in a half-removed state when the symbol range computation included the wrong brace set. Comprehensively mitigated 2026-05-15 by the three-guard chain (same as BUG-030).

## Symptom (Effect)

After removing the enum symbol, an orphaned `impl` block remained in source, breaking the file structurally. Adjacent / nested `impl Trait for Type` next to inherent `impl Type` was the worst case.

## Reproduction

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug032_repro.rs` (commit `a45f1bd7`).

## Environment

- Date observed: 2026-03-20
- Tool: `remove_symbol` (now consolidated into `edit_code action="remove"`)

## Root cause

Symbol range computation for an enum adjacent to one or more `impl` blocks could pull in the wrong brace set; coupled with stale LSP positions, the removal landed on the wrong span.

## Evidence

Repro fixture exercises the regression: `tests/fixtures/edit-eval-rust/src/bug032_repro.rs`.

## Hypotheses tried

*N/A — migrated from compact form; the original entry tracked mitigations rather than a hypothesis list.*

## Fix

Original mitigation (2026-03-20): same `validate_symbol_position` guard as BUG-030 catches the stale-position case.

Comprehensive fix (2026-05-15) — three-guard chain (`validate_symbol_position` + AST-authoritative end_line + sibling-symbol drop guard).

## Tests added

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug032_repro.rs` (commit `a45f1bd7`).

## Workarounds

Pre-2026-05-15: for adjacent/nested `impl Trait for Type` next to inherent `impl Type`, use `create_file` rather than `remove_symbol`.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-032** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-030 (`replace_symbol` on `mod tests`), BUG-037 (adjacent impl-block over-capture).
