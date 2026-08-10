---
id: '0fad8145011692a9'
kind: bug
status: fixed
title: 'BUG: edit_code action=remove over-deleted by 2 lines when repairing a truncated LSP range, silently breaking the file''s syntax'
tags:
- edit_code
- codescout-tool
- ast
- lsp
- data-integrity
closed: 2026-08-08
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

**Two layers, and reading the code corrected this file's account of the second.**

**1. The wrong range — still unknown.** The AST extent used for the removal (20-61) is 2
lines wider at the start and 5 wider at the end than the range `symbols()` reported
(21-56), and the leading over-reach destroyed the previous item's delimiters. *Measured
2026-08-07:* the ranges above and the resulting file content. *Not measured:* whether the
repair path or a stale LSP document version produced the wrong start line. Still open as a
question; **the fix below does not depend on the answer**, which is the point of it.

**2. The damage was not caught — CORRECTED 2026-08-08.** *Read, not inferred:*
`do_remove` (`src/tools/symbol/edit_code.rs`) wrote the file and returned. **It had no
post-edit verification of any kind** — no pre-AST, no post-AST, no `corruption_verdict`
call, no rollback. `references(symbol="corruption_verdict")` returned exactly **one**
production call site, in `do_replace`.

The *Symptom* section above guessed that `CorruptionVerdict` ran and declined to fire
("presumably because the target symbol was indeed gone and no sibling symbol was
dropped"). That guess was reasonable and wrong. `remove` — the one action whose entire
purpose is deleting a range — was the one action with no safety net, while `replace` next
door had a full one. Left for the record, since the wrong guess is what a reader would
reach for again.
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

**Implemented on `experiments`.** Deliberately root-cause-independent: the trigger is
still unreduced, so the fix makes the *class* of damage impossible to land silently rather
than chasing one instance.

**1. `syntax_regressed` (`src/symbol/edit.rs`)** — did this edit turn a file that parsed
into one that does not? Uses the pre-existing `has_syntax_errors`. **Gated on the
pre-image parsing**: without that clause every edit to an already-broken file would be
refused, which is exactly when someone is repairing it. Languages with no grammar yield
`false`, so nothing is refused on a guess.

**2. `CorruptionVerdict::SyntaxBroken`** — a new verdict, and both `replace` and `remove`
roll back on it. This reaches the damage the name-set checks structurally cannot: dropping
a closing delimiter loses no symbol NAME, so the file was reported `Clean`.

**3. `do_remove` gets the verification block it never had** — pre-AST, post-AST, verdict,
rollback, and the `Unverified` surfacing, matching `do_replace`. `pre_count` is passed as
`0` on purpose: dropping the target IS the operation, so the `TargetDropped` arm must never
fire there. What means something on a removal is a *sibling* vanishing, or the parse
breaking.

**Ordering — corrected mid-implementation by an existing test.** The first attempt ranked
`SyntaxBroken` first, on the reasoning that a broken parse is the strongest signal. The
integration test `replace_symbol_rejects_body_only_for_nested_method` failed and was right
to: a body-only `replace` both drops the target and breaks the parse, and answering it with
*"the range overshot into adjacent code"* is a **wrong diagnosis** for what is actually
"you passed statements instead of a whole declaration". The name checks now run first — when
a symbol vanished, that is the cause and the broken parse is its consequence — and
`SyntaxBroken` sits between them and `Unverified`, where a stopped parse is often *why*
re-extraction failed.

**Not done, and still worth doing (step 2 of the original plan): log both ranges when the
repair path fires.** The warning names the end-line adjustment only; naming the start line
too would make the next occurrence self-diagnosing instead of requiring a read-back. It is
the cheapest remaining step toward layer 1.
## Tests added

Two, in `src/symbol/edit.rs`:

- `syntax_regression_is_the_net_under_the_name_checks_not_over_them` — fires when every
  symbol name survived (the reported shape); outranks `Unverified`; **yields** to
  `TargetDropped` and `SiblingsDropped`, which is the ordering the integration test forced;
  and a control asserting the same inputs with intact syntax still return `Clean`, without
  which the guard would refuse every healthy edit.
- `syntax_regressed_blames_only_the_edit_that_broke_it` — the pre-image gate. An
  already-broken file must not have every subsequent edit refused; an edit that *fixes*
  syntax is not a regression; a language with no grammar is never blamed.

Also exercised live during the fix: `edit_code replace` refused an edit of mine that renamed
a function inside the replacement body, rolling it back with the `TargetDropped` message —
the same precise wording the ordering correction preserves.

No regression test for the original trigger. The trigger is still unreduced (see *Resume*),
and a test asserting "a wrong range gets rolled back" is what the two above already prove.

Gate: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 3578
passed / 0 failed / 44 ignored.
## Workarounds

- **Read the file back after any `edit_code` removal that warns about a truncated range.** The
  warning already says to; heed it. That is what caught this within seconds.
- **Serialize write tool calls** — never two `edit_code` calls in one tool block. The hook says so;
  this bug is a candidate consequence of ignoring it.

## Resume

The **damage** is fixed; the **trigger** is not, and is now much cheaper to hunt because a
recurrence rolls back and reports instead of landing silently.

1. **Confirm CI**, then archive via `artifact(action="move", …)`. No master-side SHA to
   record — fast-forward promotion (`docs/RELEASE.md` § *Large-Cohort Promotion*).
2. **Optional, and the cheapest next step toward layer 1:** make `RangeRepair::warning`
   (`src/symbol/query.rs`) name the START line as well as the end. It currently reports
   only the end-line adjustment, which is why the damaging change was invisible in the
   warning text.
3. **The reduction described below is still worth running** — it would confirm or reject
   hypothesis 2 (parallel writes / BUG-021), which remains untested.

Reduce the trigger: create a small file with a plain `fn` followed by a `#[cfg(test)] mod`,
issue two `edit_code` writes in a single tool block, then `edit_code remove` the module, and
compare the `removed_lines` range against `symbols()`. If that reproduces, hypothesis 2 is
confirmed and the fix is to serialize internally rather than only warn.
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
