---
status: open
opened: 2026-09-13
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/addressing-without-an-escape-hatch
kind: bug
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

## Workarounds

- Rephrase to avoid a definition keyword directly before an apostrophe. This is what was done, and
  it silently degrades prose to satisfy a scanner.
- Split into single-line edits — exempt from IL-2, and impractical for a paragraph.
- `run_command` with `acknowledge_risk: true`.

## Suggested fix

Not started. Two candidates, cheapest first.

1. **Do not let the mask supply a word boundary.** Blank to a byte that is *not* a space — the mask
   promises only byte-length and column preservation, never that the filler is whitespace. The bare
   keyword followed by a non-space cannot match a needle ending in a space. One-line change in
   `push`; the byte-offset contract is unaffected. Risk: any other consumer of `blank_non_code`
   that treats the filler as whitespace, so `scan_line`'s other callers need checking first.
2. **Only enter the quoted state when a closing apostrophe exists later on the line.** Closer to
   the comment's stated intent, but it changes the scanner rather than the consumer, and the
   scanner is shared with `scan_line` where the current behaviour may be load-bearing.

Whichever ships, **the refusal's advertised scope needs correcting in the same change** — it
currently promises that string-literal spans are blanked, which is exactly the mechanism that
caused this. The archived sibling asked for that correction for comments and got it; the
string-literal half of the same sentence is now the one that misleads.

## Tests added

None yet. A regression test wants both directions on one fixture, because the interesting property
is that the mask *adds* a match:

- `find_def_keyword` over a line holding the possessive must return `None` (reds today).
- `find_def_keyword` over a real Python definition must still return the keyword (must keep
  passing — this is the arm a naive fix to either candidate could disarm).

The second is not optional. A fix that stops matching the possessive by weakening the needle's
right-hand boundary would also stop matching a real definition, and only the pair discriminates.

## Resume

Nothing is in flight. Pick a candidate above, write the two-direction test first, and correct the
refusal's advertised scope in the same commit.

## References

- `docs/issues/archive/2026-09-11-edit-file-reads-a-keyword-in-prose-as-a-symbol-definition.md`
  — the predecessor, whose fix introduced the blanking pass this bug is about. Its § Fix already
  names the advertised-scope correction as owed.
- `src/util/text.rs:128-230` — `scan_line_into`, the apostrophe arm and the end-of-line comment.
- `src/tools/edit_file/mod.rs:62-113` — the needle matcher and `find_def_keyword`.
- Found while deleting the `mechanism` column from the `IC-N` roster; the blocked edit was prose
  for `scripts/probe-cluster-census.py`'s module docstring.
