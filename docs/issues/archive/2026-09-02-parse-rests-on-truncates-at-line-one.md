---
id: 02f14945cec9a691
kind: bug
status: fixed
title: 'BUG: parse_rests_on returns line 1 only, silently truncating 70% of declarations mid-value'
tags:
- cluster/capped-result-presented-as-complete
- librarian
- statements
- rests-on
- parser
- doc-vs-code-drift
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: medium
unverified: the 47%->59% resolvability projection in Evidence was not re-derived post-fix; only capture correctness (~70% of declarations) was measured
---

# BUG: `parse_rests_on` returns line 1 only, silently truncating 70% of declarations mid-value

## Summary

`**Rests on:**` is the Statement field naming the durable route back to a
claim's proof. `parse_rests_on` is line-anchored and returns the **first line
only**. Measured across the corpus, **145 of 208 declarations (70%) are
hard-wrapped onto a following line**, so the parsed value is a fragment
presented as the whole field — sometimes cut mid-title. The design spec's own
worked example is a shape the shipped parser truncates.

## Symptom (Effect)

`bug-fix-session-log:W-57` declares a multi-line `**Rests on:**`. The parser
captures:

```
`docs/RELEASE.md` § *Before cherry-pick: read the live output of any `
```

— ending mid-title, mid-backtick. `src/librarian/tools/context.rs:427` then
renders that fragment into the `librarian(action="context")` bundle as if it
were the declaration:

```rust
.map(|r| format!("\n**Rests on:** {r}"))
```

No error, no truncation marker. A reader of the bundle cannot tell a complete
value from a severed one.

## Root cause

`src/librarian/statements.rs:41-44` — `rests_re` is
`^\*\*Rests on:\*\*[ \t]+(.+?)[ \t]*$`, anchored to one line. `parse_rests_on`
(`:222`) returns that single capture via `first_declaration_line`, which by
design returns **the first matching line** and does not continue into the
paragraph.

The regex is correct for a one-line declaration. The corpus is not one-line.

**measured 2026-09-02** over every entry section in `docs/**/*.md`, using a
mirror of the shipped chain (`FenceState`, `headings::parse`, `entry_sections`,
`declared_section_text`, `parse_validity`, `parse_rests_on`) validated against
`librarian(action="doctor")` on 6/6 classified entries and against 44 committed
`entry_high_water_<PREFIX>` counters (40 exact, 4 in the safe direction, 0
over-counts):

| | |
|---|---|
| entries declaring `**Rests on:**` | **208** |
| of those, followed by a non-blank continuation line | **145 (70%)** |
| values with **no** resolvable target on line 1, but one below it | **25** |

## Evidence

### The truncation loses resolvable targets, not just prose

Reclassifying the full paragraph instead of line 1, over all 208:

| target class | line 1 (shipped) | full paragraph |
|---|---|---|
| no resolvable target | 110 | **85** |
| in-repo path | 37 | **54** |
| entry token | 56 | **64** |
| `path:line` | 9 | **20** |
| ADR path | 3 | **5** |

Resolvability moves from **47%** to **59%** — across the threshold at which a
derived `rests-on` edge is worth materialising at all.

### The spec's own example is a shape the parser cannot read

`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md:534-535`
shows a two-line hanging-indent `**Rests on:**`. The document that specifies the
field demonstrates a form the implementation truncates. Doc-vs-code drift, and
the doc is the older artifact.

### Why nothing caught it

`parse_rests_on` has passing tests (`statements.rs:500-517`): fenced-block
rejection, line-anchoring, absence, and a prose mention. Every one uses a
**single-line** input. The tests assert the field is found; none asserts what is
found is complete. `tests-that-cannot-fail`, case 2 — *asserts a subset while
its name claims the whole*.

## Hypotheses tried

1. **Hypothesis:** authors are expected to keep `Rests on:` on one line, so the
   70% are malformed.
   **Test:** read the field's design spec.
   **Verdict:** rejected — the spec's own worked example is two lines with a
   hanging indent.

## Fix

**Applied 2026-09-02.** `experiments` `1b071cd7`, patch-id
`9d0f25f5581c517c4b5ff663fea05d0858f855f0`.

`first_declaration_paragraph` (`src/librarian/statements.rs`) consumes the
declaration through its paragraph; `parse_rests_on` switches to it.
`parse_validity` deliberately does **not** — its grammar is closed and its tests
require a trailing em-dash after `dated` to be rejected.

Four stop conditions: blank line, heading, fence delimiter, and **the next bold
field label**. The last was not in this file's original plan and is the one that
mattered: entry sections put `**Valid:**` and `**Status:**` on lines *adjacent*
to `**Rests on:**` with no blank between, so without it the validity class is
swallowed into the rests-on value and `parse_validity` is silently asked about
different text. Measured cost of the stricter rule: **7 of 236** declarations
stop at line 1 that a maximally-permissive paragraph model would have consumed
— and each of those 7 is a case where consuming further corrupts a sibling
field.

**Measured effect, and the part that is NOT established.** Capture improves for
**~70%** of declarations — 166 of 236 by an upper-bound count post-fix, 145 of
208 by the independent fence-aware count that opened this file. Two instruments,
same rate. The **47% → 59% resolvability** figure quoted in *Evidence* is a
different measure — whether a captured value *contains* a resolvable target —
and was **not** re-derived after the fix. Do not cite it as a post-fix result.

**Out of scope, still open:** the `rests-on` **edge** does not exist.
`artifact_link` has zero rows with that rel and no code creates one. This fix
makes the edge worth building; it does not build it.
## Tests added

Four, in `src/librarian/statements.rs`'s `tests` module:

- `rests_on_captures_a_hard_wrapped_continuation` — synthetic, **indented**
  continuation (the hanging-indent form the design spec shows).
- `rests_on_captures_the_real_corpus_declaration_that_was_being_truncated` —
  three lines of **verbatim** corpus bytes from `bug-fix-session-log` `W-57`,
  which wrap at **column 0**.
- `rests_on_stops_at_the_next_bold_field` — asserts both that the value stops
  *and* that `parse_validity` still sees its own field unconsumed.
- `rests_on_stops_at_a_blank_line_heading_or_fence`.

**The two capture fixtures disagree on purpose, and that is load-bearing.** A
continuation rule requiring indentation passes the synthetic one and still
truncates the real corpus — which is this defect. Annotated on the fixtures so a
later tidy-up cannot silently remove the discrimination.

**Mutation-verified, one site each:**

| mutation | reds |
|---|---|
| revert `parse_rests_on` to `first_declaration_line` | exactly the 2 capture tests |
| drop `\|\| is_bold_field_label(line)` | exactly `rests_on_stops_at_the_next_bold_field` |

Note the 3 stop-condition tests were **inert** before this change — line-1
parsing stops at everything, so they passed vacuously. They become
discriminating only with the paragraph consumer, and they guard the fix
**over-reaching**, not the original defect.
## Workarounds

Put the whole `**Rests on:**` value on one line where the target matters. The
rendered `librarian(action="context")` bundle is the only consumer today, so
the loss is currently confined to that surface.

## Resume

N/A for the parser.

The follow-on, tracked separately: materialise the `rests-on` edge. 208
declarations exist, `artifact_link` holds **zero** rows with that rel, and
`link_scan`'s resolver already handles the target forms that dominate — entry
tokens (56 bare + 21 qualified) and in-repo paths (37, rising to ~54 now that
continuations are captured). Note **0** sampled values name a commit SHA and
only 3 of 208 name an ADR path, so an ADR-or-nothing resolver would find almost
nothing. Build it additive, with a reported-not-resolved bucket, exactly as
`link_scan` already treats cross-repo citations.
## References

- `src/librarian/statements.rs:41-44`, `:79-94`, `:222`, tests at `:500-517`
- `src/librarian/tools/context.rs:427` — the only consumer today
- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md:534-535` — the two-line example
- `docs/trackers/bug-fix-session-log.md` `W-57` — a live truncated value
- Related: `rests-on` materialises **zero** edges today (`SELECT ... FROM artifact_link WHERE rel LIKE '%rest%'` → 0); this parser is upstream of any decision to build that edge
