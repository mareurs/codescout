---
kind: bug
status: fixed
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-14
fix_patch_id: e3da14808a0df6174c949e024f1b3493141ad061
fix_sha: 989fa34c
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

# The mask that removes false keywords fabricates one

## Summary

`edit_file`'s IL-2 structural guard blanks comments and string literals before scanning for
definition keywords, so that a keyword *mentioned* in prose does not read as a definition. On a
line containing an ordinary English possessive, that blanking pass **creates the keyword it exists
to suppress**.

`each class's field` contains no `class ` — there is an apostrophe after `class`, not a space. The
mask turns it into `each class` followed by spaces, and the guard then matches `"class "` and
refuses the edit. The needle is absent from the source and present only in the mask.

This is **not** the archived
`docs/issues/archive/2026-09-11-edit-file-reads-a-keyword-in-prose-as-a-symbol-definition.md`,
which was about a keyword genuinely present in a comment. That bug's fix (`c9c03a74`, shape 2)
introduced the blanking pass. This is the new failure mode the fix brought with it.

## Symptom (Effect)

A multi-line `edit_file` on a Python file is refused whenever any changed line contains a
definition keyword immediately followed by an apostrophe:

```
edit contains a symbol definition ("class ") — use symbol tools for structural changes
```

The hint routes the caller to `edit_code`, which is the wrong tool — there is no symbol to edit,
only prose. The refusal names a keyword the caller can search for and not find, so the natural
next step is to conclude the guard is broken rather than to look for an apostrophe.

The advertised scope makes this harder, not easier. Every refusal's condition text says *"a keyword
inside a string literal or a line comment — those spans are blanked before the scan."* A reader who
believes it will not suspect blanking as the cause, because blanking is described as the thing that
prevents this.

## Reproduction

Verified 2026-09-13 against `4f268eb1`. A refusal writes nothing, so this probe is free.

```
edit_file(path="scripts/probe-cluster-census.py",
          old_string="not summarise either of the latter two; summarising a judgement is how a probe starts inventing\none.",
          new_string="not summarise either of the latter two; summarising a judgement is how a probe starts inventing\none. PROBE LINE: each class's field is the authoritative copy.")
→ edit contains a symbol definition ("class ") — use symbol tools for structural changes
```

A literal search for `class` followed by a space over that `new_string` returns nothing.

**Negative control, observed in the same session and not constructed after the fact.** A
functionally identical edit to the same docstring, containing an apostrophe (`roster's`) but no
definition keyword before one, was **accepted**. So the trigger is the conjunction — keyword
immediately followed by an apostrophe — and not the apostrophe alone. Two further refusals preceded
these, both containing a literal `class` + space, which is the *archived* shape rather than this one.

## Root cause

Two correct-in-isolation decisions compose into this.

1. `scan_line_into` (`src/util/text.rs:128-230`) treats an apostrophe as a string opener while in
   `Scan::Code`, setting `Scan::Quoted`, after which every subsequent byte is pushed to the mask as
   a space until a closing apostrophe or end of line.
2. `contains_def_keyword_at_word_start` (`src/tools/edit_file/mod.rs:62-78`) matches a needle that
   carries a **trailing space** — `def_keywords_for_lang` (`src/tools/edit_file/mod.rs:15-34`)
   gives Python three needles, each ending in a space. That trailing space is what supplies the
   right-hand word boundary.

The mask's spaces are indistinguishable from source spaces to step 2. So the possessive masks to
the bare keyword followed by spaces, satisfying a needle the unmasked text does not.

The function already knows the apostrophe reading is wrong. Its end-of-line handler says so
verbatim: a lone apostrophe *"is a lifetime or an apostrophe in prose, not a literal. Resetting
bounds the misreading to the line it appeared on."* It bounds the misreading to one line and then
hands that line's mask to a consumer that indexes it as if it were code. Bounding the blast radius
is not the same as not misreading, and the comment records only the first.

**Why the guard's own error asymmetry does not excuse it.** `contains_def_keyword_at_word_start`
argues, correctly, that a false positive costs one rejected edit while a false negative risks range
corruption, so it narrows only where unambiguous. That reasoning prices a false positive against
*code*. Here the false positive lands on *prose*, where the input is not merely rejected but
**unrepresentable**: no rewriting of the edit expresses the possessive in a multi-line change, and
the caller cannot tell why. That is `IC-6` — the defect is the input the parser admits no way to
write, which is why ordinary testing does not reach it.

### A SECOND, independent defect, measured 2026-09-14 — this file named only the first

Three probes at HEAD, all placed in the same Python module docstring. **Correction to the
Reproduction section above: "a refusal writes nothing, so this probe is free" covers only
the refusing half.** Probe A was accepted and wrote; it had to be reverted.

