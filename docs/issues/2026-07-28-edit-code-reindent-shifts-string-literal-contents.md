---
id: b2473c4a0731aaf9
kind: bug
status: fixed
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

Fixed on `experiments` — **option 2**, the literal-span-aware reindent, plus the guard
repair option 1 described. Both, because they close different halves of the surface.

`src/util/text.rs` gains a small literal scanner:

- `literal_continuation_mask(block) -> Vec<bool>` marks lines that *begin* inside a
  literal opened on an earlier line. Those lines' leading whitespace is string content,
  not indentation. The opener line is never marked (its indent really is code), and the
  closing line is (the bytes before its closing token are still string content) — so
  interior, closer, and opener are each handled correctly by one rule.
- `min_indent_outside_literals` measures the base over unmarked lines only. This is
  option 1: in the reported case the code lines are at 4/8, so the base is `"    "`,
  equals `target_base`, and `reindent_to`'s existing early return fires — the block comes
  back byte-for-byte.
- `reindent_block` emits marked lines **verbatim**. This is option 2, and it covers the
  case option 1 alone cannot: a body whose code genuinely does need shifting *and* which
  contains a literal. The code shifts, the literal does not.

The scanner recognises every line-spanning literal form the supported languages use — a
`"`/`'` literal held open by a trailing `\`, a triple-quoted Python literal, a backtick
(JS/TS template, Go raw string), and a Rust raw string at any hash count — and tracks
escapes in all but raw strings, where `\` is data and must not hide the closing token.

Two decisions worth keeping:

**The fix went into `reindent_block`, not into `reindent_to`'s base computation.** Caller
enumeration turned up a second production entry point the report missed:
`src/tools/edit_file/mod.rs:747` calls `reindent_block` directly, with bases taken from
the first non-blank line rather than from `min_indent`, on the whitespace-normalized-match
repair path. It shifts literals the same way, and its post-edit `has_syntax_errors` guard
cannot catch it — a literal with four extra spaces still parses. Masking inside the
shifter fixes both entry points; masking inside `reindent_to` would have fixed one.

**Ambiguity resolves toward "this is code".** A `"`/`'` left unclosed at end of line
resets to code rather than latching, so a lifetime (`&'a T`) or an apostrophe in prose
cannot mask the rest of the block; a `//` line comment opens nothing, so the odd backtick
count in a markdown-flavoured doc comment cannot either. The asymmetry is the reason: a
mis-indented code line is a loud failure — compiler, formatter, review — and a mutated
string literal is a silent one. The worst residual case is a line-spanning literal that
never closes, which masks everything after the opener and so degrades to reindenting one
line instead of corrupting a string.
## Tests added

Six, in `src/util/text.rs`'s `tests` module. All value-shaped, as the original entry
required — a formatting-shaped assertion would pass while the string stayed corrupted.

- `reindent_to_leaves_multi_line_literal_contents_alone` — the headline regression: the
  reported fixture, asserting the block returns **byte-for-byte** unchanged.
- `reindent_to_shifts_code_and_leaves_the_literal_where_it_was` — the case option 1 alone
  would not have fixed; asserts the code moved *and* that the literal did not gain
  indentation.
- `literal_continuation_mask_covers_each_line_spanning_form` — triple-quote, backtick,
  hashed Rust raw string, and the backslash-continued literal running past one line end.
- `literal_continuation_mask_does_not_latch_on_prose_quotes` — a lone lifetime tick and a
  line comment with an odd backtick count must mask nothing.
- `literal_continuation_mask_honours_escapes_inside_a_continued_literal` — `\"` must not
  read as the closing quote; and the raw-string converse, where `\` must not hide `\"#`.
- `reindent_block_emits_literal_continuations_verbatim` — pins the `edit_file` entry
  point, which passes explicit bases.

The four pre-existing `reindent_to` tests (dedent, no-op, shallower-target, blank-line)
stay green untouched. Full gate: 18 binaries, 3445 passed, 0 failed, 44 ignored; clippy
`--all-targets -D warnings` clean.

Every fixture uses `\n`-escaped strings rather than multi-line literals, with a comment
in the file saying why: a multi-line literal there would be re-indented by the very
defect these tests pin, corrupting the fixture on the way in. That constraint lifts once
the release binary carries this fix.
## Workarounds

**Still required until the release binary is rebuilt.** The running MCP server executes
`~/.cargo/bin/codescout` -> `target/release/codescout`; this fix exists only in the debug
build until `cargo rb` followed by an `/mcp` reconnect. Until then, keep using
`\n`-escaped single-line strings in any body passed to `edit_code`, or pass the body
already indented to the target column so the early return fires.

After the rebuild, neither workaround is needed: a multi-line literal in a submitted body
keeps its value whether or not the surrounding code needs shifting.
## Resume

Two loose ends, neither blocking:

1. **Live-MCP confirmation is outstanding.** Verified at unit level only. Confirming it
   end-to-end means `cargo rb`, an `/mcp` reconnect, then an `edit_code` insert carrying
   a genuine multi-line literal — assert the written literal still contains `"\n# "` and
   not `"\n    # "`.
2. **Master-side SHA still needs recording** after cherry-pick. The SHA below is an
   `experiments` SHA and orphans on rebase.

One finding from the fix is filed separately and is *not* closed by it:
`docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`. The insert that
added these very tests landed 4 columns too deep because `edit_code` samples the insert
column through an unrepaired LSP line index. `cargo fmt` fully repaired it here only
because the fixtures contain no multi-line literal — with one present, the two defects
compound and the literal keeps the shift.
## References

- `src/tools/symbol/edit_code.rs:814` — insert-path `reindent_to` call
- `src/tools/symbol/edit_code.rs:683` — replace-path `reindent_to` call
- `src/util/text.rs:86-92` — `reindent_to` and the guard that fails here
- `src/memory/filter.rs` — `filter_sections_matches_h2_sectioned_memory`, the test
  that surfaced it, now written with escaped newlines
- `src/memory/filter.rs` — `filter_sections_indented_heading_not_a_boundary`, the
  test proving the indented-heading rejection is intended behaviour
