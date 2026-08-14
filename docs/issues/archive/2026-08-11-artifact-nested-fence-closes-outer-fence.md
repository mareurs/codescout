---
id: '2149583d7f1faf37'
kind: bug
status: fixed
title: 'artifact body_edits: a nested ``` fence closes an enclosing ```` fence, so code comments become headings'
owners:
- marius
tags:
- librarian
- artifact
- markdown-parser
- body_edits
- fenced-code
closed: 2026-08-14
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

**Measured 2026-08-14** — the inferred mechanism below was confirmed at the
bytes, and the surface count turned out to be three parsers, not one.

Three independent line-oriented scanners each kept a bare `bool` and flipped it
on any line starting with three backticks:

| parser | surface | site |
|---|---|---|
| `librarian::preview::headings::parse` | `artifact(get)`'s heading map | `src/librarian/preview/headings.rs:17-23` |
| `file_summary::parse_all_headings` | `artifact(get, heading=…)` via `extract_markdown_section` | `src/tools/file_summary/file_summary.rs:151-160` |
| `edit_markdown::compute_section_end` + `find_consumed_subsections` + `line_in_code_block` | `body_edits` scoping (`update.rs:173,214` call `perform_scoped_edit` / `perform_section_edit_ext`) | `src/tools/markdown/edit_markdown.rs:328,373,959` |

Two more sites shared the defect without being implicated in this report:
`librarian::preview::summary::extract` and `librarian::preview::plan::extract`.

CommonMark requires a closing fence to be the **same character** and **at least
as long** as the opener. A boolean records neither, so an inner three-backtick
run closes an outer four-backtick block and the remainder of the block parses as
document body.

### The falsifying trace, on this very bug file

No new fixture was needed — this file's own § Reproduction nests fences, so the
live server reproduced it. `artifact(get, id=2149583d7f1faf37)` returned a
heading map containing `{"level": 1, "text": "Page Title", "line": 58}`.
Walking the old toggle over the same lines predicts exactly that, and only that:

