---
id: '716183640e6a047e'
kind: bug
status: open
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

Not attempted. Two independent halves, and the second is worth having even if the first
is declined:

1. **Share the coordinate space, or name it.** Either `read_file`'s buffer path applies
   the same expansion, or `grep`'s buffer result marks its line numbers as belonging to an
   expanded view (and says which reader resolves them). Sharing is the smaller change and
   the one that makes the citation work.
2. **An out-of-range buffer read must not return `0 lines`.** It should say the buffer has
   N lines and the requested range starts past it. This is
   `docs/adrs/2026-08-27-negative-results-name-their-scope.md` exactly: the zero is
   suspicious, so it owes its scope. This half is cheap and is what converts the failure
   from silent to self-explaining.

## Tests added

None yet. Note the shape a guard needs: asserting `grep`'s line resolves in `read_file` is
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

Decide half 2 first — it is small, self-contained, and makes half 1 diagnosable by whoever
hits it next instead of silent.

## References

- `docs/issues/archive/2026-07-01-grep-buffer-multiline-string-value-collapses.md` — the
  fix this is the downstream cost of. Not a regression of it: that bug is still fixed.
- `docs/issues/archive/2026-05-09-read-file-buffer-midpoint-empty.md` (`wontfix`) — a
  different empty-read shape; that one is midpoint-dependent, this one is a coordinate
  mismatch with an exact boundary.
- `get_guide("progressive-disclosure")` — whose `@ref` section advertises
  `read_file("@tool_xyz", start_line=N, end_line=M)` as the browse form.
