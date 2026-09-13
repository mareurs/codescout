---
id: 067febaee23f1a22
kind: bug
status: fixed
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

Fixed in `abd85071` (patch-id `1c8318fb5b5a0c557e883c150539117bc2ee2940`).

`json_payload_cites()` in `tests/doc_tool_refs.rs` scans fenced ` ```json ` blocks, parses each
with `serde_json`, and emits one claim per top-level key of any object carrying both `"tool"` and
`"arguments"`. A block may hold one call or an array of them; a block that parses but has no
`tool`/`arguments` pair is a config example and is skipped by shape, not by heuristic.

**The favourable shape held.** No grammar, no escape, no disambiguator — `serde_json` either
parses a block or does not, so none of `IC-6`'s debt applies. That is worth contrasting with the
parent: `da911452d5a00116` had to invent `<placeholder>` because its input was prose and a parser
over prose owes an escape. Picking the format-aware parser over a wider regex is what avoided it.

### It found a defect in the guard's own schema extractor on its first run

The census produced exactly one candidate, and the document was **right**:

```
docs/manual/src/tools/symbol-navigation.md:30
  {"tool": "symbols", "arguments": {"workspace": …}}   -> "symbols has no such parameter"
```

`workspace` **is** a real `symbols` parameter. `server.rs`'s `inject_workspace_param` (`:650`)
adds it to every `pinnable()` tool between `input_schema()` and the wire, so it never appears in
the source literal that `tool_params()` reads. **The extractor was narrower than the surface it
checks**, and shipping the payload scan without fixing that would have red a correct page.

Fixed by parsing `Tool::pinnable`'s own `matches!` arm rather than restating the exclusion list,
so a tool added there cannot leave this guard checking a surface the server no longer advertises.
`server.rs`'s `tool_surface_chars` already reproduces this injection for the byte budget, with a
comment that a bare `input_schema()` measurement *"would miss ~6.2 KB of injected `workspace`
prose"* — the knowledge was in the codebase and this file did not share it.

**Why the prose scan never hit it:** it happened not to reach a site using an injected parameter.
A blind spot is only visible when something walks into it, and the new parser walked into this one
immediately.

### Not an `IC-14` member, and the ledger says so

The extractor bug is a guard checking against a stale **model**, not a guard whose reach is
narrower than its **name**. Folding it into the cluster would blur the inclusion test, so the `+1`
names it as a non-member explicitly rather than leaving the omission to be read as an oversight.
## Tests added

One test, `a_documented_json_payload_names_real_parameters`, plus two controls that both earned
their place immediately.

**Controls, because every assertion about what a parser FINDS is satisfied by a parser that finds
nothing:**

- the unpinnable-list parse asserts non-empty. An empty parse silently grants every tool a
  `workspace` parameter — loosening in the false-negative direction, which `tool_params`' own
  comment calls the one direction no assertion in this file can report. **It fired for real on the
  first implementation:** `find(')')` landed on `self.name()`'s closing paren, which sits before
  every string literal in the arm, so the extraction returned nothing and the code looked correct.
  Replaced with a balanced scan. That is an observed RED on that site obtained without mutating
  anything.
- floors on blocks parsed (≥150, measured 191) and cites found (≥70, measured 89). Floors rather
  than equalities so a doc edit does not red the suite but a dead parser does.
- the DECLINED population is asserted rather than absorbed: 17 of 191 blocks are unparseable
  (elisions, fragments) and the test reds if that ever passes a quarter. A silent skip is a new
  blind spot of exactly the kind this file exists to catch.

**Mutations, one per guarded SITE:**

| site | mutation | observed |
|---|---|---|
| payload scan's cite emission | never emits | non-vacuity reds, naming the layer (shape recognition, not the fence scan) |
| `workspace` injection in `tool_params` | removed | reproduces the original false positive verbatim |
| unpinnable-list parse | — | fired during development, not synthetically |

And **end to end**: injecting `{"tool": "symbols", "arguments": {"nmae": …}}` into a real manual
page reds the guard, naming the file, the key and the declared set — whose tail now reads
`… scope, workspace`, so the injection fix is visible in the output. Probe reverted.

The failure message names its own most likely false positive: if a parameter you know is real is
reported missing, suspect the injected-parameter list before the document. That is the one
self-diagnosis this guard can offer, and it is the failure that actually happened.

Gate green: fmt 0, clippy 0, lean 3814 passed, default 5849 passed.
## References

- `docs/issues/archive/2026-09-12-the-documented-parameter-guard-reads-only-key-equals-value-args.md`
  — the parent; this is its § *Fix* direction 2.
- `docs/issues/archive/2026-09-10-the-references-manual-page-teaches-name_path-a-parameter-the-tool-has-never-accepted.md`
  — the defect class in the prose form.
