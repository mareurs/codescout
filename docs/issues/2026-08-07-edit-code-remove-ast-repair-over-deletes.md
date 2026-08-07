---
id: '0fad8145011692a9'
kind: bug
status: open
title: 'BUG: edit_code action=remove over-deleted by 2 lines when repairing a truncated LSP range, silently breaking the file''s syntax'
tags:
- edit_code
- codescout-tool
- ast
- lsp
- data-integrity
---

# BUG: `edit_code action=remove` over-deleted by 2 lines when repairing a truncated LSP range

## Summary

`edit_code(action="remove", symbol="rerank_opt_in_tests")` reported that the LSP returned a truncated
range and that it had "repaired from the AST", then removed **two lines more than the symbol
occupied** — taking the closing `)` and `}` of the *preceding* function with it. The file was left
syntactically invalid. The tool did warn ("verify the result"), and the warning is the only reason it
was caught immediately rather than at the next build.

## Symptom (Effect)

```
edit_code(path="src/retrieval/config.rs", action="remove", symbol="rerank_opt_in_tests")
->
{
  "status": "ok",
  "removed_lines": "20-61",
  "line_count": 42,
  "warning": "LSP returned a truncated range for 'rerank_opt_in_tests' (ended at line 56); repaired from the AST to line 61. The edit used the AST extent — verify the result."
}
```

`symbols(path)` immediately before the removal reported the module as spanning **21-56**. The removal
took **20-61**. Lines 20-21 were the tail of `parse_rerank_opt_in`, the item *before* the module:

```rust
pub(crate) fn parse_rerank_opt_in(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim).unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )      // <-- removed
}          // <-- removed

#[cfg(test)]
mod rerank_opt_in_tests {   // <-- line 21, the actual start
```

Post-removal the file read:

```rust
        "1" | "true" | "yes" | "on"

pub struct RetrievalConfig {
```

`status: "ok"` — no error, no rollback. `edit_code`'s own corruption check
(`CorruptionVerdict`, `src/symbol/edit.rs`) did not fire, presumably because the *target* symbol was
indeed gone and no *sibling symbol* was dropped: what was destroyed was two closing delimiters, which
is not a symbol.

## Reproduction

Not yet reduced to a minimal case. Observed once, on `src/retrieval/config.rs` at
`experiments` around `6ce49487`, with `rust-analyzer` from the pinned 1.97.1 toolchain. The
preconditions present at the time, any of which may be load-bearing:

- the file had been edited several times in the preceding minute (two `edit_code` inserts and two
  `edit_file` edits), so rust-analyzer may have been serving a stale document version
- the target was a `#[cfg(test)] mod` — a cfg-gated item, which rust-analyzer may range differently
- **two `edit_code` calls were issued in the same tool block** (parallel writes), and the
  `PostToolUse` hook flagged exactly that: *"Parallel writes risk inconsistent state (BUG-021) —
  serialize write tool calls."* This is the most likely contributor and the cheapest to test.

## Environment

codescout at `6ce49487` (experiments), rust-analyzer from pinned 1.97.1, Linux. Project: codescout
itself.

## Root cause

**Unknown — see Hypotheses tried.** What is established is only the arithmetic: the AST extent used
for the removal (20-61) is 2 lines wider at the start and 5 wider at the end than the range
`symbols()` reported (21-56), and the leading over-reach destroyed the previous item's delimiters.

*Measured 2026-08-07:* the reported ranges above, and the resulting file content. *Not measured:*
whether the AST repair path or a stale LSP document version produced the wrong start line — the two
are distinguishable by logging both ranges before the splice, which nothing currently does.

## Evidence

The tool's own warning text is the evidence that a repair path ran and that it changed the end line
(56 -> 61). It does **not** mention the start line changing, which is the part that caused the damage
— so either the repair adjusted the start silently, or the start was already wrong when it arrived.

## Hypotheses tried

1. **Hypothesis:** the file had pre-existing syntax errors that broke the parse.
   **Test:** `cargo check --lib` after manually restoring the two lines.
   **Verdict:** rejected — exit 0, so the file was valid Rust both before the repair (it had just
   been written by `edit_code insert`) and after the manual fix.
   **Evidence:** the check ran clean at that point in the session.

