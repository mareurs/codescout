---
id: '946aec96025d532e'
kind: bug
status: taken
title: 'BUG: the documented-parameter guard never parses the manual''s JSON payload form, so 72 payloads'' arguments are unchecked'
tags:
- cluster/guard-narrower-than-its-name
claimed_at: 2026-09-13
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
parent: da911452d5a00116
---

## Summary

`tests/doc_tool_refs.rs` anchors on `CALL_OPEN` — `\b([a-z][a-z0-9]*(?:_[a-z0-9]+)*)\(` — a tool
name followed by an open paren. **The manual's JSON payload form has no paren**, so no call is
ever recognised and no argument inside one is ever checked:

```json
{ "tool": "symbols", "arguments": { "name": "UserService", "path": "backend/src" } }
```

Measured 2026-09-13 at `d779fbd3`: **72 such payloads across 17 files** in `docs/manual/**`. Every
argument key in all 72 is unguarded. The guard's sibling `a_documented_tool_parameter_exists_on_that_tool`
reports green over them, and its name promises otherwise.

This is the residual `da911452d5a00116` § *Fix* called **direction 2** and explicitly did not close
— "a second parser rather than a widening of the first".

## Symptom (Effect)

A dropped key returns a plausible answer rather than an error (`IC-15`), so a wrong argument in a
worked example is followed, silently does something else, and nothing reports it. The payload form
is the manual's most copy-pasteable surface — it is literally a ready-made call — which makes it
the worst surface to leave unchecked.

## Reproduction

```
grep -nE '"tool"\s*:\s*"[a-z_]+\("  docs/manual/**/*.md   -> 0
grep -cE '"arguments"\s*:'          docs/manual/**/*.md   -> 72 in 17 files
cargo test --workspace --test doc_tool_refs             -> 12 passed
```

Add `{ "tool": "symbols", "arguments": { "nmae": "X" } }` to any present-tense surface and the
suite still passes.

### What the reproduction CHANGED, and it shrinks this bug

`da911452d5a00116` recorded **7** sites of the shape `"tool": "index(action: build)"` — an
unresolvable tool NAME, which cannot dispatch at all and is worse than a wrong parameter.
**Those are gone**, fixed by `04badf94`. The first grep above is the check and it returns 0.

So the half of the original argument that was about unresolvable names is closed. What remains is
the argument-key half, and the population is now **bounded at 72** rather than unknown.

### The suppressor argument still holds, and is the reason to fix this rather than shrug

From `da911452d5a00116`, measured by `b0b9bc40` on their second census pass:
`{"tool": "workspace(action: status)", "arguments": {"threshold": 0.3}}` concealed a second,
independent defect behind the first — no tool has a `threshold` parameter and no per-file drift
score exists anywhere in codescout. Both census runs reported that site as **one** finding.

An unresolvable name suppressed the check on everything inside it. Now that the names are fixed,
the payloads *parse* — and still nothing reads their arguments. The suppression moved from "the
name blocks the check" to "no check exists", which is the same zero with a different cause.

## Root cause

One anchor, two documented forms. `CALL_OPEN` requires `(`; the payload form uses `:` and `{`.
`\b…\(` cannot be widened to reach it without matching prose, which is why the parent bug called
this a second parser.

## Fix

Not fixed. **The shape is favourable and worth stating, because it is the opposite of the parent
bug's problem.** A fenced ` ```json ` block is real JSON, so this needs no grammar of its own:
`serde_json::from_str` either parses it or does not. No `IC-6` debt — no escape to invent, no
disambiguator to design, no prose convention dictated by a parser. The parent bug needed
`<placeholder>` precisely because its input was prose; this input is not.

Two things a naive version gets wrong:

1. **Blocks that are not valid JSON** (elided with `…`, or fragments) must be skipped — and a
   silent skip is a new blind spot of exactly the kind this file is about. Count them and assert
   the count, so the population the parser *declines* is visible rather than absorbed.
2. **Non-vacuity.** A parser that silently finds nothing passes every assertion about what it
   finds. The guard must assert it parsed a non-zero number of payloads, the way
   `the_scan_is_not_reading_an_empty_corpus` already does for the prose scan.

## Tests added

None yet. Must be observed RED: inject a payload with a misspelled argument key, confirm the new
check reds, and mutate the parser to parse nothing and confirm the non-vacuity assertion catches it.

## References

- `docs/issues/archive/2026-09-12-the-documented-parameter-guard-reads-only-key-equals-value-args.md`
  — the parent; this is its § *Fix* direction 2.
- `docs/issues/archive/2026-09-10-the-references-manual-page-teaches-name_path-a-parameter-the-tool-has-never-accepted.md`
  — the defect class in the prose form.
