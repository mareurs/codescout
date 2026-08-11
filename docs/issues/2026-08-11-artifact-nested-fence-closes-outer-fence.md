---
id: '2149583d7f1faf37'
kind: bug
status: open
title: 'artifact body_edits: a nested ``` fence closes an enclosing ```` fence, so code comments become headings'
owners:
- marius
tags:
- librarian
- artifact
- markdown-parser
- body_edits
- fenced-code
---

# BUG: a nested ``` fence closes an enclosing ```` fence in the artifact heading parser

## Summary

The librarian's markdown section parser (behind `artifact(action="get")`'s
heading map and `artifact(action="update", patch={body_edits})`'s scoping)
closes a four-backtick fence when it meets a three-backtick opener. Everything
after the inner fence is then parsed as document body, so `#`-prefixed code
comments become headings. Scoped edits targeting text after that point fail with
`old_string not found`, and the heading map reports sections that do not exist.

Distinct from `docs/issues/archive/2026-06-19-edit-markdown-fenced-comment-section-truncation.md`
(fixed 2026-07-03), which was `read_markdown`/`edit_markdown` not tracking fence
state at all. This parser *does* track it — it gets the fence *length* rule wrong.

## Symptom (Effect)

Editing a plan whose task section contains a ````markdown block that itself
contains a ```toml block:

```
body_edits[2]: old_string not found in section
'### Task 9: Report the truth, and document the path'.
The text must match exactly (whitespace-sensitive).
```

The text was present and byte-identical — verified with
`sed -n '1465,1470p' <file> | cat -A`:

```
Add it to `docs/manual/src/SUMMARY.md` under the guide section, matching the existing entry style.$
```

Retrying with an explicit heading then revealed the phantom section in the
error's own heading list:

```
heading '## Switching models on an existing index' not found — hint: Available headings:
# Local ONNX Embedding Reaches the Query Path — Implementation Plan, ## Global Constraints,
### Task 1: …, ### Task 9: Report the truth, and document the path,
# .codescout/project.toml,                      <-- phantom, from inside a fence
## Verification (whole branch), ## Deviations from the spec — both need a ruling
```

The edit succeeded once retargeted at `heading="# .codescout/project.toml"`.

## Reproduction

1. `git rev-parse HEAD` → `946d9635` (branch `experiments`).
2. Create an artifact whose body contains, inside one `##`/`###` section:

````markdown
### Some Task

Create the page:

```` ```markdown ````
# Page Title

```toml
# .codescout/project.toml
[embeddings]
model = "local:AllMiniLML6V2Q"
```
```` ``` ````

Then a line of prose after the outer fence closes.
````

3. `artifact(action="update", id=…, patch={body_edits: [{heading: "### Some Task",
   action: "edit", old_string: "<the prose after the outer fence>", …}]})`
4. Observe `old_string not found in section`.
5. `artifact(action="get", id=…)` → the heading map lists `# .codescout/project.toml`.

## Environment

codescout MCP, branch `experiments` at `946d9635`, Linux. Surfaced while writing
`docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md`.

## Root cause

Unknown at the code level — **inferred from observed behaviour, not measured
against the parser source**. The discriminating observation: headings inside the
*outer* ````markdown fence that appear **before** the inner ```toml fence
(`# Embedding without a server`, `## Build with the local backend`) are correctly
absent from the heading map, while `# .codescout/project.toml` — which appears
**after** the inner fence opens — is present. That is the signature of a parser
that opens on a fence run but closes on any subsequent run regardless of length:
the inner ` ``` ` terminates the outer ` ```` `, and the rest of the block is read
as body.

CommonMark requires the closing fence to be **at least as long as** the opening
fence, and to be the same character. A parser storing only a boolean
`in_fence` — rather than the opening run's length and character — produces
exactly this.

Next session: find the section extractor the librarian uses for `body_edits`
scoping and the `get` heading map (it is a different path from the one fixed in
`c6184884`-era work for `read_markdown`/`edit_markdown`), and check whether it
records the opening fence length.

## Evidence

### The heading map contains a fenced line

Quoted verbatim in § Symptom — `# .codescout/project.toml` is a TOML comment on
the first line of a ```toml block nested inside a ````markdown block. It is the
only phantom entry, and it is the first `#` line after the inner fence opens.

### Byte-level confirmation the target text existed

`sed -n '1465,1470p' … | cat -A` printed the line with a trailing `$` and no
stray whitespace, ruling out the ordinary causes of an exact-match miss (a
rendered-vs-bytes gap, trailing spaces, non-breaking hyphens).

## Hypotheses tried

1. **Hypothesis:** the `old_string` had a whitespace or unicode mismatch.
   **Test:** `cat -A` on the exact line range.
   **Verdict:** rejected — byte-identical.
2. **Hypothesis:** batch edits are applied sequentially and an earlier edit in
   the same call moved the target text.
   **Test:** the failing edit's `old_string` shares no text with the other three;
   the batch is atomic and nothing applied.
   **Verdict:** rejected.
3. **Hypothesis:** the outer ````markdown fence is not tracked at all (the
   2026-06-19 bug, recurring).
   **Test:** check whether headings *inside* the outer fence and *before* the
   inner fence appear in the heading map.
   **Verdict:** rejected — they are correctly absent. Only lines after the inner
   fence leak, which points at fence *length*, not fence *tracking*.

## Fix

Not implemented. Proposed: store the opening fence's character and run length,
and close only on a run of the same character that is at least as long, per
CommonMark. Then re-run the reproduction and confirm `# .codescout/project.toml`
disappears from the heading map.

## Tests added

None yet. The regression test should be a fixture with a ````-fenced block
containing a ```-fenced block containing a `#` comment, asserting the heading map
contains only the real headings — mirroring the fixture style of the
2026-06-19 bug's regression test.

## Workarounds

Two, both verified in this session:

1. Target the phantom heading. `artifact(action="get")`'s heading-not-found error
   lists every heading the parser sees, phantoms included — read that list and
   scope the edit to the phantom.
2. Avoid nesting fences: use an indented block or a single fence level inside
   artifact bodies that will later be edited with `body_edits`.

## Resume

Locate the section extractor used by the librarian's `body_edits` scoping and
`artifact(action="get")` heading map — start from the `body_edits` handler in
`src/librarian/tools/` and follow the heading-splitting helper it calls. Confirm
whether it stores the opening fence run length; if it stores a boolean, that is
the defect. Reproduce with the fixture in § Reproduction before changing
anything.

## References

- `docs/issues/archive/2026-06-19-edit-markdown-fenced-comment-section-truncation.md`
  — same symptom class, different surface and different mechanism, fixed 2026-07-03
- `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md` — the
  artifact that surfaced it