| probe | text | result |
|---|---|---|
| the repro above | `each class's field` | REFUSED |
| A | `each klass's field` | **accepted** — so the apostrophe alone is not the trigger |
| B | `each class of thing` | **REFUSED** |

Probe B carries a genuine `class ` with a real space and no apostrophe, so the fabrication
mechanism is not involved. It refuses because **a multi-line string is not blanked at all**:
`spanning_opener` fires on the `"""` line and blanks the rest of *that line*, then
`blank_non_code` re-enters at `Scan::Code` for every line after it.

**That is not a missing feature and it is not fixable here.** Cross-line scanning already
ships — `literal_continuation_mask` (`src/util/text.rs:292-300`) threads `Scan` state line to
line, and `spanning_opener` already recognises `"""`, `'''`, backticks and Rust raw strings.
`blank_non_code` declines to use it because its input is **the lines an edit CHANGED,
joined** — non-contiguous source, where a quote on one changed line and a quote three
changed lines later never opened a literal in the file.
`blank_non_code_does_not_carry_literal_state_between_lines` (`:733-742`) exists to red on
exactly that "optimisation", and its doc comment says it reads like a correctness
improvement and is the one change that breaks the caller. Over non-adjacent lines there is
no coherent docstring state to track.

So probe B is a **named residual**, not a bug to fix at this layer, and the refusal text now
says so.

### Which lever is safe, and why the apostrophe is not the one

`scan_line_into` has two callers and they split cleanly: `blank_non_code` (`:255`) keeps the
**mask bytes** and discards the state; `scan_line` (`:113`) keeps the **state** and discards
the mask. The state path runs `literal_continuation_mask` → `reindent_block` / `reindent_to`
→ **`edit_code`'s replace/insert reindentation**.

The mask bytes therefore have exactly one consumer in the whole project, `find_def_keyword`
(`src/tools/edit_file/mod.rs:97`). Changing the **filler** is invisible to `edit_code`.
Changing the **apostrophe arm** is not — and the `'` opener earns its keep on a case its own
comment never states. The comment defends it by naming Rust lifetimes, but lifetimes are
handled by the end-of-line *reset* (`:225-226`), not the opener. What the opener actually
prevents is a char literal containing a quote, `let q = '"';`: without it the inner `"` sets
`Scan::Quoted('"')`, end-of-line promotes it to `Scan::Spanning`, and
`literal_continuation_mask` latches for every remaining line — silently disabling
reindentation. **No test covers that.** Narrowing the apostrophe would have removed an
untested load-bearing behaviour while its comment appeared to explain what it was for.
## Workarounds

- Rephrase to avoid a definition keyword directly before an apostrophe. This is what was done, and
  it silently degrades prose to satisfy a scanner.
- Split into single-line edits — exempt from IL-2, and impractical for a paragraph.
- `run_command` with `acknowledge_risk: true`.

## Fix

