---
id: bc79a20e28c9ad1b
kind: bug
status: open
title: 'BUG: a scoped audit-doc-refs:ignore-refs marker suppresses its entire section, so 73 refs in PROBES.md are unguarded and the scan still exits 0'
tags:
- cluster/selector-narrower-than-its-population
---

## Summary

A scoped `<!-- audit-doc-refs:ignore-refs \`a\` \`b\` -->` marker in `docs/PROBES.md` is suppressing
**every** reference in its section, not the tokens it names. Measured 2026-09-15 at `HEAD`:
`librarian(action="audit_doc_refs", paths=["docs/PROBES.md"])` reports **36 refs found** and **not
one** of them lies between the marker (`:163`) and the next heading (`:202`) — a span that holds
**73** path-shaped backticked tokens.

The marker's author anticipated this exact outcome and wrote six lines of prose to avoid it:

> Scoped by token, not by section, because this section carries **27 real refs** and a bare
> `audit-doc-refs:ignore` would silence every one of them — in the document whose whole job is
> telling a reader which instrument to trust.

`parser.rs` agrees, at the type level: *"Kept separate from [`blocks`] so the `Only` case still walks
its tokens: collapsing the two would make a scoped marker behave like a bare one."* That is the
observed behaviour.

## Symptom (Effect)

`audit_doc_refs` returns `exit_code: 0` on `docs/PROBES.md` having examined none of the section's
refs. A stale path there is unguarded and **cannot red** — including in the `Audit Doc Refs` CI job,
which reads the same scan.

**Two counts, two units, deliberately not reconciled.** 73 is my count of path-shaped backticked
tokens in `:163–:201` by grep, 2026-09-15. 27 is `parser.rs`'s own figure for "real refs" in that
section, written 2026-09-02. Different methods, different dates, and the section has grown; the
magnitude is *dozens*, and either number quoted alone would be a false precision.

## Reproduction

```
librarian(action="audit_doc_refs", paths=["docs/PROBES.md"], emit_tracker=false)
```

Read `findings[].md_line`: every value is `< 163` or `> 201`. Control that this is suppression and
not an empty span: `sed -n '163,201p' docs/PROBES.md | grep -o '`[A-Za-z0-9_./-]*\.\(rs\|sh\|py\|md\)`' | wc -l`
→ **73**.

## Environment

codescout `experiments` at `8289a448`. Marker at `docs/PROBES.md`:163, section `## Standalone
scripts` (`:162`) to `## Built-in \`librarian\` scans` (`:202`).

## Root cause

**NOT ESTABLISHED, and two plausible causes are already falsified — recorded so the next reader does
not re-run them.**

1. **Line length — FALSIFIED.** Lines 175/180/185/186/201 (4441–7704 chars) are unreported and line
   208 (2453) is reported, which looked like a cap. It is not: 208 sits *past the section boundary*.
   Every unreported line is inside `:163–:201` and every reported one is outside. The correlation
   with length is an artefact of the longest cells happening to live in that section.
2. **Prefix collision — FALSIFIED.** `is_ignore_marker` is a `contains` of `"audit-doc-refs:ignore"`,
   which `"audit-doc-refs:ignore-refs"` also contains — so the scoped form looked like it might be
   swallowed by the bare one. It is not: `parse_ignore_marker` tests
   `html.contains("audit-doc-refs:ignore-refs")` **before** falling back to `Suppression::All`. The
   grammar disambiguates correctly.

What is established is the **behaviour**: the suppression in force over that span is `All`, not
`Only`. Which path produces it is the open question.

## Evidence

**An over-capture that is real but does NOT explain this.** `backtick_re` collects every backticked
token in the marker body, and the body carries prose. Declared targets are 4 — `src/serve`,
`src/lsp/m`, `args`, `audit-doc-refs:ignore` — where the author intended 2; the last two are quoted
*in the explanation*. `parser.rs`'s own doc comment says backticks were chosen as the delimiter
*"because the marker body also carries prose, and a whitespace split would read the explanation as
targets"* — and backticked prose is read as targets anyway, which is that reasoning holding against
the parser that states it. But `Only([4 targets])` still blocks 4 refs, not 73, so this is a
separate small defect and not the cause.

**The blast radius includes this bug's own neighbours.** Twenty minutes before this file was
written, `audit_doc_refs` was used to verify an edit to `docs/PROBES.md`:180 — inside the suppressed
span. It returned `exit_code: 0` and that zero established nothing. The edit was verified by `ls` on
the link target instead, and only because the silence looked wrong.

## Hypotheses tried

`IC-6` (`addressing-without-an-escape-hatch`) was the first guess, on the prefix collision, and is
**rejected** — see Root cause (2). The grammar owes a disambiguator and has one.

## Fix

Not designed; the cause is not known. **Do not "fix" this by deleting the marker from
`docs/PROBES.md`** — that unguards two genuine false positives the author correctly annotated, and
it would hide the defect rather than close it.

Two things are worth doing independently of the diagnosis:

- **A regression test at the behaviour, not the parse.** `Suppression::blocks` and
  `blocks_everything` are already unit-correct by inspection; the failure is downstream of them, so
  a test on those two would pass and prove nothing. Assert on a whole-file scan: a fixture with a
  scoped marker and three refs, one named, must report **two**.
- **Narrow the target capture** to the run of backticked tokens *before* the first prose word, so a
  marker's explanation cannot contribute targets.

## Resume

Start at the caller of `parse_ignore_marker`, not at `parse_ignore_marker` itself — it is correct in
isolation. The question is how a `Suppression::Only` in force over a section ends up blocking every
ref in it: whether the value is recomputed per event, whether a later comment in the span re-parses
to `All`, or whether `blocks_everything()` is consulted where `blocks(raw)` was meant.

## References

- `src/librarian/tools/audit_doc_refs/parser.rs` — `parse_ignore_marker`, `Suppression`,
  `is_ignore_marker`, `backtick_re`
- `docs/PROBES.md`:163 — the marker, and its author's reasoning for the scoped form
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the principle: a zero must name the
  scope it examined
- `docs/issues/2026-09-15-probes-recommends-strings-to-settle-a-binarys-contents-and-a-miss-proves-nothing.md`
  — the sibling filed today, same shape one layer out: an instrument whose miss is read as absence
