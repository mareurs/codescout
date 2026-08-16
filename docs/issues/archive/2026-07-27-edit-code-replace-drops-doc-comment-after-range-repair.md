---
kind: bug
status: fixed
tags:
- edit_code
- lsp
- tooling
- data-loss
- self-inflicted
closed: null
opened: 2026-07-27
owner: marius
related: []
severity: medium
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

**Found 2026-07-29. The hypothesis this section carried was wrong**, and is kept below
because its wrongness is instructive.

### What it actually is

`src/tools/symbol/edit_code.rs`, the `replace` path. `editing_start_line` deliberately
walks *back* past preceding doc comments and attributes, so that a `new_body` containing
doc-comment + signature replaces them cleanly instead of duplicating them (BUG-031). To
avoid dropping documentation when the caller passes a body that omits decorators, the code
then narrows `start` forward again — but only under one all-or-nothing test:

```rust
let body_leads_with_decorator = new_body.lines().find(|l| !l.trim().is_empty())
    .map(|l| { let t = l.trim_start();
        t.starts_with("///") || t.starts_with("//!") || t.starts_with("//")
            || t.starts_with("#[")  || t.starts_with("/**") || t.starts_with("/*")
            || t.starts_with('@') })
    .unwrap_or(false);
let start_narrowed = if !body_leads_with_decorator { /* walk forward */ } else { start };
```

One question, two independent classes of trivia. A body leading with **`#[test]`** answers
"yes, I lead with a decorator", so no narrowing happens at all — the file's `///` lines stay
inside the replaced range and the body, which contains only the attribute, never re-emits
them. The documentation is gone. A body leading with a plain **`//`** comment does the same
thing, and that one is worse, because a plain comment is not a lead region in any
meaningful sense.

So the trigger is *"the new body leads with a decorator of a different class than the one
in the file"*, and it needs no LSP involvement whatsoever.

### Why the original hypothesis was wrong

It read: *"the AST-repair path recomputes the symbol's span … a repair that widens the
range to include \[doc comments\] would delete the comment."* `repair_symbol_range`
(`src/symbol/query.rs:247`) mutates exactly one field:

```rust
if ast_end <= sym.end_line { return None; }
sym.end_line = ast_end;
```

It only ever widens the **end**. It cannot reach a preceding doc comment. The
`range_repair` warning in the report was **correlated, not causal** — the same call
happened to hit a truncated range.

That is the second time in two days a `range_repair` warning was read as the cause of a
nearby symptom; see
`docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`, whose root cause
was downgraded for the same reason. A warning that fires on the same call is evidence of
timing, not of mechanism. The file's own honesty about this — *"That is speculation … the
actual repair code has not been read"* — is what made the correction cheap.
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

Fixed on `experiments`. The narrowing is now **two class-scoped phases** instead of one
all-or-nothing gate, because the lead region holds two independent classes and a single
forward pointer cannot preserve them with a single question.

New helper `skip_lead_region(lines, start, end, class)` in `src/symbol/edit.rs`, with
`LeadClass::{Docs, All}`. It is the old walk, generalised over which line kinds count as
skippable, and it lives beside `anchor_indent` for the same reason — out of the
LSP-dependent path, so it is unit-testable without a harness.

The replace path now runs:

1. **Docs** — if the body supplies no documentation of its own (`///`, `//!`, `/**`,
   `/*`), skip forward past the file's doc-comment lines so they survive. This phase is
   the fix: the old form never reached it when the body led with an attribute.
2. **All** — if the body supplies no lead region at all, continue past attributes too.
   Unchanged behaviour, and it composes: for a body that leads with code, phase 1 then
   phase 2 skip exactly the prefix the single old pass did.

Note what is deliberately *not* fixed: a body that leads with `///` but omits an attribute
the file carries still loses that attribute. A single forward `start` pointer cannot keep
later trivia while replacing earlier trivia, and that direction already has an explicit
channel (the `attributes` param, U-19/U-21) plus a wontfix entry
(`archive/2026-05-18-edit-code-replace-misses-outer-attrs.md`). Widening the fix to cover
it would mean splicing rather than range-replacing, which is a different change.

A plain `//` comment now counts as supplying *no* documentation, so a body opening with
`// TODO` preserves the file's `///` block and lands beneath it — lossless, where before it
overwrote.
## Tests added

Five, on `skip_lead_region` in `src/symbol/edit.rs`. They pin the logic the replace path
now delegates to; the path itself needs a live LSP, which the suite does not stand up.

- `skip_lead_region_docs_stops_at_the_first_attribute` — **the regression.** Asserts
  `Docs` stops at `#[test]` (so the attribute stays replaceable) while `All` continues to
  the declaration. Both numbers asserted, so a mutation collapsing the two classes fails.
- `skip_lead_region_follows_a_multi_line_attribute_to_its_closing_bracket` — a wrapped
  `#[derive(…)]`, whose continuation lines look like ordinary code to a line-wise test.
- `skip_lead_region_treats_block_comments_and_plain_comments_as_docs` — `/** */` bodies
  and plain `//`.
- `skip_lead_region_is_a_noop_when_the_first_line_is_already_code`.
- `skip_lead_region_respects_the_end_bound_and_a_short_slice` — including an `end` past
  the slice, which must not panic.

Full gate: 18 binaries, 3458 passed, 0 failed, 44 ignored; clippy
`--all-targets -D warnings` clean. The BUG-031 / BUG-037 / U-19 / U-21 tests that guard
this exact lead-region behaviour are untouched and green — which is the evidence that the
split is behaviour-preserving everywhere except the case it fixes.
## Workarounds

**Re-read any documented symbol after an `edit_code replace`, especially when the tool
reports a range repair.** Do not rely on the preserve-by-default heuristic when the tool has
just told you it recomputed the range. For a message- or line-level change inside a
documented function, prefer `edit_file` (exact string replace) over `edit_code replace` —
it cannot touch the surrounding comment.

## Resume

Two items, neither blocking:

1. **Live confirmation.** Verified at unit level. Confirming end-to-end means `cargo rb`,
   an `/mcp` reconnect, then `edit_code(action="replace")` on a documented function with a
   body that leads with `#[test]` — assert the `///` block is still above it afterwards.
   Convenient target: `skip_lead_region_docs_stops_at_the_first_attribute` is itself a
   `///`-documented `#[test]`, so it is the fixture and the subject at once.
2. **Master-side SHA** after cherry-pick; the SHA here is an `experiments` SHA and orphans
   on rebase.
## References

- `.superpowers/sdd/2026-07-27-index-lock-and-embedder-batching/final-fix-wave-report.md` —
  § "Incidental repair during Fix 4's edit", the contemporaneous account
- `src/retrieval/index_lock.rs` — `acquire`, the affected symbol (content now correct)
- `replace_symbol_retries_on_stale_lsp_positions_until_fresh` — the existing stale-LSP test,
  where a regression guard for this belongs
- commit `50842163` — the fix-wave commit; its content is unaffected
