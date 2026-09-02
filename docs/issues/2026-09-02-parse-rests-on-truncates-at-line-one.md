---
id: e4b308dae72e5da1
kind: bug
status: open
title: 'BUG: parse_rests_on returns line 1 only, silently truncating 70% of declarations mid-value'
tags:
- cluster/capped-result-presented-as-complete
- librarian
- statements
- rests-on
- parser
- doc-vs-code-drift
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
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

**Not yet applied.** Extend `parse_rests_on` to consume the declaration's full
paragraph: from the `**Rests on:**` line through the last non-blank line before
the next blank line, heading, or fence.

Three constraints the change must respect:

- **Do not apply this to `parse_validity`.** `**Valid:**` has a closed grammar
  (`invariant` / `dated <ISO>` / `conditional — <event>`) and its own tests
  require an em-dash tail to be rejected after `dated`. Paragraph-consuming it
  would change what parses.
- **Fence-skipping must survive** — `first_declaration_line` delegates to
  `FenceState` precisely because a worked example teaching the syntax is
  extracted identically to a real declaration.
- **Column-0 strictness on the *first* line only.** An indented `**Rests on:**`
  is prose under a list item and must still not declare; continuation lines are
  expected to be indented.

## Tests added

None yet. The regression test must assert the **captured value**, not its
presence — a two-line declaration whose target lives on line 2, asserted byte
for byte. Every existing test passes against the truncating parser.

## Workarounds

Put the whole `**Rests on:**` value on one line where the target matters. The
rendered `librarian(action="context")` bundle is the only consumer today, so
the loss is currently confined to that surface.

## Resume

Read `src/librarian/statements.rs:41-44` (`rests_re`), `:79-94`
(`first_declaration_line`) and `:222` (`parse_rests_on`). Add a
paragraph-consuming variant used only by `parse_rests_on`; leave
`parse_validity` on the single-line path. Then re-run the classification in
*Evidence* and confirm resolvable targets move 47% → ~59%.

## References

- `src/librarian/statements.rs:41-44`, `:79-94`, `:222`, tests at `:500-517`
- `src/librarian/tools/context.rs:427` — the only consumer today
- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md:534-535` — the two-line example
- `docs/trackers/bug-fix-session-log.md` `W-57` — a live truncated value
- Related: `rests-on` materialises **zero** edges today (`SELECT ... FROM artifact_link WHERE rel LIKE '%rest%'` → 0); this parser is upstream of any decision to build that edge

