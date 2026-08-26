---
id: 01028508fdd0d089
kind: bug
status: mitigated
title: edit_code derives the insert indentation from an unrepaired LSP line index, so a stale range silently picks the wrong column
tags:
- edit_code
- tooling
- lsp
- silent-corruption
topic: edit_code write fidelity
closed: 2026-07-28
unverified: the original +4 shift was never reproduced, so this fix is not known to address the reported symptom; the two hazards it does close were verified independently of the unconfirmed mechanism
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

**Downgraded 2026-07-28, later the same session: the mechanism below is NOT confirmed.**
The original entry asserted "the LSP index is stale, so the wrong line is sampled" as a
finding. Reading the layer between the two functions I had read does not support it. What
is established and what is not, separated:

### Established (read at the bytes)

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

`lines` is `content.lines()` from the disk read at `:802`. The replace path does the same
thing at `:683` with its own range start: `let target_base = leading_ws(lines[start])`.

`editing_start_line` (`src/symbol/edit.rs:43-44`) keys off **`sym.range_start_line`**.
`validate_symbol_position` (`src/symbol/query.rs:292`) validates **`sym.start_line`** — a
different field. It requires the symbol's name to appear on that line, or the line to be
lead-in content (whitespace, brackets, comments, attributes) with the name in a small
window below, and returns a `RecoverableError` so `fetch_validated_symbol` retries with a
fresh `did_change`. `repair_symbol_range` (`:247`) only ever widens `end_line`; it never
touches either start field.

So: **`range_start_line`, the field the indentation is sampled through, is validated by
nothing and repaired by nothing.** That is a real gap, and it is sharper than what this
file originally claimed.

Also established, and independently a defect: `leading_ws` returns the *entire* line for a
whitespace-only line (`leading_ws("   ") == "   "`, pinned by
`util::text::tests::leading_ws_extracts_indent`). If the sampled line is blank-with-
trailing-whitespace, `target_base` becomes that whitespace outright.

### Not established

Which line `range_start_line` actually pointed at, and therefore why the sampled
indentation was 8 rather than 4. The anchor was `tests/reindent_to_preserves_blank_lines`,
whose `#[test]` line and `fn` line — the two plausible values of `range_start_line` — are
**both at column 4**. A truncated range that collapses to a selection range would put
`range_start_line` on the identifier, also column 4. No candidate reading of the observed
range produces 8, so "stale index sampled a body line" remains a guess.

### What narrowed it

A second `edit_code insert`, later in the same session, into the same file and the same
`mod tests`, with a body likewise at column 4, on a `#[test]`-attributed anchor: it landed
at column 4, correctly. The two calls differ in exactly one observable — the first
reported a `range_repair`, the second did not. So the correlation with a truncated range
survives; only the causal story about *which* line was sampled does not. A single
non-reproducing instance is not a mechanism.
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
`docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`
mechanism) and the two
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


3. **Hypothesis:** the stale index escaped detection because nothing validates it.
   **Test:** read `fetch_validated_symbol` -> `validate_symbol_position` ->
   `repair_symbol_range`.
   **Verdict:** half-confirmed, and it moved the entry from "finding" to "open question".
   `range_start_line` really is unvalidated and unrepaired — a genuine gap. But the guard
   on `start_line` means a wholesale stale position would have been *caught and retried*,
   and for this anchor every candidate value of `range_start_line` is at column 4 anyway.
   The gap is real; it is not yet shown to be what happened here.
4. **Hypothesis:** the shift is systematic for `edit_code insert` on a `#[test]` anchor.
   **Test:** a later insert in the same file, same module, same anchor shape, body at
   column 4.
   **Verdict:** rejected — it landed at column 4. One instance, not a property. The one
   observable that differed is the `range_repair` warning on the first call.
## Fix

Two changes are justified by properties verified in the code, independently of the
unconfirmed mechanism. Both make a wrong sample *harmless* rather than trying to make the
sample right, which is the only defence that does not depend on knowing why it was wrong:

1. **Never take a base from a blank line.** `leading_ws` on a whitespace-only line returns
   the whole line, so a sampled blank produces a base out of thin air. Sample the first
   non-blank line at or below the anchor within a small window, falling back to `""`.
   Provably correct, cannot change behaviour on well-formed input, and closes a hole that
   exists whether or not it caused this report.
2. **Prefer the validated field.** The column of a symbol is available from
   `sym.start_line`, which `validate_symbol_position` guarantees on every fetch, rather
   than from `range_start_line`, which nothing checks. `editing_start_line` exists to find
   where the *editing range* begins so a replace carries its doc comment along — a
   different question from "what column does this symbol sit at", and the two only
   coincide because attributes and doc comments are conventionally indented with their
   declaration.

Deferred, pending a reproduction: any change that assumes the index was stale — rejecting
an implausible anchor line, or threading the AST-repaired start into the base. Those are
fixes for a mechanism this entry no longer claims.

