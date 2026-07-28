---
id: '9e649674c95cd7bd'
kind: bug
status: open
title: edit_code derives the insert indentation from an unrepaired LSP line index, so a stale range silently picks the wrong column
tags:
- edit_code
- tooling
- lsp
- silent-corruption
topic: edit_code write fidelity
---

# BUG: edit_code derives the insert indentation from an unrepaired LSP line index

## Summary

`edit_code`'s insert path computes the column to re-base the new code onto as
`leading_ws(lines[editing_start_line(&sym, &lines)])`. The index comes from the **LSP**
(`sym.range_start_line`); `lines` comes from a **fresh read of the file on disk**. When
the LSP's view lags the file — which it does after any unindexed edit to that same file —
the index selects a *different line than intended* and whatever indentation happens to
sit there becomes the base for the inserted code.

The tool already detects this staleness class on the *end* side and repairs it from the
AST, emitting a `range_repair` warning. `target_base` is derived from the *start* side,
which the repair does not touch. The insert lands in the right **place** and at the wrong
**column**.

## Symptom (Effect)

Third consecutive `edit_code` call against `src/util/text.rs` (two prior `replace`s had
grown the file by ~100 lines, so the LSP was behind). Inserting a block of tests after
`tests/reindent_to_preserves_blank_lines`, with every line of the submitted body at
column 4 to match the anchor:

```
    #[test]
    fn reindent_to_leaves_multi_line_literal_contents_alone() {
```

landed on disk at column 8:

```
        #[test]
        fn reindent_to_leaves_multi_line_literal_contents_alone() {
```

The same call returned:

```
"warning": "LSP returned a truncated range for 'reindent_to_preserves_blank_lines'
 (ended at line 381); repaired from the AST to line 384. The edit used the AST extent —
 verify the result."
```

so the staleness was both real and *reported* — just not accounted for in the base.

## Reproduction

```
git rev-parse HEAD    # 9fdd4b97, branch experiments
```

1. `edit_code(action="replace", …)` two or three times against one Rust file, growing it
   by enough lines that the LSP's cached ranges no longer line up.
2. `edit_code(action="insert", position="after", symbol="tests/<some_test>", body=…)`
   with every line of `body` already indented to the anchor's column.
3. Read the file. The inserted block sits one indentation step deeper than the anchor.

The anchor matters: a symbol whose *body* is one step deeper than its signature (any
`fn` with statements) is what turns an off-by-N index into an off-by-one-step column.
Point the stale index at a `}` or a blank line and the block lands at column 0 instead.

## Environment

Linux, codescout `experiments` @ `9fdd4b97`, rust-analyzer via the LSP mux. The session
had run `workspace(post_compact=true)` at start, so LSP clients were cold and restarted
lazily — the reads that populated the stale range came from a client that had not seen
the two preceding writes.

## Root cause

`src/tools/symbol/edit_code.rs:808-814`:

```rust
let sibling_line = editing_start_line(&sym, &lines);
let target_base = lines
    .get(sibling_line)
    .map(|l| leading_ws(l))
    .unwrap_or("")
    .to_string();
let reindented = reindent_to(code, &target_base);
```

`lines` is `content.lines()` from the disk read at `:802`. `editing_start_line`
(`src/symbol/edit.rs:43-44`) opens with `if let Some(r) = sym.range_start_line` and
indexes `lines[r]` — an LSP coordinate against on-disk content. Nothing reconciles the
two. The `(sym, symbols, range_repair)` triple returned by `fetch_validated_symbol`
carries the repair signal, and the insert path uses it for the *end* (`range_repair` is
surfaced in the response and `editing_end_line_strict` comes from the AST), but
`target_base` still reads through the unrepaired start.

Two independent things go wrong together, which is why it is easy to misread as a
reindent bug:

1. The index is stale, so the wrong line is sampled.
2. The sampled line's indentation is applied silently — there is no check that the
   submitted body's own base already matched the anchor's.

## Evidence

### The base, not the position

The block landed *after* the anchor test, correctly, and as a well-formed sibling. Only
the column moved. A wrong insertion point would have spliced mid-body (that failure mode
is the already-fixed `2026-07-19-edit-code-insert-after-lands-mid-statement`).

### The shift is exactly one indentation step