| line | text | delimiter? | `in_fence` after |
|---|---|---|---|
| 52 | ` ````markdown ` | toggled | true |
| 53 | `### Some Task` | — | skipped (correct) |
| 57 | ` ```` ```markdown ```` ` | toggled | **false** |
| 58 | `# Page Title` | — | **emitted → phantom** |
| 60 | ` ```toml ` | toggled | true |
| 64 | ` ``` ` | toggled | false |
| 65 | ` ```` ``` ```` ` | toggled | true |
| 68 | ` ```` ` | toggled | false |

One phantom, at line 58, and no others — because the fence-ish lines happen to
be even in number, parity restores itself and later real headings survive. That
accidental parity is why the bug looked intermittent.

### Two rules were load-bearing, not one

The run-length rule alone does not fix line 57: its run is **four** backticks,
so it satisfies `run >= open_run`. What rejects it is CommonMark's other two
constraints — a closer may be followed only by whitespace, and a backtick
fence's info string may not contain a backtick. Both were needed; each was
mutation-tested (see § Tests added).
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

Implemented in `8ba65118` (`experiments`). `master` is a strict ancestor
(`git rev-list --left-right --count master...experiments` → `0 658`), so
promotion is a fast-forward and this SHA already is the master-side SHA — there
is no second SHA to record.

A shared tracker replaces all seven boolean sites:
`src/util/markdown_fence.rs` — `FenceState` stores the open fence's `(char, run
length)` and closes only on a run of the same character that is at least as long
and followed by nothing but whitespace; it also refuses to *open* on a backtick
run whose info string contains a backtick. `fences_balanced()` replaces the two
parity pre-scans with a real scan.

Converted sites:

- `src/librarian/preview/headings.rs` — `parse`
- `src/librarian/preview/summary.rs` — `extract`
- `src/librarian/preview/plan.rs` — `extract`
- `src/tools/file_summary/file_summary.rs` — `parse_all_headings`
- `src/tools/markdown/edit_markdown.rs` — `compute_section_end`,
  `find_consumed_subsections`, `line_in_code_block`

Three deliberate scope decisions:

1. **`FenceState::feed` never trims.** Call sites in this tree disagree about
   leading whitespace (raw / `trim_start` / `trim`), and CommonMark's real rule
   (≤3 spaces, relative to the enclosing container) needs full block parsing.
   Each site keeps its existing trim, so the diff carries only the fence-length
   and fence-character rules.
2. **The parity pre-scans became real scans.** `fences_balanced` previously
   counted fence-ish lines and called an odd count unbalanced — which
   *disabled fence tracking entirely* for any file containing a nested shorter
   run. That is a second, wider failure mode the parity count created on its
   own; the regression test `balanced_is_not_a_parity_count` pins it. The
   escape hatch's original intent (a half-fence from an in-flight batch edit
   must not hide later headings — `docs/issues/2026-05-21-edit-markdown-last-heading-unaddressable.md`)
   is preserved exactly: when the scan ends inside a fence, tracking is skipped.
3. **`src/tools/symbol/display.rs:89` was NOT converted.** Its `in_code_block`
   is written and never read, so converting it would start skipping fenced
   content in rendered hover text — a rendering change, not a defect fix. Filed
   separately as `docs/issues/2026-08-14-format-hover-fence-tracking-is-dead-state.md`.

`line_in_code_block` also gains a correctness improvement for free: a backtick
run no longer closes a `~~~` block.
## Tests added

**Helper unit tests** — `src/util/markdown_fence.rs`, module `tests` (11):
`a_shorter_run_does_not_close_a_longer_fence`,
`a_backtick_run_does_not_close_a_tilde_fence`,
`a_longer_run_closes_a_shorter_fence`,
`a_closer_with_trailing_content_is_not_a_closer`,
`an_inline_code_span_of_a_fence_does_not_open_a_block`,
`a_run_shorter_than_three_never_opens_a_fence`,
`a_tilde_fence_info_string_may_contain_backticks`,
`plain_nested_fences_still_round_trip`,
`balanced_reports_an_unclosed_fence`, `balanced_is_not_a_parity_count`,
`feed_reports_only_real_delimiters`.

**Surface regression tests** — one per affected parser:

- `src/librarian/preview/headings.rs` —
  `a_nested_shorter_fence_does_not_leak_headings_from_the_outer_block` (the
  exact fixture from § Reproduction), `a_backtick_fence_does_not_close_a_tilde_block`
- `src/tools/file_summary/tests.rs` —
  `parse_all_headings_respects_nested_fence_run_length`,
  `extract_markdown_section_spans_a_nested_fence`,
  `parse_all_headings_does_not_close_a_tilde_fence_with_backticks`
- `src/tools/markdown/tests.rs` — `scoped_edit_reaches_text_after_a_nested_fence`
  (reproduces the reported `old_string not found` end-to-end),
  `find_consumed_subsections_ignores_headings_inside_a_nested_fence`

**Mutation-tested, checking *which* assertion fires.** Two runs, both against
the helper:

| mutation | tests that failed | failure shape |
|---|---|---|
| drop `run >= open_run` | `a_shorter_run_does_not_close_a_longer_fence`, `balanced_is_not_a_parity_count`, `feed_reports_only_real_delimiters` | `left: ["before"]` — trailing content swallowed |
| drop the backtick info-string rule | `an_inline_code_span_of_a_fence_does_not_open_a_block` only | `left: []` — whole document swallowed |

Both rules are therefore load-bearing, and neither test is a tautology.

Each surface test was additionally traced against the old boolean to confirm it
discriminates: all five fail under the pre-fix parser (three via a level-1
phantom truncating the enclosing section, one via the `old_string not found`
error itself, one via the phantom appearing as a `replace` victim).

Gate at the fix commit: `cargo test --lib` → 3607 passed / 0 failed / 7 ignored;
`cargo clippy --workspace --all-targets -- -D warnings` clean.
## Workarounds

Two, both verified in this session:

1. Target the phantom heading. `artifact(action="get")`'s heading-not-found error
   lists every heading the parser sees, phantoms included — read that list and
   scope the edit to the phantom.
2. Avoid nesting fences: use an indented block or a single fence level inside
   artifact bodies that will later be edited with `body_edits`.

## Resume

N/A — fixed and gated.

One live-surface caveat if you are reading this before the next `cargo rb` +
`/mcp`: the **running** MCP server still carries the pre-fix parser, so
`artifact(get)` on this file will keep reporting the `# Page Title` phantom
until the binary is rebuilt. The phantom is harmless for editing this file —
it sits at line 58, ahead of every section below § Environment, so those scope
correctly.
## References

- `docs/issues/archive/2026-06-19-edit-markdown-fenced-comment-section-truncation.md`
  — same symptom class, different surface and different mechanism, fixed 2026-07-03
- `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md` — the
  artifact that surfaced it
