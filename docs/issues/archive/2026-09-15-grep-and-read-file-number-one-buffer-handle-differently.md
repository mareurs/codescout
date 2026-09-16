---
id: 24bc873cca114d96
kind: bug
status: fixed
title: 'BUG: grep and read_file number the same @tool_* handle differently, so a grep citation reads back as 0 lines'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
- grep
- read_file
- buffer
- progressive-disclosure
topic: buffer addressing
---

## Summary

`grep` and `read_file` address the **same `@tool_*` handle in two different coordinate
systems**, and nothing in either response says so. `grep`'s buffer branch expands escaped
newlines before matching, so it numbers the *expanded* text; `read_file` numbers the raw
pretty-printed JSON. For any buffer holding a multi-line string value the two diverge, and
a line number copied from `grep` into `read_file` addresses **past the end of the buffer**.

The second half is what makes it expensive: an out-of-range range returns `0 lines` — no
error, no "this buffer has N lines". So a citation from the sibling tool reads back as
*"the content is not there"*, which is the one conclusion that is false.

## Symptom (Effect)

A reader greps a buffer, gets `line 228`, reads `start_line=228` on the same handle, and
is told nothing is there. The natural next move is to re-run the producing tool — the
buffer system exists precisely to avoid that.

## Reproduction

Measured 2026-09-15, `experiments` at `b095bcae` plus uncommitted work.

```
symbols(name="append_entry", path="src/librarian/catalog/augmentation.rs",
        include_body=true)                       -> @tool_a3a0dce8
grep(pattern="fs::write|\\.commit\\(\\)", path="@tool_a3a0dce8")
                                                 -> matches at lines 197-229, 321-399
read_file("@tool_a3a0dce8", start_line=228, end_line=300)
                                                 -> "0 lines"
read_file("@tool_a3a0dce8", start_line=1, end_line=4)
                                                 -> works; the buffer is 157 lines total
```

157 < 197, so **every** line `grep` reported is unaddressable by `read_file`.

**The control, which is what makes this a measurement rather than a broken read.** On a
buffer with no multi-line string value the two agree exactly:

```
grep(pattern="pub fn", glob="src/**/*.rs", limit=80)   -> @tool_a3b2ae1f (788 lines)
grep(pattern="default_workspace_mut", path="@tool_a3b2ae1f") -> line 12
read_file("@tool_a3b2ae1f", start_line=10, end_line=14)      -> same content at line 12
```

So the numbering *can* agree, and the divergence is a property of the payload's shape
rather than of buffers in general. Without this control the first reading is "line-range
reads on `@tool_*` are broken", which is false and sends the fix at the wrong function.

**The silent-zero half, separately:**

```
read_file("@tool_a3b2ae1f", start_line=900, end_line=910)   -> "0 lines"
```

on a buffer whose own reads report `788 lines`.

## Environment

codescout MCP, `.claude-sdd` profile, shared checkout with 5 live sessions.

## Root cause

Read, not inferred. `grep_in_buffer`'s `@tool_*` branch (`src/tools/grep.rs:983`)
pretty-prints the JSON and then materializes escaped newlines:

```rust
.map(|pretty| pretty.replace("\\n", "\n"))
```

That line was the **fix** for
`docs/issues/archive/2026-07-01-grep-buffer-multiline-string-value-collapses.md`, where a
whole artifact `body` collapsed to one physical line and matched at most once. The fix is
correct for matching. What it also did — invisibly, because no test compares the two tools
— is give `grep` a line space that `read_file`'s buffer path does not share: `read_file`
never applies that `replace`, so it counts the un-expanded lines.

Neither tool is wrong on its own. The defect is that one handle now denotes two line
spaces and no field distinguishes them.

## Evidence

The reproduction above was run in this session before the file was opened. The producing
call was incidental — the symbols-with-body shape is exactly the payload that triggers it,
and it is one of the most common buffers an agent creates.

## Hypotheses tried

1. **Line ranges on `@tool_*` are broken.** **Refuted** by the control — `@tool_a3b2ae1f`
   reads ranges correctly, including the exact line `grep` cited.
2. **The buffer was evicted / expired.** **Refuted** — both older handles still read fine
   at `start_line=1` later in the same session.
3. **`read_file` mis-reports the total.** **Refuted** — `157` and `788` are consistent
   across repeated reads and match the payload.

## Fix

**Fixed** in `57758be9`, patch-id `1ee5d0d79de83a2e29dda872e7f13863a795f97a`. Both halves
shipped. The priority proposed below **inverted under reproduction**, and the original wording is
kept rather than tidied because it is a rejected approach a reader would otherwise retry:

1. **Share the coordinate space, or name it.** Either `read_file`'s buffer path applies
   the same expansion, or `grep`'s buffer result marks its line numbers as belonging to an
   expanded view (and says which reader resolves them). Sharing is the smaller change and
   the one that makes the citation work.
2. **An out-of-range buffer read must not return `0 lines`.** It should say the buffer has
   N lines and the requested range starts past it. This is
   `docs/adrs/2026-08-27-negative-results-name-their-scope.md` exactly: the zero is
   suspicious, so it owes its scope. This half is cheap and is what converts the failure
   from silent to self-explaining.

### What the reproduction changed — half 2 is NOT safe to ship alone