Submitted at 4, landed at 8. The anchor `tests/reindent_to_preserves_blank_lines` has
its `#[test]`/`fn`/`}` lines at 4 and its two statement lines at 8. An index off by
+2 lands on a statement line. No other line in that symbol is at 8.

### `cargo fmt` repaired it completely

Because the submitted body used only `\n`-escaped single-line strings, rustfmt restored
every line to column 4 and the result was correct. That is the tell for *this* bug
versus its sibling: a uniform shift with no multi-line literal present is fully
recoverable by the formatter, and therefore invisible in the committed diff. Had the
body contained a multi-line literal, the literal's interior would have kept the +4 (the
`2026-07-28-edit-code-reindent-shifts-string-literal-contents` mechanism) and the two
bugs would have compounded.

## Hypotheses tried

1. **Hypothesis:** the submitted body contained a column-0 line, so `min_indent` read
   `""` and `reindent_to` shifted it — i.e. the string-literal bug again.
   **Test:** re-read the submitted `body`; every non-blank line was at column 4 or
   deeper and every string in it was single-line `\n`-escaped.
   **Verdict:** rejected. `min_indent(body) == "    "`, so with a correct
   `target_base == "    "` the existing early return would have fired and the body
   would have been written byte-for-byte.
2. **Hypothesis:** `target_base` is taken from the anchor's signature line and the
   anchor was misidentified.
   **Test:** read `editing_start_line`'s body.
   **Verdict:** refined, not rejected — the anchor is right, but the *line index* used
   to sample its indentation is an LSP coordinate applied to disk content.

## Fix

Not implemented. Options, in preference order:

1. **Sample the base from the AST-repaired range**, the same source the insert position
   already trusts. `fetch_validated_symbol` returns the repair; use its start rather
   than `sym.range_start_line` when one is present. Smallest change, and it makes the
   two halves of the same call agree on one coordinate system.
2. **Validate before re-basing.** If the submitted body's own `min_indent` is non-empty
   and differs from the sampled `target_base`, the caller stated an intent that
   disagrees with the sample — prefer the caller's, or refuse with a hint. This is the
   defence that does not depend on getting the index right.
3. **Reject a stale index outright**: require the sampled line to be plausible for the
   anchor (its trimmed text should start with the symbol's name, a `#[`, `/`, or the
   language's declaration keyword). A statement line is not a plausible anchor line.

Option 1 alone leaves the failure available whenever the AST also has no opinion.
Option 2 is the one that makes a wrong sample harmless, and is cheap.

## Tests added

None yet — no fix. A regression test wants a `SymbolInfo` whose `range_start_line`
points at a statement line inside the anchor, and must assert that a body submitted at
the anchor's own column is written **unchanged**. Asserting on the final column alone
would pass for the wrong reason once `cargo fmt` runs.

## Workarounds

- Run `cargo fmt` after any `edit_code` insert. It fully repairs a uniform shift so long
  as the body contains no multi-line string literal.
- Avoid multi-line string literals in submitted bodies (already required by
  `2026-07-28-edit-code-reindent-shifts-string-literal-contents`) — that keeps this bug
  in the recoverable class instead of letting the two compound.
- When a call returns a `range_repair` warning, treat the written column as unverified
  and read the region back.

## Resume

Read `fetch_validated_symbol` in `src/tools/symbol/edit_code.rs` to see what the
`range_repair` value carries, then decide whether it exposes a usable repaired start
line. Both the insert path (`:808`) and the replace path (`:683`) compute a
`target_base`; check whether the replace path samples the same way before fixing only
one.

## References

- `src/tools/symbol/edit_code.rs:808-814` — `sibling_line` / `target_base` / `reindent_to`
- `src/tools/symbol/edit_code.rs:802` — the disk read that `lines` comes from
- `src/symbol/edit.rs:43-44` — `editing_start_line` indexing `lines[r]` from `sym.range_start_line`
- `docs/issues/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md` — the
  sibling defect this one compounds with
- `docs/issues/archive/2026-07-19-edit-code-insert-after-lands-mid-statement.md` — same
  stale/repaired-range family, but the insert *position* rather than the column
- `docs/issues/archive/2026-05-29-edit-code-kotlin-stale-lsp-range.md` — earliest
  stale-LSP-range entry in this family