**Shipped 2026-09-14 as candidate 1** — the one this file listed first, and its stated risk
(*"any other consumer of `blank_non_code` that treats the filler as whitespace… need checking
first"*) is what settled the design rather than merely clearing it. See § *Which lever is safe*.

**SHA:** `989fa34c` (on `experiments`). **patch-id:** `e3da14808a0df6174c949e024f1b3493141ad061`.

`src/util/text.rs` gains `const MASK_FILL: char = '\0'` and `push`'s non-code branch emits it
instead of `' '`. Two production lines. A needle ending in a space can now only match if that
space came from the source, so the fabrication is **unrepresentable rather than policed** —
CLAUDE.md § *Observer Blindness* position 3. The constant carries the four requirements on any
replacement: one UTF-8 byte so the per-line length contract survives; absent from every needle;
neither alphanumeric nor `_`, so a left word-boundary test still sees a boundary; and not
whitespace, so a caller's `trim_start()` filter is unchanged.

Candidate 2 (only enter the quoted state when a closing apostrophe exists on the line) was
**rejected on evidence, not on cost** — it changes the scanner, and the scanner's *state* is what
`edit_code`'s reindentation consumes. § *Which lever is safe* has the chain.

Also considered and rejected: re-verifying each match against the unmasked line at the same
offset. It works — byte length is an asserted contract — but it *polices* the fabrication and
leaves the mask able to invent bytes for the next consumer.

`src/prompts/mod.rs:641` corrected in the same commit, as this file asked: it claimed *"a keyword
inside a string literal … those spans are blanked before the scan"*, false for multi-line
strings. It now says SINGLE-LINE and names both unblanked residuals.

### Verified end to end, against the running binary

The unit and guard tests were green before this, but the tool still fabricated until a `cargo rb`
and a reconnect — the suite cannot tell you the shipped binary behaves. Re-run 2026-09-14 09:28
against a server positively identified as running the post-fix image (PID 1593923, fresh inode):

| probe | before | after |
|---|---|---|
| this file's Reproduction (`class's`) | REFUSED | **accepted** |
| A (`klass's`) | accepted | accepted |
| B (`class of thing`) | REFUSED | **still REFUSED** |

Probe B is the one that had to *not* change, and the refusal it now returns names its own cause:
*"a MULTI-LINE string such as a Python docstring — each line is scanned on its own, so only the
opening line is blanked."* A reader hitting it learns why instead of searching for a keyword that
is genuinely there and being told nothing useful. Both accepting probes write; both were reverted
and the tree confirmed clean.
## Tests added

Four, at three sites. This file asked for a two-direction pair and the shipped set keeps that
shape at every site — a fix that stopped scanning altogether would satisfy each first assertion
and fail each second.

`src/util/text.rs`

- `blank_non_code_cannot_manufacture_a_word_boundary` — the invariant stated directly: the mask
  must not contain `class ` when the source does not. **Observed RED:** the mask read
  `"each class                                  "`, which is the bug rendered. Paired with an
  assertion that everything after the apostrophe is *still* blanked, so a filler change that also
  stopped blanking would fail.
- `blank_non_code_keeps_code_and_blanks_comments_and_literals` — **rebuilt, not merely updated.**
  Two of its expectations were exact string literals spelling the filler as spaces, so they were
  pinning the FILLER BYTE — which is not what that test is about. It asserts which SPANS are
  blanked, never what they become. All expectations are now built from `MASK_FILL` via `repeat`.
  Its own note already forbade hand-counted padding; that note now has a second reason.

`src/tools/edit_file/tests.rs`

- `find_def_keyword_does_not_fabricate_a_keyword_from_a_possessive` — the unit. **Observed RED:**
  `left: Some("class ")`, the same signature the 2026-09-11 fix recorded.
- `guard_allows_a_possessive_in_prose_but_still_catches_a_python_definition` — the same defect at
  the GUARD, because that is where it was reported and a unit kill says nothing about the caller.
  **Observed RED:** the guard refused an edit it must allow. The docstring in the fixture is
  load-bearing but NOT for the obvious reason — only the `"""` opener line is blanked, so line 3
  is scanned exactly as if it were source. Move the prose onto the opener line and it asserts
  nothing.

`src/prompts/mod.rs`

- `the_il2_condition_names_both_of_its_unblanked_residuals` — a SHAPE assertion, not a prose pin.
  Pinning the sentence reds on every rewording and is rightly avoided, which is precisely how the
  text came to over-promise; so it asserts only that both residuals stay NAMED and that the
  blanking promise stays scoped to single-line literals. It buys arrival, never answerability.

**Mutation, on the production path.** `MASK_FILL` reverted to `' '` killed 3 of 3. The prompts
shape assertion killed separately by `MULTI-LINE` → `MULTILINE`. Both mutations reverted and the
files confirmed byte-identical by sha256.

**Untouched and green** — every `reindent_*` and `literal_continuation_*` test, plus
`blank_non_code_does_not_carry_literal_state_between_lines`. That is the direct evidence the
`edit_code` state path is unaffected, and it is why the filler was the safe lever.
## Resume

`N/A` — fixed, verified against the running binary, archived 2026-09-14.

One thing a later reader should not re-derive: **the docstring residual is not a bug to fix at
this layer.** Probe B still refuses by design. `blank_non_code` receives the lines an edit
CHANGED, joined — non-contiguous source — so there is no coherent multi-line string state to
track over them, and `blank_non_code_does_not_carry_literal_state_between_lines` exists to red on
any attempt. § *A SECOND, independent defect* has the reasoning. If it ever becomes worth fixing,
it needs the FILE, not the edit fragment, and that is a different design.
## References

- `docs/issues/archive/2026-09-11-edit-file-reads-a-keyword-in-prose-as-a-symbol-definition.md`
  — the predecessor, whose fix introduced the blanking pass this bug is about. Its § Fix already
  names the advertised-scope correction as owed.
- `src/util/text.rs:128-230` — `scan_line_into`, the apostrophe arm and the end-of-line comment.
- `src/tools/edit_file/mod.rs:62-113` — the needle matcher and `find_def_keyword`.
- Found while deleting the `mechanism` column from the `IC-N` roster; the blocked edit was prose
  for `scripts/probe-cluster-census.py`'s module docstring.
