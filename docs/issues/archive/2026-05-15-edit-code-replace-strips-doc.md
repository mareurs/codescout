---
status: fixed
opened: 2026-05-15
closed: 2026-05-15
severity: medium
owner: marius
related: ["BUG-031", "BUG-054"]
tags: ["edit_code", "doc-comment", "walk-back"]
---

# BUG: `edit_code action="replace"` stripped preceding doc comment when `new_body` omitted it

## Summary

`edit_code(action="replace")` on a function with an immediately-above `///` doc comment (no blank line separating) removed the doc when the caller's `new_body` omitted decorators. The `editing_start_line` walk-back (BUG-031 fix) included the doc comment in the replace range, but the new body had nothing to put back. Fixed 2026-05-15 with forward narrowing logic that preserves the doc when `new_body` does not lead with decorators.

## Symptom (Effect)

```
edit_code(action="replace", body="pub fn documented() -> &'static str {\n    \"after\"\n}")
```

against

```rust
/// Doc that lives immediately above the target with no blank line.
pub fn documented() -> &'static str { "before" }
```

removed the `///` doc comment. Post-state had only the new body, no doc.

## Reproduction

Surfaced by edit_code eval R-08 (`tests/fixtures/edit-eval-rust/src/replace_doc_adj.rs`), rounds 1–6.

## Environment

- Date: 2026-05-15
- Tool: `edit_code` (`action="replace"`)

## Root cause

`editing_start_line` (BUG-031 fix) walked back past `///` / `#[...]` decorators above the keyword line so that a `new_body` containing the doc-comment + signature replaced them cleanly (no duplication). But when the LLM passed a `new_body` that intentionally omitted decorators (e.g. only changing the body), the walk-back dropped the original doc comment — it was inside the replace range but absent from the new body.

## Evidence

R-08 eval reproduction across rounds 1–6 (deterministic).

## Hypotheses tried

1. **Hypothesis:** Always preserve decorators above the symbol regardless of `new_body`. **Verdict:** Rejected — regresses BUG-031 (duplication when `new_body` does lead with decorators).
2. **Hypothesis:** Inspect `new_body`'s first non-empty line and narrow the start forward when no decorator detected. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.

## Fix

Applied 2026-05-15 in `EditCode::do_replace` (`src/tools/symbol/edit_code.rs`): after computing the walk-back `start`, inspect `new_body`'s first non-empty line. If it does NOT start with a decorator (`///` / `//!` / `//` / `#[` / `/**` / `/*` / `@`), narrow `start` forward past any decorator lines (with multi-line `#[...]` bracket tracking) inside the captured range. Result: doc comments / attributes that exist above the symbol but are absent from the new body are preserved; the BUG-031 duplication-prevention path still fires when `new_body` does lead with decorators.

## Tests added

- `tests/symbol_lsp.rs::replace_symbol_preserves_doc_when_new_body_has_no_doc_comment` (mock-LSP unit)
- Edit_code eval R-08 (end-to-end via live rust-analyzer)

## Workarounds

Pre-fix: include the doc comment in `new_body` whenever replacing a documented symbol.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-055** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-031 (walk-back duplication prevention), BUG-054 (stray-brace bug — fixed by same `ea2f314f` commit family).
