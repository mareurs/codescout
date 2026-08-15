---
id: '04cefabc58e23c88'
kind: bug
status: fixed
title: 'BUG: format_hover''s in_code_block is dead state — it strips fence delimiters from docstrings but never actually tracks a code block'
owners:
- marius
tags:
- symbols
- hover
- dead-code
- markdown-parser
- fenced-code
closed: 2026-08-15
opened: 2026-08-14
owner: marius
related:
- docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md
severity: low
---

# BUG: `format_hover`'s `in_code_block` is dead state

## Summary

`format_hover` declares `in_code_block`, toggles it on every line starting with
three backticks, and then never reads it. The loop emits every non-fence line
unconditionally, so the only observable effect is that fence **delimiter lines
are stripped** from rendered hover text while the fenced content itself is kept
and indented like prose. The variable reads as fence tracking to anyone
maintaining the function, but it tracks nothing.

Found incidentally while converting this repo's boolean fence toggles to the
shared `FenceState` tracker for
`docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md`. It is the
one boolean site that was **not** converted, because converting it would change
what hover renders — that is a product decision, not a defect fix.

## Symptom (Effect)

A docstring containing a fenced example renders with the fences gone:

```
/// Example:
/// ```
/// let x = 1;
/// ```
```

renders as

```
  Example:
  let x = 1;
```

The code line survives, the delimiters do not, and nothing marks the example as
code. Whether that is desirable is exactly the open question.

## Reproduction

1. `git rev-parse HEAD` → `141b2cbf` (branch `experiments`).
2. Call `symbols(name=<any symbol whose docstring contains a fenced example>)`
   through a hover-formatting path, or read the function directly.
3. Observe the rendered text has no fence delimiters.

There is no failing assertion — this is a code-shape finding confirmed by
reading, plus the rendering consequence above. `format_hover` has no test
covering a fenced docstring.

## Environment

codescout, branch `experiments` at `141b2cbf`, Linux. Surfaced during the
fence-tracker conversion in the same session.

## Root cause

`src/tools/symbol/display.rs:89-96`, inside `format_hover`
(`src/tools/symbol/display.rs:65-105`). Measured 2026-08-14:
`grep in_code_block src/tools/symbol/display.rs` returns exactly two hits — the
`let mut in_code_block = false;` declaration and the `in_code_block =
!in_code_block;` self-assignment. There is no third occurrence, so no branch
ever consumes the value:

```rust
let mut in_code_block = false;
let mut first_content_line = true;
for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with("```") {
        in_code_block = !in_code_block;
        continue;
    }
    if !first_content_line {
        out.push('\n');
    }
    out.push_str("  ");
    out.push_str(line);
    first_content_line = false;
}
```

`rustc`'s `unused_assignments` does not fire because the self-assignment's RHS
is itself a read, so the chain looks live to the lint. That is why the defect
survived a clean `clippy -D warnings`.

## Evidence

### The variable has exactly two occurrences

```
$ grep -n in_code_block src/tools/symbol/display.rs
89:    let mut in_code_block = false;
94:            in_code_block = !in_code_block;
```

### Two readings, and the file does not say which is intended

- **Abandoned skip.** The author meant `if in_code_block { continue; }` — hover
  should omit code examples — and dropped the guard. Under this reading the
  current output is wrong.
- **Deliberate delimiter strip.** The author wanted the example text without the
  fence noise, and the boolean is leftover scaffolding. Under this reading the
  output is right and the variable should just be deleted.

Nothing in the function, its callers, or its tests discriminates. There is no
test asserting either behaviour, so the suite cannot tell us which one shipped
on purpose.

## Hypotheses tried

1. **Hypothesis:** the value is read further down the function and the grep
   window was too narrow.
   **Test:** `symbols(path=src/tools/symbol/display.rs)` puts `format_hover` at
   lines 65-105; the grep covers the whole file, not a window.
   **Verdict:** rejected — two occurrences in the entire file.
2. **Hypothesis:** a lint would have caught it, so it must be live.
   **Test:** `cargo clippy --workspace --all-targets -- -D warnings` is clean at
   `141b2cbf`.
   **Verdict:** rejected — `unused_assignments` is defeated by the
   self-referential RHS. Clean clippy is not evidence of liveness here.

## Fix

Option 1, shipped in `51283504` — but not as "the conservative default". The
filing's framing was wrong twice, and both errors pointed the same way:

1. **The two readings are not equally defensible.** Option 2 was described as
   "hover then omits code examples entirely". An LSP hover has no code examples
   — rust-analyzer wraps the *signature* in a ```rust fence. Verified live
   2026-08-15 via `symbol_at(src/tools/symbol/display.rs, 65)`, whose rendered
   hover carries `codescout::tools` and `pub mod symbol`: both came out of a
   fenced region. Skipping fenced content would delete the single most useful
   line hover produces.
2. **A test already discriminated them.** This file claimed "There is no test
   asserting either behaviour, so the suite cannot tell us which one shipped on
   purpose." `hover_with_code_fence` (`src/tools/symbol/tests.rs:4764`) asserts
   `result.contains("  pub struct OutputGuard {")` *and*
   `!result.contains("```")` — the fenced content survives, the delimiters do
   not. Option 2 would have failed it. The behaviour was pinned all along.

So the fix is a pure deletion of the two dead lines, output byte-identical, plus
a comment recording *why* the loop strips the delimiter and keeps what it wraps.
No new test: `hover_with_code_fence` already fails in both directions — drop the
`continue` and "```" appears; skip fenced content and the signature vanishes.
## Tests added

None, deliberately. `hover_with_code_fence` already covers both mutation
directions; a second test asserting the same pair would be redundant.

Gate after the change: all 12 `hover` tests green (`cargo test --lib hover`),
clippy `--workspace --all-targets -D warnings` clean.
## Workarounds

None needed — the rendering is cosmetic and lossless (code content survives).
Callers wanting the raw docstring can read the symbol body directly.

## Resume

Closed. No follow-up.

The transferable lesson is R-82's, earned again here: **a bug file's own
reasoning is a hypothesis, not a reading of the code.** This one asserted an
absence ("no test discriminates") that a single `symbols(name=...)` call
refuted. Absence claims in bug files are the cheapest to write and the most
expensive to trust.
## References

- `docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md` — the
  fence-tracker bug whose conversion pass surfaced this site
- `src/util/markdown_fence.rs` — the shared `FenceState` tracker the other seven
  boolean sites were converted to
