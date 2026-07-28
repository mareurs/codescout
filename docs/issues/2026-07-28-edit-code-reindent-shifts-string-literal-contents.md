---
id: b2473c4a0731aaf9
kind: bug
status: open
title: edit_code reindent shifts multi-line string-literal contents, and cargo fmt cannot undo it
tags:
- edit_code
- tooling
- silent-corruption
- rustfmt
closed: ''
opened: 2026-07-28
owner: marius
related:
- docs/issues/archive/2026-07-09-edit-code-replace-drops-visibility-modifier.md
- docs/issues/2026-07-27-edit-code-replace-drops-doc-comment-after-range-repair.md
severity: medium
---

# BUG: edit_code reindent shifts multi-line string-literal contents, and cargo fmt cannot undo it

## Summary

`edit_code` re-bases an inserted/replaced body onto the target symbol's indentation
column via `reindent_to`. The shift is line-wise and literal-blind, so the interior of
a multi-line string literal inside the body gets indented too — silently changing the
string's *value*. `cargo fmt` afterwards restores the surrounding code lines to their
canonical columns but leaves the literal alone (rustfmt does not reformat string
contents by default), so the corruption survives and looks like it was authored that
way. Observed while adding a test whose fixture was markdown with column-0 headings:
the headings became 4-space-indented, the code under test correctly classified them as
body rather than headings, and the test failed for a reason that had nothing to do
with the code being tested.

## Symptom (Effect)

`edit_code(action="insert", position="after", symbol="tests/…")` with a body containing:

```
        let content = "\
# Gotchas

## MCP Binary Symlink

`~/.cargo/bin/codescout` is a symlink.
";
```

landed on disk as:

```
        let content = "\
    # Gotchas

    Preamble line.

    ## MCP Binary Symlink

    `~/.cargo/bin/codescout` is a symlink.
    ";
```

Note the asymmetry that makes this hard to spot: the surrounding statement lines are
at their correct columns (`let content` at 8, the doc comment at 4), so the block
*looks* properly formatted. Only the string's interior moved.

Consequence: the test asserting `##`-heading matching failed with

```
thread 'memory::filter::tests::filter_sections_matches_h2_sectioned_memory' panicked
at src/memory/filter.rs:330:9:
## sections must be matchable
```

The implementation was correct. It rejected `    ## MCP Binary Symlink` because an
indented ATX heading is body content, not a heading — which another test in the same
module (`filter_sections_indented_heading_not_a_boundary`) exists to enforce.

## Reproduction

```
git rev-parse HEAD    # 5b90788d, branch experiments
```

1. `edit_code(path=<rust file>, symbol="tests/<some_test>", action="insert",
   position="after", body=<a test fn whose body contains a multi-line string literal
   with lines at column 0>)`.
2. `cargo fmt`.
3. Read the file. The literal's interior lines carry the target symbol's indentation.

The trigger condition is specifically **`min_indent(body) == 0` caused by the literal
itself**. A body whose code is at column 0 *and* has no multi-line literal reindents
harmlessly; a body already based at the target column is untouched (see Root cause).

## Environment

Linux, codescout `experiments` @ `5b90788d`. No `rustfmt.toml` / `.rustfmt.toml`
anywhere in the repo, so rustfmt runs with defaults — `format_strings = false`, which
is why step 2 cannot repair the literal. Language-agnostic: the reindent is applied to
every language `edit_code` handles.

## Root cause

Two mechanisms compounding.

**1. `edit_code` shifts the whole body, literal included.**
`src/tools/symbol/edit_code.rs:814` on the insert path
(`let reindented = reindent_to(code, &target_base);`) and `:683` on the replace path.
`target_base` is `leading_ws` of the sibling/target line. The intent is documented and
sound — a caller often supplies a body dedented to column 0 from a `symbols` dump, and
splicing that verbatim into a nested symbol produces mis-aligned code or, in Python, a
hard `IndentationError`.

**2. `reindent_to`'s guard against exactly this fails here.**
`src/util/text.rs:86-92`:

```rust
pub fn reindent_to(block: &str, target_base: &str) -> String {
    let agent_base = min_indent(block);
    if agent_base == target_base {
        return block.to_string();
    }
    reindent_block(block, agent_base, target_base)
}
```

