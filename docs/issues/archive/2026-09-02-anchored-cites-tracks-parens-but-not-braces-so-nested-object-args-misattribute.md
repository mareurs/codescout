---
id: 2daa46427c757702
kind: bug
status: fixed
title: 'BUG: anchored_cites tracks parens but not braces, so a nested object''s key=value is billed to the outer call'
tags:
- cluster/addressing-without-an-escape-hatch
---

## Summary

`anchored_cites()` in `tests/doc_tool_refs.rs` walks `(`/`)` depth to find a call's argument span
but does **not** track `{`/`}`. So a `key=value` pair inside a nested object-literal argument is
billed as a top-level named argument of the *outer* call. The corpus cannot express a legal syntax
without the scanner mis-reading it, and there is no escape at the refusal site.

**Latent, and the measurement is the point:** 130 surfaces, 618 anchored citations, **0 at
brace/bracket depth > 0**. No document is mis-parsed today, because the corpus uniformly uses `:`
inside `{}`. This is filed so the next person to write `=` inside a nested object gets a record
instead of a mystery.

## Symptom (Effect)

A documented call of the form

```
doc(action="augment", id="...", augment={prompt=..., params={...}})
```

is scanned as though `prompt` were a named argument of `doc(`. The parameter-existence guard then
reports `doc` has no parameter `prompt` — a **false RED** naming a real tool and a real-looking
parameter, with nothing pointing at the nesting as the cause.

## Reproduction

Observed 2026-09-02 during Task 5 of the tool-surface-collapse plan. `src/prompts/guides/tracker-conventions.md:374`
used `=` rather than `:` inside a nested `augment={...}` example — the only place in the corpus that
did — and `a_documented_tool_parameter_exists_on_that_tool` failed on a bogus `doc(prompt=…)`
citation.

The change that made it green was to the **documentation** (`prompt=` → `prompt:`), not the scanner.

To re-create: put `=` inside a nested object literal in any scanned surface and run
`cargo test --test doc_tool_refs`.

## Environment

Branch `tool-collapse` at `5da2537d`. `tests/doc_tool_refs.rs` is shared with `experiments`; the
scanner behaviour is not branch-specific.

## Root cause

`anchored_cites()` (`tests/doc_tool_refs.rs` ~`:305-330`) extracts a call's argument span by
counting parentheses only. A `{`-delimited object literal appearing as an argument value is
therefore transparent to it, and every `key=value` inside that object is attributed to the enclosing
call.

Measured 2026-09-02 by the Opus task review of `5da2537d`, which replicated the scanner exactly and
bucketed all 618 anchored citations by brace/bracket depth — **not** inferred from reading:

```
130 surfaces scanned
618 anchored citations
  0 at brace/bracket depth > 0
```

**A second latent case in the same function:** quoted-span blanking happens *after* paren
extraction, so a `(` or `)` inside a string literal is mis-counted the same way. Not currently
triggered; same fix location.

## Evidence

### Why the documentation fix was independently correct

`prompt:` is the JSON-ish house style used everywhere else in the corpus, so the edit that silenced
the scanner was the right edit on its own merits. That is what makes this worth a record rather than
a revert: **the workaround was indistinguishable from good practice**, so nothing marked that a
parser limitation had just dictated a prose convention.

### Why it is `IC-6` and not merely a bug

The class is *a parser over a namespace with no escape hatch*. The nested object literal is legal,
meaningful, and unrepresentable to this scanner; the corpus must avoid it to keep the gate quiet.
No test can be written for the case, because the case cannot be expressed. That is the class's
signature, and it is why ordinary testing does not reach it.

### The failure direction is the favourable one

Unlike most `IC-6` members, this one fails **loud**: a false RED, not a silent mis-attribution. A
reader gets a confusing message rather than nothing. That is why this is filed at low severity
despite the class being the corpus's largest.

## Hypotheses tried

1. **Hypothesis:** other nested-object examples are mis-parsed today and merely unnoticed.
   **Test:** replicated the scanner and bucketed all 618 anchored citations by brace depth.
   **Verdict:** rejected — 0 at depth > 0.
   **Evidence:** § Root cause.

2. **Hypothesis:** the `=` vs `:` convention is enforced somewhere, so the corpus cannot regress.
   **Test:** looked for a gate on nested-object syntax in scanned surfaces.
   **Verdict:** rejected — the uniformity is convention only. Nothing prevents the next author from
   writing `=`, and the diagnostic they receive will not mention nesting.
   **Evidence:** the Task 5 incident is that author.

## Fix

Done, and in four parts rather than the two this section planned.

**1. The span walker counts `{}`/`[]` alongside `()`.** As prescribed. A `key=value` at nesting
depth > 0 is a field of a literal and is billed to nothing.

**2. The per-line core was extracted into a pure `calls_on_line(&str)`, and that is part of the
fix rather than tidying around it.** This class's signature is *"no test can be written, because
the case cannot be expressed"* — and that was true here in a second, concrete way this file did
not name: the only entry point walked the filesystem, so a fixture had nowhere to live except
the corpus the gate scans. Planting one there makes the gate's own input a test fixture. That is
why an `IC-6` member sat filed-but-untested; the extraction is what made the reproduction
expressible at all.