The headline above is the out-of-range case, which is the half a reader can notice. Re-running
the reproduction before reading this plan showed the **addressable** range is the worse half: on
a live `symbols(include_body=true)` buffer, `grep`'s line 10 was a doc-comment line and
`read_file`'s line 10 was `"body_start_line": 680,` — different content, same number, **both
calls succeeding**.

So "worth having even if the first is declined" was wrong, and wrong in the expensive direction.
Half 2 alone makes the loud case loud and leaves the silent case silent, and a reader who learns
*out-of-range now errors* will reasonably infer *therefore a successful read is correctly
addressed*. That inference is false, and the partial fix is precisely what licenses it.

### And the shape was wrong too — a shared derivation, not a second copy

The plan's half 1 reads as "apply the same expansion in `read_file`". That closes today's
divergence and leaves the next one one edit away. It was already not hypothetical: a **third**
copy had grown inside `read_file_buffer_single_oversized_line_still_fits_the_threshold`, which
re-typed `to_string_pretty` to compute the line its assertion targets — a test asserting against
its own re-implementation. Shipped instead as `line_addressable_text` in `output_buffer.rs`, the
single implementation both tools and that test now call.

**One constraint the plan does not mention, and a naive fix would have broken silently.**
`read_file`'s `json_path` branch RE-PARSES its text, and expansion puts a bare newline inside a
JSON string literal — invalid JSON. `grep` never re-parses, which is what its own "search-only
text" comment was recording. So the identical transform is safe in one function and destructive
in the other, and the shared call must sit **after** the `json_path` branch, not beside the
pretty-print where it visually belongs.

## Tests added

Six, in `src/tools/read_file.rs`. All five mutation sites killed; `M2` survived its first run
and is the finding worth keeping.

- `grep_and_read_file_number_one_tool_handle_identically` — the cross-tool assertion. Greps a
  buffer, reads back the line `grep` cited, asserts the content matches.
- `a_flat_tool_buffer_numbers_identically_and_witnesses_nothing` — the control, annotated
  **inert** so nobody credits it: it is green before and after the fix.
- `json_path_still_resolves_when_a_sibling_value_is_multi_line` — the ordering constraint. The
  pre-existing `json_path` test uses a single-line body and stays green under the mutation that
  breaks this, so it was not a witness.
- `a_cmd_buffer_that_happens_to_be_json_is_served_raw_not_reformatted` — written because the
  handle-kind guard SURVIVED its mutation. For plain text `from_str` fails and the fallback
  returns raw anyway, so the guard reads as inert; but a `@cmd_*` capture that IS valid JSON
  (`curl`, `gh api --json`) would have been reformatted and expanded. Real domain, common,
  untested.
- `an_out_of_range_read_names_the_total_instead_of_a_bare_zero` and
  `a_genuinely_empty_target_still_reads_as_a_plain_zero` — the pair. The second exists so the
  first cannot be satisfied by unconditionally printing a total.

One pre-existing test had to be repaired rather than adjusted: the `\n`-joined fixture in
`read_file_buffer_single_oversized_line_still_fits_the_threshold` went **inert** under the
unification, and its own premise guard caught it and named the reason (`widest line is 22 vs
budget 9000`). That guard is the best-behaved thing this bug touched — it was written against a
change nobody anticipated and still fired.

The original note on the shape a guard needs, which held up: asserting `grep`'s line resolves in `read_file` is
a **cross-tool** assertion, and neither tool's own suite is a place it will occur to
anyone to put one — which is why this shipped inside a fix that was itself well tested.
The fixture must contain a multi-line string value, and that detail is load-bearing: on
any other payload the two agree and the test passes while asserting nothing.

## Workarounds

- Use `json_path` to address a `@tool_*` buffer; it is unaffected.
- Treat a `grep` line number on a `@tool_*` handle as an ordering hint, not an address.
- A `0 lines` answer from a buffer read means "check the total", never "absent".

## Classification

`cluster/addressing-without-an-escape-hatch` (`IC-6`), the **no-disambiguator** half: one
handle names two line spaces and nothing in either response tells a reader which one they
are holding. Weighed and rejected: `IC-13` `capped-result-presented-as-complete` (nothing
is truncated here — the bytes are all present and correctly counted in both views), and
`IC-18` `selector-narrower-than-its-population` (the selector is right; the address space
it returns coordinates in is the thing that differs).

## Resume

Nothing owed. Both halves shipped in `57758be9`; gate green `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

**The advice this section used to give was wrong and is worth one line, because it is the kind
that reads as prudent.** It said to decide half 2 first — small, self-contained, makes half 1
diagnosable. Reproduction showed half 2 addresses only the case a reader can already notice,
while licensing a false inference about the case they cannot. Shipping the cheap half of a
two-half defect is not always a safe down-payment; when the halves are *loud* and *silent*, the
cheap one can make the silent half harder to find.

## References

- `docs/issues/archive/2026-07-01-grep-buffer-multiline-string-value-collapses.md` — the
  fix this is the downstream cost of. Not a regression of it: that bug is still fixed.
- `docs/issues/archive/2026-05-09-read-file-buffer-midpoint-empty.md` (`wontfix`) — a
  different empty-read shape; that one is midpoint-dependent, this one is a coordinate
  mismatch with an exact boundary.
- `get_guide("progressive-disclosure")` — whose `@ref` section advertises
  `read_file("@tool_xyz", start_line=N, end_line=M)` as the browse form.
