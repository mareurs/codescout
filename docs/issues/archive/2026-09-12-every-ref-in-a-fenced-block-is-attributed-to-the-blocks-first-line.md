---
kind: bug
status: archived
title: 'BUG: every ref in a fenced block is attributed to the block''s first line'
tags:
- cluster/attribute-derived-at-container-granularity
closed: null
opened: 2026-09-12
owner: marius
related: []
severity: medium
---

# BUG: `audit_doc_refs` reads a backtick INSIDE a fenced block as an inline-code delimiter

## Summary

**The title is falsified and is kept only because `IC-6`'s `**Members:**` line cites this
slug.** A backtick has nothing to do with it. Read this section, not the headline.

Every reference inside a fenced code block is attributed to the block's **first content
line**, not to the line it sits on. `parse_refs` (`src/librarian/tools/audit_doc_refs/parser.rs`)
walks pulldown-cmark events and takes `md_line` from `byte_offset_to_line(text, span.start)`
— the span's **start**. pulldown-cmark hands a fenced block's entire body over as a single
`Event::Text`, so that start is the block's first content line for every token in it.

The drift is `(ref_line - block_start)`: **unbounded, not an off-by-one.** A 60-line fenced
payload with a ref at the bottom is off by 59.

The second filed symptom — a lost `code_block` severity cap — **is not a defect**; see
§ Symptom.
## Symptom (Effect)

1. **Line attribution is wrong for every ref below a fence's first content line.** A reader
   following the finding to the line it names finds something else there. Magnitude grows
   with the block's height.

2. ~~Severity cap lost~~ — **withdrawn, not a defect.** `cap_code_block`
   (`src/librarian/tools/audit_doc_refs/severity.rs`) fires only when the verdict is
   `Missing | FileMissing | SymbolMissing` **and** severity is already `High`. The observed
   finding was `verdict: unknown`, `severity: low`, so `policy_default` was the correct
   reason and no cap was lost. `resolver.rs`'s `in_block` assertion already pins the real
   path as `Missing -> Med`, so the alarming case this file originally described — a
   path-shaped token inside a fence reaching `high` and redding CI — does not occur.
   This file's own § Hypotheses tried predicted this outcome and marked it untested.

**Two further consequences, NEITHER DEMONSTRATED, recorded with their preconditions so a
reader does not mistake them for observations:**

- **A severity band can flip.** `md_line` feeds `cap_released_history` (`mod.rs`), which
  tests `md_line >= boundary`. Drift is always **downward**, so a fence straddling a
  CHANGELOG's released-history boundary reports its refs below the boundary, loses a cap it
  earned, and stays `High` — redding CI on a historical reference that is unfixable by
  design, which is the exact failure that cap exists to prevent. `released_history_boundary`
  is a naive `text.lines()` scan and is **not** fence-aware, so the boundary can itself land
  inside a fence. Precondition: `CHANGELOG.md` plus a straddling fence.
- **A dedup can fail open.** `scan_code_comments` dedups `parse_prose_refs` against
  `parse_refs` on the key `(md_line, raw_ref)`. `parse_prose_refs` numbers lines correctly
  and `parse_refs` did not, so for a ref inside an indented block within a doc comment the
  two keys disagree and the same ref is reported **twice**. Precondition: a code comment
  containing an indented or fenced block — common in Rust doc comments.
## Reproduction

Tree `408709ea`. Scanning `docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`,
whose fence opens at line 83 and closes at 86, with line 84 holding a raw-string regex
that contains a literal backtick inside a character class.

```
librarian(action="audit_doc_refs",
          paths=["docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md"],
          emit_tracker=false)
```

**The control is what makes this a finding and not a miscount.** Three refs in one file,
two outside any fence and one inside the backtick-bearing fence:

| ref | actual line | reported `md_line` | in a fence? | severity_reason |
|---|---|---|---|---|
| `claimed.difference` | 33 | 33 | no | `policy_default` |
| `claimed.difference` | 39 | 39 | no | `policy_default` |
| `re.captures_iter` | **85** | **84** | **yes** | `policy_default` |

