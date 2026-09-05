---
id: 62fde75268516628
kind: bug
status: open
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

Track `{`/`}` and `[`/`]` depth alongside `(`/`)` in the span walker, and attribute a `key=value`
only at depth 0. Move the quoted-span blanking **before** paren extraction while in there, which
closes the second latent case at the same site.

**And say so at the refusal site.** The class's standing remedy is that where an escape is not
affordable, the *limitation is documented where the refusal happens* — a reader who hits a false RED
should be told the scanner is paren-only, not left to discover it. That half is worth more than the
parser fix, because it is what converts a mystery into a two-minute correction.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance: a fixture with `key=value` inside a nested object literal must **not** produce
a citation attributed to the outer call, and the existing 618 citations must be unchanged — the
second half matters, since a depth-aware walker that drops legitimate top-level citations would also
turn the suite green.

## Workarounds

Use `:` inside `{}` in any scanned surface. This is the house style anyway, so the workaround costs
nothing — which is precisely why the limitation went unrecorded.

## Resume

Add brace/bracket depth to the span walker in `tests/doc_tool_refs.rs`'s `anchored_cites()`, move
quoted-span blanking ahead of paren extraction, then re-run the replication and confirm the citation
count is still **618** — an unchanged total is the guard against a fix that silently narrows the
scan.

## References

- Found during Task 5 of the tool-surface-collapse plan, 2026-09-02; scoped and measured by the Opus
  task review of `5da2537d` as finding M3.
- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*; the class is
  `IC-6`, this corpus's largest at 27 instances across five subsystems.
- Sibling with the same shape one level up:
  `docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md`.

