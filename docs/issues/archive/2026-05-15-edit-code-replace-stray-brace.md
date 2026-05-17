---
status: fixed
opened: 2026-05-15
closed: 2026-05-15
severity: medium
owner: marius
related: ["BUG-030", "BUG-032", "BUG-055"]
tags: ["edit_code", "ast", "range-over-capture", "trait-impl"]
kind: bug
---

# BUG: `edit_code action="replace"` on trait-method body appended a stray `}`

## Summary

`edit_code(action="replace")` against a trait-method body whose end was near the surrounding `impl` block boundary appended one extra `}` because the symbol range computation included the impl-block's trailing closer. Silently fixed by `ea2f314f` (the narrow-forward fix for BUG-055) — same range-computation path.

## Symptom (Effect)

After replacing the body of `format_compact` inside `impl Tool for ReadMarkdown` in `src/tools/markdown/read_markdown.rs`, the file had one extra `}` at the end. Compile-broken; no data loss.

## Reproduction

Replace the body of `format_compact` inside `impl Tool for ReadMarkdown` (`src/tools/markdown/read_markdown.rs`). New body was multi-branch (CONTENT / MAP) with internal `return` statements.

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug054_repro.rs` (commit `2989dc12`) covers all three variants (single-branch, nested-block, middle-method-in-multi).

## Environment

- Date: 2026-05-15
- Tool: `edit_code` (`action=replace`, `symbol=impl Tool for ReadMarkdown/format_compact`)

## Root cause

Symbol range computation for methods inside an `impl` block could include the trailing closing brace when the body ended near the impl boundary. Related to BUG-030 / BUG-032 (range over-capture) but on `edit_code` not `replace_symbol`.

## Evidence

Phase 1 reproduction across three variants (single-branch, nested-block, middle-method-in-multi) confirmed the fix in `ea2f314f`.

## Hypotheses tried

*N/A — migrated from compact form; root cause was inferred directly from the range over-capture pattern shared with BUG-030 / BUG-032.*

## Fix

Silently fixed by commit `ea2f314f` (the narrow-forward fix for BUG-055) — same range-computation path. Confirmed by Phase 1 reproduction across three variants.

## Tests added

Repro fixture only: `tests/fixtures/edit-eval-rust/src/bug054_repro.rs` (commit `2989dc12`).

**Recommendation (R-09 in edit-code eval):** promote the bug054 fixture into edit-eval as a permanent regression case.

## Workarounds

Pre-fix: sanity-check brace balance after `edit_code action=replace` on method bodies; fix via `edit_file` for the single stray brace.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-054** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Fix commit: `ea2f314f`.
- Related: BUG-030, BUG-032 (range over-capture), BUG-055 (stripped doc comment).
- Edit-eval recommendation: R-09 (promote bug054 fixture into edit-eval regression suite).