The two unfenced refs are attributed exactly, so the scanner is 1-indexed and correct in
general. Only the fenced one drifts, and only that fence contains a backtick. The same
finding should have read `code_block` and did not.

## Environment

- Tree `408709ea` on `experiments`.
- `librarian(action="audit_doc_refs")`, run against the live release binary built
  2026-09-12 20:06.

## Root cause

**Confirmed in code and by controlled probe**, superseding the black-box hypothesis this
section previously carried.

`parse_refs` computes one line per **event**:

```rust
let line = byte_offset_to_line(text, span.start);
```

and stamps it on every candidate found in that event. For `Event::Code` (an inline span)
that is correct — a code span is one line. For `Event::Text` inside a code block it is not:
pulldown-cmark emits the block's whole body as ONE event, so `span.start` is the first
content line and every token in the block inherits it.

**There is no backtick counter to desynchronise.** Fence state belongs to pulldown-cmark;
`parse_refs` only mirrors it into a bool (`in_code_block`) to choose a `RefPosition`. The
filed hypothesis described a scanner this code is not.

**Why the reporter saw ±1.** Both fences in the sampled file are two lines tall, where
"attribute to the block's first line" and "subtract one" give the same answer. CLAUDE.md
§ *Testing Discipline*: a count must arrive with its unit — the magnitude was a property of
the sample, not of the defect.
## Evidence

Table above, re-derivable from the single `audit_doc_refs` call. `grep -n` for each token
gives the actual lines.

## Hypotheses tried

- **"The scanner is 0-indexed"** — **rejected by the control**, correctly. Two refs in the
  same file report their exact 1-indexed lines; a uniform off-by-one would have shifted all
  three.
- **"`module_path` refs are never capped `code_block`"** — **CONFIRMED.** Filed as "not
  tested"; now tested. `cap_code_block`'s guard is
  `Missing|FileMissing|SymbolMissing && sev == High`, which a `module_path`/`unknown`/`low`
  finding cannot satisfy. This is what dissolves Symptom 2, and it was the entry's own
  second hypothesis — the file predicted its own correction and shipped the alarming reading
  anyway.
- **"A literal backtick inside the fence desynchronises an inline-code counter"** —
  **FALSIFIED.** A fence containing no backtick at all drifts identically:
  `"intro\n\n```\ndocs/a.md\ndocs/b.md\n```\n"` reports `[(4, a), (4, b)]`.
  **The falsifier was already inside this file's own § Reproduction:** the second fence
  (lines 48-52) holds no backtick and its refs on lines **49 and 50** both report 49. The
  original table enumerated three refs and stopped one short of the pair that separates the
  two explanations. Pinned by `a_backtick_inside_a_fence_does_not_change_attribution` so the
  dead hypothesis cannot be re-implemented as a fix.
## Fix

**FIXED** on `experiments` — `5d6b87b4`, patch-id `98a832004d2114382fb66ddfec81c8788b770606`.

`tokenize_code_span` (`src/librarian/tools/audit_doc_refs/parser.rs`) now yields
`(newlines_before_token, token)` instead of a bare token, and both `parse_refs` arms stamp
`md_line: line + row`. The separator predicate is extracted as `is_code_span_sep` so it can
be handed to `str::find` in both polarities.

**Why the offset is returned rather than computed by the caller.** A caller that forgets it
compiles clean and is wrong — exactly the defect being fixed. Returning it makes the
omission a type error at the two sites that need it, and the doc comment says so, because
the next person to add an event arm is the party who cannot see this.

**Two facts make the offset exact rather than approximate, and both are stated at the
function:** a token can never *contain* a newline, because `\n` is whitespace and therefore
a separator — which is why a token has a line and not a line range; and `trim_token_edges`
strips only `[]{}` and a trailing `.`, so trimming cannot move a token across a line.

`parse_prose_refs` is the deliberate exception: it feeds one line at a time, so it always
receives `0` and discards it with a comment saying why.