2. **Hypothesis:** parallel `edit_code` writes in one tool block left the LSP with an inconsistent
   view (BUG-021).
   **Test:** not run.
   **Verdict:** deferred — this is the leading candidate, flagged by the project's own hook at the
   moment it happened.

## Fix

Plan, not yet implemented. Two independent hardening steps, in order of value:

1. **Extend the corruption check to delimiter balance, not just symbol survival.** `CorruptionVerdict`
   catches a dropped target and dropped siblings; it did not catch this because closing delimiters
   are not symbols. Re-parsing the file after a removal and refusing the edit when the parse fails
   would have rolled this back automatically — and `has_syntax_errors` (`src/ast/parser.rs`) already
   exists for exactly that question.
2. **Log both ranges when the repair path fires.** The warning names the end-line adjustment only.
   Naming the start line too would have made this self-evident instead of requiring a read-back.

## Tests added

None yet. A regression test needs the trigger reduced first (see Reproduction).

## Workarounds

- **Read the file back after any `edit_code` removal that warns about a truncated range.** The
  warning already says to; heed it. That is what caught this within seconds.
- **Serialize write tool calls** — never two `edit_code` calls in one tool block. The hook says so;
  this bug is a candidate consequence of ignoring it.

## Resume

Reduce the trigger: create a small file with a plain `fn` followed by a `#[cfg(test)] mod`, issue two
`edit_code` writes in a single tool block, then `edit_code remove` the module, and compare the
`removed_lines` range against `symbols()`. If that reproduces, hypothesis 2 is confirmed and the fix
is to serialize internally rather than only warn. If it does not, instrument the repair path per Fix
step 2.

## Adjacent gap found in the same session: no supported way to append an item at end-of-file

Not the same defect, but the reason the workaround above had to reach outside the tool surface, so it
belongs with it.

Adding a `#[cfg(test)] mod` at the end of `src/retrieval/config.rs` and `src/retrieval/search.rs` was
necessary (clippy 1.97's `items_after_test_module` requires a test module to be the **last** item in
its file) and turned out to be unreachable through the intended tools:

| attempt | result |
|---|---|
| `edit_file` with the file's tail as anchor | **refused** — *"edit contains a symbol definition (`fn `) — use symbol tools for structural changes"*. Correct guard; a test module is all `fn`s. |
| `edit_code insert, symbol="impl RetrievalConfig", position="after"` | **failed** — *"cannot determine end of 'impl RetrievalConfig' for insert-after — AST parse failed"*, while `symbols(path)` reported that same impl at 62-104 and `cargo check --lib` exited 0. So not a syntax error. |
| `edit_code insert, symbol="from_env", position="after"` | would place the module **inside** the impl block — invalid Rust. |
| `run_command` heredoc append | blocked by the source-file guard, then allowed with `acknowledge_risk: true`. **This is what was used.** |

So when the last item in a file is an `impl` block, there is no non-escape-hatch path to append a new
top-level item after it. The escape hatch worked and was verified immediately (`cargo fmt`, `clippy
--all-targets -D warnings`, 3536 tests), but reaching for `acknowledge_risk` to do something as
ordinary as "add a test module at the end of a file" is a signal about tool coverage rather than about
the author.

**Worth noting the guards were all individually right.** `edit_file` should refuse `fn ` definitions
(BUG-027). Source-file shell access should be gated. The gap is only that the *supported* path
(`edit_code`) cannot anchor on an `impl` block, so the two correct guards compose into a dead end.

**Cheapest fix:** teach `edit_code insert` an explicit end-of-file target — `position="eof"`, needing
no symbol resolution at all and therefore immune to the AST-parse failure above. That single addition
removes the dead end without weakening either guard. Secondary: make `insert-after` on an `impl` work,
since `symbols()` clearly resolves its extent even when the insert path cannot.

## References

- `src/symbol/edit.rs` — `CorruptionVerdict`, the post-edit check that did not catch this
- `src/ast/parser.rs` — `has_syntax_errors`, the predicate Fix step 1 would use
- BUG-021 (parallel writes) — the hook warning present at the time
- F-19 / F-18 in `docs/trackers/release-promotion-session-log.md` — same session
