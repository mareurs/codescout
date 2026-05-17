---
status: fixed
opened: 2026-04-29
closed: 2026-05-09
severity: high
owner: marius
related: []
tags: ["edit_file", "structural-guard", "silent-corruption"]
kind: bug
---

# BUG: `edit_file` batch silently injected mid-function when `new_string` contained `fn `

## Summary

`edit_file` in batch mode accepted an edit whose `new_string` introduced a new function declaration but whose `old_string` was a single-line match, slipping past `guard_structural_rewrite`. The new function body got spliced mid-existing-fn, corrupting `crates/librarian-mcp/src/catalog/events.rs` until `cargo build` flagged it.

## Symptom (Effect)

- Tool returned success (no error).
- Source file silently corrupted: replacement text injected mid-function (`insert`), splicing into an unrelated fn body.
- Caller noticed only via subsequent `cargo build` failure.

## Reproduction

Batch `edit_file` call against `crates/librarian-mcp/src/catalog/events.rs`. Two edits in one batch. The second edit's `new_string` contained the literal `fn ` (declaring a new private helper function); its `old_string` was a single-line match.

## Environment

- Date: 2026-04-29
- Tool: `mcp__codescout__edit_file` (batch mode with two edits)

## Root cause

Confirmed 2026-05-09: `guard_structural_rewrite` returned `Ok(())` on the very first line when `old_string` lacked a newline, and `find_def_keyword` was only ever called against `old_string`. A single-line `old_string` paired with a multi-line `new_string` that introduced a new symbol slipped through the gate entirely — the new function got spliced into whatever surrounded the anchor match.

## Evidence

- `cargo build` failure naming the corrupted file as ground truth.
- Recovery: re-issued the edit with surrounding context anchor, which forced multi-line `old_string` and tripped the guard correctly.

## Hypotheses tried

*N/A — migrated from compact form; original investigation not recorded as a hypothesis list. Root cause confirmed by inspecting `guard_structural_rewrite` and `find_def_keyword`.*

## Fix

Applied 2026-05-09: `guard_structural_rewrite` now also rejects edits where a multi-line `new_string` contains a definition keyword for the file's language — covers both "rewriting an existing symbol" (old check) and "introducing a new symbol" (new check). Single-line `new_string` containing a `fn` token (e.g. comment edits) remains allowed.

## Tests added

- `batch_edit_blocks_new_symbol_introduction_via_new_string`
- `single_edit_blocks_new_symbol_introduction_via_new_string`
- `singleline_new_string_with_fn_token_still_allowed`

All in `src/tools/edit_file/tests.rs`.

## Workarounds

Use multi-line `old_string` anchors (3+ lines of surrounding context) so the structural guard fires reliably. Or use `edit_code` (`action="insert"`) for any edit that introduces a new symbol.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-050** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: structural-edit gate philosophy in `docs/PROGRESSIVE_DISCOVERABILITY.md`.