**Not changed, and the restraint is the point.** `released_history_boundary`'s
fence-unawareness and `scan_code_comments`' dedup key are both reachable from this defect
(§ Symptom) but neither is demonstrated, and neither is this bug. Widening the fix to
surfaces whose failure has not been observed is how a fix acquires unreviewed blast radius.
The first is filed separately so it survives this file's archival.
## Tests added

Four, in `parser.rs`'s `tests` module. All four were observed RED against the pre-fix code
and GREEN after — the production path was mutated, not the test's inputs.

| test | what it pins | what kills it |
|---|---|---|
| `a_fenced_block_ref_reports_its_own_line_not_the_blocks_first` | the core claim | reverting the `+ row` |
| `fenced_line_drift_grows_with_the_block_it_is_not_an_off_by_one` | the **magnitude** | a `line + 1` "fix", which answers 5 where the truth is 44 |
| `a_backtick_inside_a_fence_does_not_change_attribution` | the falsified hypothesis stays falsified | re-implementing a fence-suspended backtick counter |
| `an_inline_code_span_folds_its_newline_so_the_row_offset_is_inert_there` | an **inert** arm, labelled inert | pulldown-cmark ceasing to fold — which is the notice that arm went live |

**The magnitude test exists because the first one cannot do its job alone.** Across a
two-line fence, "attribute to the block's first line" and "subtract one" are the same
answer, which is precisely how this bug came to be filed as an off-by-one. Its 40 filler
lines are load-bearing: shrink them and the test keeps passing while no longer
discriminating a real fix from an increment.

**The backtick control is a negative fixture and is annotated as one.** It asserts that a
backtick changes *nothing*, so it is monotone under the wrong kind of repair — its value is
that it fails loudly if someone reintroduces backtick-sensitivity while chasing a drift
report, not that it proves anything on its own.

**The inert fixture is annotated as inert, per CLAUDE.md § Testing Discipline.** The
`Event::Code` arm takes the same `+ row` offset, but pulldown-cmark normalises a newline
inside an inline code span to a space, so `row` is observably always 0 there and that arm's
offset is never exercised. Measured, not assumed. It is kept as a tripwire rather than
deleted: if the fold ever stops, the red is the only thing that would announce the arm had
become live.
## Workarounds

None needed at observed severity. Do not "fix" a `high` finding on a correctly-fenced
example by unfencing or rewording it — check first whether the fence contains a backtick.

## Resume

Filed 2026-09-12 from a black-box `audit_doc_refs` run, with the root cause explicitly
marked unconfirmed — which is why the correction cost a day rather than a week.

**The disconfirming case was inside the reproduction the whole time.** § Reproduction
tabulates three refs and calls itself "the control that makes this a finding and not a
miscount", which it is. The same file's second fence holds no backtick and reports two refs
at one line; the table stops one row short of it. The control was sound and *also* contained
its own disconfirmation, and reading stopped where it confirmed.

**The entry predicted its own correction and shipped the other reading anyway.**
§ Hypotheses tried names "`module_path` refs are never capped `code_block`" and marks it
untested, while § Symptom calls the cap loss "the one with teeth" and sets the file's
severity. An untested hypothesis that would dissolve the symptom was recorded *beside* the
symptom without gating it.

**Cluster: the `IC-6` tag is a MISFIT and is retained only pending adjudication.** It was
assigned on the falsified backtick diagnosis. IC-6's claim is *"an addressing scheme
interprets every token in its namespace and provides no way to write one literally, or to
disambiguate two that collide — the defect is the input it makes unrepresentable."* Nothing
here is unrepresentable: line numbers are an adequate address space, and the fix added
neither an escape nor a disambiguator, which is IC-6's remedy test in practice. `IC-21`
(instrument reports a count where magnitude decides) and `IC-13` (a capped result presented
as complete) were checked against their claims and fail too. The tag is left in place
because "exactly one reserved tag" is a documented invariant and withdrawing without a
destination breaks it; the argument is recorded here and on IC-6's `**Members:**` line so a
reader arriving by query meets it immediately rather than counting this as a member.
## References

- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*, including
  the four-independent-shell-gates heredoc precedent.
- `docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`.
