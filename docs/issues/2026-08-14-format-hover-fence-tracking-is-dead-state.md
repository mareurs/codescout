---
id: '04cefabc58e23c88'
kind: bug
status: open
title: 'BUG: format_hover''s in_code_block is dead state — it strips fence delimiters from docstrings but never actually tracks a code block'
owners:
- marius
tags:
- symbols
- hover
- dead-code
- markdown-parser
- fenced-code
closed: null
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

Not implemented, and deliberately not bundled with the fence-tracker
conversion. Two candidates, and picking one is a rendering decision:

1. **Delete the variable.** Keeps today's output byte-identical, removes the
   misleading state. Add a test pinning "fenced docstring renders without
   delimiters" so the behaviour is asserted rather than accidental.
2. **Make it real** — convert to `crate::util::markdown_fence::FenceState` and
   add the missing `if fence.in_fence() { continue; }`. Hover then omits code
   examples entirely, which changes output for every docstring with a fence.

Option 1 is the conservative default; option 2 is only right if hover is meant
to be prose-only. Either way the fix is ~5 lines plus one test.

## Tests added

None. `format_hover` currently has no test covering a fenced docstring — that
absence is part of the finding, and whichever option is chosen should close it.

## Workarounds

None needed — the rendering is cosmetic and lossless (code content survives).
Callers wanting the raw docstring can read the symbol body directly.

## Resume

Decide option 1 (delete, pin current output) or option 2 (skip fenced content),
then implement in `format_hover` at `src/tools/symbol/display.rs:65-105` and add
the missing fenced-docstring test to the `tests` module at
`src/tools/symbol/display.rs:495-603`. Do not convert this site to `FenceState`
without making that choice first — a mechanical conversion silently ships
option 2.

## References

- `docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md` — the
  fence-tracker bug whose conversion pass surfaced this site
- `src/util/markdown_fence.rs` — the shared `FenceState` tracker the other seven
  boolean sites were converted to

