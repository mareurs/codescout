---
status: open
opened: 2026-07-27
closed:
severity: medium
owner: marius
related: []
tags: [edit_code, lsp, tooling, data-loss, self-inflicted]
kind: bug
---

# BUG: `edit_code` structural `replace` silently dropped a symbol's doc comment after an AST range repair

## Summary

`edit_code(action="replace")` on a documented function removed the function's leading `///`
doc comment along with the body, despite documenting doc comments as preserved by default.
It happened on a call where the tool itself warned that the LSP had returned a truncated
range which it had "repaired from the AST" — so the repaired range appears to absorb the
doc comment into the replaced region without re-emitting it. Silent content loss in codescout's
own primary code-editing tool.

## Symptom (Effect)

Observed 2026-07-27 by an implementer subagent editing
`src/retrieval/index_lock.rs`'s `acquire` function (rewriting its `.with_context` message).
The tool surfaced a warning of roughly this shape alongside a successful edit:

```
LSP returned a truncated range for 'acquire' ... repaired from the AST
```

The replace succeeded, but the function's leading `///` doc block was gone from the result.
Caught only because the implementer re-read the file after the edit; restored verbatim via a
follow-up text edit before compiling. Nothing in the tool's own output flagged the loss —
the warning is about the *range*, not about discarded content.

## Reproduction

Not yet reduced to a minimal case. Best lead: the loss appears tied to the range-repair
path, not to `replace` generally — `edit_code replace` on documented symbols is used
routinely in this repo without dropping comments, and the same session performed several
such edits successfully. To reproduce, look for a symbol where the Rust LSP returns a
truncated `full_range` (large function, or one edited moments earlier so the LSP's view is
stale) and replace it while the AST-repair warning fires.

The stale-LSP angle is worth trying first: this same file had just been edited, and
`edit_code` has a documented retry-on-stale-position path
(`replace_symbol_retries_on_stale_lsp_positions_until_fresh` in the test suite), which
suggests LSP staleness in this file's editing flow is a known live condition.

## Environment

- codescout `0.15.0`, branch `experiments`, commit `50842163`
- `src/retrieval/index_lock.rs`, function `acquire` (documented with a multi-line `///` block)
- Rust LSP (rust-analyzer) via codescout's LSP mux
- Edit performed by an implementer subagent through the MCP `edit_code` tool

## Root cause

Unknown — under investigation. Hypothesis, in mechanism language: when the LSP returns a
truncated range, the AST-repair path recomputes the symbol's span from the tree-sitter node.
A tree-sitter `function_item` node does **not** include preceding `///` line comments (they
are siblings, not children), so a repair that widens the range to include them — or a
preserve-heuristic that reads the doc comment relative to the *LSP* range and then applies it
to the *repaired* range — would delete the comment without re-emitting it. That is
speculation from the observed behaviour and the tool's own warning text; the actual repair
code has not been read.

## Evidence

### Implementer's contemporaneous note (final fix wave, 2026-07-27)

Quoted from
`.superpowers/sdd/2026-07-27-index-lock-and-embedder-batching/final-fix-wave-report.md`
§ "Incidental repair during Fix 4's edit":

> `edit_code`'s structural `replace` on the `acquire` function silently dropped its leading
> `///` doc comment on the first attempt (the tool surfaced a warning: "LSP returned a
> truncated range for 'acquire' ... repaired from the AST" — the AST-repaired range
> apparently absorbed the doc comment into the replaced region without preserving it,
> contrary to the tool's documented default-preserve heuristic). Caught immediately by
> re-reading the file after the edit; restored the doc comment verbatim via a follow-up text
> edit before proceeding.

### Why the committed content is clean

The loss was caught and repaired before `cargo fmt`/`clippy`/`cargo test` and before the
commit, so `50842163` contains the correct doc comment. The bug is in the tool, not in the
shipped code.

## Hypotheses tried

1. **Hypothesis:** the implementer simply omitted the doc comment from its replacement text.
   **Test:** read the report's account and the committed file.
   **Verdict:** rejected. The implementer's replacement targeted only the `.with_context`
   message string, and it reported the tool's range-repair warning at the same call — it did
   not author a body without the comment.

## Fix

Not started. Two directions, cheapest first:

1. **Make the loss loud.** When the AST-repair path fires, compare the replaced region's
   leading trivia against the emitted text and refuse (or warn explicitly) if a `///` /
   `//!` / `/** */` block would be dropped. A tool that silently discards documentation is
   worse than one that errors.
2. **Fix the range.** Ensure the repaired range starts *after* any preceding comment trivia
   (matching what the preserve heuristic assumes), rather than absorbing it.

Filed per CLAUDE.md § Bug Tracking: *"Open a bug file for ANY bug noticed during work —
including incidental bugs we won't fix and tool quirks/misbehaviors."* The implementer
declined to file it on the grounds that committed content was unaffected; that reasoning
covers *this* edit, but the tool will do the same thing on the next documented symbol whose
range needs repair, and the next agent may not re-read the file.

## Tests added

None yet. A regression test belongs next to
`replace_symbol_retries_on_stale_lsp_positions_until_fresh` (the existing stale-LSP guard in
the `edit_code` suite): replace a documented symbol under a forced truncated-range condition
and assert the doc comment survives.

## Workarounds

**Re-read any documented symbol after an `edit_code replace`, especially when the tool
reports a range repair.** Do not rely on the preserve-by-default heuristic when the tool has
just told you it recomputed the range. For a message- or line-level change inside a
documented function, prefer `edit_file` (exact string replace) over `edit_code replace` —
it cannot touch the surrounding comment.

## Resume

Reduce to a minimal reproduction first: find a symbol whose LSP `full_range` comes back
truncated (try editing a documented function, then immediately `edit_code replace` it again
so the LSP view is stale) and confirm the doc comment drops. Then read the AST-repair path
that emits the "repaired from the AST" warning and check how it computes the replaced span
relative to leading comment trivia. Decide between the two fix directions once the mechanism
is confirmed rather than hypothesised.

## References

- `.superpowers/sdd/2026-07-27-index-lock-and-embedder-batching/final-fix-wave-report.md` —
  § "Incidental repair during Fix 4's edit", the contemporaneous account
- `src/retrieval/index_lock.rs` — `acquire`, the affected symbol (content now correct)
- `replace_symbol_retries_on_stale_lsp_positions_until_fresh` — the existing stale-LSP test,
  where a regression guard for this belongs
- commit `50842163` — the fix-wave commit; its content is unaffected