**3. Quote-awareness was added to the WALK, not by blanking the line first as prescribed.**
Blanking first would apply `'[^']*'` to prose, where an apostrophe (*"the tool's
`doc(action=…)`"*) opens a span that can swallow a real call. The walk now ignores delimiters
inside double-quoted strings only; single quotes are deliberately untracked and that limitation
is stated in the gate's own failure text.

**4. The brace fix SILENTLY REMOVED a catch, and that had to be paid for.** `present_tense_surfaces`'
own doc comment records this gate catching `augment={prompt: ..., params=...)` in
`augmented-artifacts.md` — a malformed call whose unclosed brace made the paren-only walker read
`params=` as a top-level argument. Counting braces puts that field at depth 1, so the malformed
document would now pass unremarked. Rather than accept the trade, `LineCall::unclosed` records it
and `a_documented_call_closes_its_own_literals` reports it — which is this class's standing remedy
(*say so at the refusal site*) in executable form.

**The corpus refined that detector, and the first cut was wrong.** Flagging `nest > 0` fired on
**8 correct documents**, every one a call whose arguments WRAP across lines: the scan is per-line,
so such a line ends with its paren and its literal both open. Requiring the paren to have CLOSED
separates malformed from merely wrapped — a wrapped call never reaches `)`. Wrapped calls are a
silent-MISS shape, not a false-RED one, and are named in the failure text as out of scope here.

**A performance regression was introduced and caught by hand, not by any assertion.** The first
cut of the extraction compiled three regexes per LINE, taking the suite from 0.32s to **133s**.
Every test still passed — a 400x slowdown is not a wrong answer. Fixed with `OnceLock`; back to
0.33s.

Fix SHA: `70ae3a4e`
Patch-id: `ba7219d80708453bd694b9b8a2768a8501b147d7`

## Tests added

Six, all in `tests/doc_tool_refs.rs`, and all against the extracted `calls_on_line` — which is
what made any of them possible (§ Fix, part 2).

- `a_named_argument_inside_a_nested_object_is_not_billed_to_the_outer_call` — the reproduction.
  Asserts **equality** on the parameter list, not `contains`: extra names are the filed defect,
  missing names are an over-suppressing walker dropping real citations, and both turn this gate
  green for the wrong reason.
- `a_key_whose_value_is_a_literal_is_still_billed_to_the_call` — the over-match guard, and not a
  hypothetical one: `read_file(path, headings=[...])` is the exact shape of a live violation
  `CALL_OPEN`'s doc comment records this gate catching. A walker suppressing a `key=` for merely
  sitting near a literal would lose it and look like a clean fix.
- `an_unbalanced_delimiter_inside_a_string_does_not_swallow_the_rest_of_the_span` — the second
  latent case. Failure direction is the opposite of the headline bug: a silent loss, not a false
  RED.
- `the_walker_still_separates_neighbouring_calls_and_ignores_quoted_equals` — pins the two
  behaviours the walker already had, so the depth change cannot quietly cost either.
- `a_documented_call_closes_its_own_literals` — the malformed-call detector that replaces the
  catch the brace fix removed.
- `the_unclosed_flag_fires_on_the_malformed_form_and_not_the_correct_one` — its non-vacuity, in
  three arms: malformed must fire, well-formed must not, **and a WRAPPED call must not**. The
  third arm is the one the corpus taught; without it the `closed &&` half can be deleted with the
  other two still green.

**Mutation-tested once per guarded behaviour — four observed REDs:**

1. `{}`/`[]` depth removed → the reproduction reds with
   `["action", "id", "augment", "prompt", "params", "inner"]`, which is the filed defect verbatim.
2. Quote-awareness removed → exactly one test reds; `glob` is swallowed by a `[` inside a string.
3. `closed &&` dropped from the `unclosed` condition → the wrapped arm reds AND the corpus gate
   reds on 8 correct documents.
4. The detector disabled entirely → the malformed arm reds.

**Acceptance was a DIFF, not a count** (§ Resume): all 713 corpus citations byte-identical before
and after, re-measured separately across the brace fix, the perf fix and the quote-awareness
change. The count assertion this file originally prescribed would have compared against a stale
618.

## Workarounds

Use `:` inside `{}` in any scanned surface. This is the house style anyway, so the workaround costs
nothing — which is precisely why the limitation went unrecorded.

## Resume

Fixed and archived — nothing to resume.

**One correction for anyone re-deriving this, because this section's own instruction was a
trap.** It said to *"confirm the citation count is still 618"*. The measured count on
`experiments` at 2026-09-12 is **713**, and the test's own module header says **313** measured
2026-09-01. Three numbers, one population, all honestly derived at different instants —
`CLAUDE.md` § *Testing Discipline*'s "a count must arrive with its instant and its tree" in its
purest form. A re-deriver following the literal instruction would have hunted a 95-citation
phantom.

What the acceptance actually needs is a DIFF, not a count: a walker that drops three real
citations and gains three phantoms holds any total steady. The method used here, and worth
reusing — a temporary test dumping `file:line\ttool\tparam` sorted, run before and after, and
`diff`ed. All 713 were byte-identical across the whole change, including the perf fix and the
quote-awareness change measured separately.

## References

- Found during Task 5 of the tool-surface-collapse plan, 2026-09-02; scoped and measured by the Opus
  task review of `5da2537d` as finding M3.
- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*; the class is
  `IC-6`, this corpus's largest at 27 instances across five subsystems.
- Sibling with the same shape one level up:
  `docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`.