Its own doc comment claims the early return *"keeps the transform off lines inside
multi-line string literals in the common path"* — so the hazard was known and this
no-op is the mitigation. The mitigation is defeated when the literal's own column-0
interior is what sets `min_indent` to `""`: the body then looks dedented, the guard
does not fire, and every line shifts. The literal is both the victim and the cause.

**3. `cargo fmt` hides the evidence.** rustfmt re-normalises the code lines to their
canonical columns, so the only surviving trace is inside the literal. Without step 3
the block would look uniformly over-indented and the cause would be obvious.

## Evidence

### rustfmt is not the shifter

`find . -maxdepth 2 -name '*rustfmt*'` → nothing; no config in the repo root. Default
`format_strings = false` means rustfmt never rewrites literal contents. It is the
concealer, not the cause.

### The asymmetry pins the interaction

If `edit_code` alone were responsible, every line would be `+4` — the doc comment at
8, `let content` at 12. On disk they are at 4 and 8, their canonical columns, while
only the literal is `+4`. That is exactly the signature of "shift everything, then let
rustfmt fix what it can".

## Hypotheses tried

1. **Hypothesis:** I authored the test with the literal already indented.
   **Test:** compare the `body` argument sent to `edit_code` against the file on disk.
   **Verdict:** rejected — the argument had those lines at column 0.
2. **Hypothesis:** `cargo fmt` reindented the literal (`format_strings` enabled).
   **Test:** search for any rustfmt config in the repo.
   **Verdict:** rejected — no config exists, so `format_strings` is `false`.
3. **Hypothesis:** `edit_code` uniformly shifts the inserted block by the target
   column.
   **Verdict:** partially confirmed — it does shift uniformly, but the observed
   end state is not a uniform shift because rustfmt then repairs the code lines. The
   full explanation needs both steps.

## Fix

Not implemented. Options, in preference order:

1. **Compute `min_indent` over code lines only** — skip lines inside string literals
   when determining `agent_base`. In the reported case the code lines are at 4/8, so
   `agent_base` would be `"    "`, equal to `target_base`, and the existing early
   return fires: body untouched. This repairs the guard rather than adding a new one,
   and needs no new concept. Requires a cheap literal-span scan (track unescaped `"`
   and `r#"`), not a full parse.
2. **Reindent only lines outside literal spans.** Strictly more correct, and handles
   the case where the code genuinely does need shifting *and* contains a literal —
   which option 1 leaves broken (it would decline to shift at all).
3. **Refuse and hint** when the body contains a multi-line literal and
   `agent_base != target_base`, telling the caller to pass a pre-indented body. Cheap
   and safe, but pushes work onto every caller.

Option 1 fixes the observed bug; option 2 is the real fix. Note the sibling defects in
`related` — `edit_code`'s lead/attribute region has produced two other silent-drop
bugs, so this area warrants the more thorough option.

## Tests added

None yet — no fix. When fixed, the regression test must assert on the string's
**value**, not on the file's formatting: insert a body containing a multi-line literal
with column-0 interior lines into a nested symbol, then assert the resulting literal
still contains `"\n# Heading"` and not `"\n    # Heading"`. A formatting-shaped
assertion would pass while the value stayed corrupted.

## Workarounds

Use `\n`-escaped single-line strings rather than multi-line literals in any body passed
to `edit_code`. That is what the affected test now does, with a comment pointing here
so the next author does not "tidy" it back into a multi-line literal:

```rust
let content = "# Gotchas\n\nPreamble line.\n\n## MCP Binary Symlink\n\n…";
```

Alternatively pass the body already indented to the target column, so `min_indent`
matches `target_base` and the early return fires.

## Resume

Read `min_indent` in `src/util/text.rs` and add a literal-aware variant, then use it
at `src/util/text.rs:87`. Verify with the value-shaped test described above, plus
re-run `cargo test --lib util::text` (4 existing `reindent_to` tests must stay green —
they cover the dedent, no-op, blank-line and shift cases).

## References

- `src/tools/symbol/edit_code.rs:814` — insert-path `reindent_to` call
- `src/tools/symbol/edit_code.rs:683` — replace-path `reindent_to` call
- `src/util/text.rs:86-92` — `reindent_to` and the guard that fails here
- `src/memory/filter.rs` — `filter_sections_matches_h2_sectioned_memory`, the test
  that surfaced it, now written with escaped newlines
- `src/memory/filter.rs` — `filter_sections_indented_heading_not_a_boundary`, the
  test proving the indented-heading rejection is intended behaviour