Original options 1 and 3, kept for the record: sample from the AST-repaired range; reject
a sampled line that does not look like a declaration, attribute, or comment.
## Tests added

Four, on the new `anchor_indent` helper in `src/symbol/edit.rs`:

- `anchor_indent_reads_the_anchor_line_when_it_has_content` — the ordinary path, at
  columns 0 / 4 / 8.
- `anchor_indent_skips_a_blank_anchor_instead_of_inventing_a_base` — the blank-sample
  hazard, with a companion assertion pinning what the old code *would* have returned
  (`leading_ws("      ") == "      "`), so the test states the defect rather than merely
  avoiding it. The fixture uses trailing whitespace on purpose: it is invisible on screen.
- `anchor_indent_returns_empty_past_the_end_and_past_the_window` — out-of-range anchor,
  and a blank run longer than the window.
- `anchor_indent_at_a_declaration_beats_a_block_comment_continuation` — the case that
  motivated change 2, asserting **both** columns: `"     "` (five, the hazard) at the
  continuation line and `"    "` (four) at the declaration.

No regression test for the original report, deliberately: it has not been reproduced, and
a test asserting a mechanism this file no longer claims would be a fiction with a green
checkmark next to it.

Gate: 18 binaries, clippy `--all-targets -D warnings` clean.
## Workarounds

- Run `cargo fmt` after any `edit_code` insert. It fully repairs a uniform shift so long
  as the body contains no multi-line string literal.
- Avoid multi-line string literals in submitted bodies (required by
  `docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md`
  until its fix reaches the release binary) — that keeps this bug
  in the recoverable class instead of letting the two compound.
- When a call returns a `range_repair` warning, treat the written column as unverified
  and read the region back.

## Resume

**Status: mitigated, not fixed.** The two verified hazards are closed; the reported
symptom is unexplained and unreproduced.

What shipped: `anchor_indent(lines, anchor)` in `src/symbol/edit.rs`, used by both
`do_insert` (`edit_code.rs:820`) and `do_replace` (`:684`), sampling at the validated
`sym.start_line`. It skips blank anchors and bounds a wrong index to a small window.

The replace path's change is a no-op on its narrowed branch — that branch already walks
forward past decorators to the declaration line, which is the same line `start_line`
names. It differs only on the un-narrowed branch, which is the ` * ` continuation hazard.
Checked rather than assumed.

To close this properly, someone needs a reproduction of the original +4 shift. The one
lead: it correlated with a `range_repair` warning, and a second insert into the same file
and module without that warning landed correctly. If it recurs, capture `sym.start_line`,
`sym.range_start_line`, `editing_start_line`'s return, and the sampled line's text in the
same breath — that quartet settles it, and nothing short of it will.

Nothing is outstanding on provenance. That paragraph used to read *"Also outstanding:
master-side SHA after cherry-pick — the SHA here is an `experiments` SHA and orphans on
rebase"*, which was wrong twice over: the practice it named has been retired in favour of a
patch-id recorded once, and **no fix SHA was ever written into this file at all**, so the
sentence worried about the durability of a pointer that did not exist. Recovered and
recorded under § *Fix provenance*.
## References

- `src/tools/symbol/edit_code.rs:808-814` — `sibling_line` / `target_base` / `reindent_to`
- `src/tools/symbol/edit_code.rs:802` — the disk read that `lines` comes from
- `src/symbol/edit.rs:43-44` — `editing_start_line` indexing `lines[r]` from `sym.range_start_line`
- `docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md` —
  the sibling defect this one compounds with, fixed in `79cd1428`
- `docs/issues/archive/2026-07-19-edit-code-insert-after-lands-mid-statement.md` — same
  stale/repaired-range family, but the insert *position* rather than the column
- `docs/issues/archive/2026-05-29-edit-code-kotlin-stale-lsp-range.md` — earliest
  stale-LSP-range entry in this family


## Fix provenance

- **SHA:** `1e7722a0` (`experiments`) — *fix(symbol): sample the reindent base from the
  validated line, never from a blank*. Positional; does not survive a rebase of
  `experiments`.
- **patch-id:** `06f6ef543adeec3b67370c8a83e945034e8121fb` — content hash of the diff;
  survives rebase and cherry-pick.

Recovered 2026-08-19 with `git log -S 'anchor_indent' -- src/symbol/edit.rs`, then confirmed
against the commit's own stat: it carries `src/symbol/edit.rs` (+86) and
`src/tools/symbol/edit_code.rs`, which are the two files § *Fix* describes. `anchor_indent`
is live at `src/symbol/edit.rs:125` with four regression tests.

**The status stays `mitigated`, and the `unverified:` field stays set.** Neither is an
oversight. `doctor`'s `terminal_status_with_caveat` will keep naming this record, which is
the check working — its remedy is *"discharge it and clear the field, **or** leave both"*,
and leaving both is what keeps an unreproduced symptom honest without hiding it from a
triage query.
