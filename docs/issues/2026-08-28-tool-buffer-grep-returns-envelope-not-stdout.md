---
id: '2d546e0f7b8fcc0c'
kind: bug
status: investigating
title: 'BUG: grepping a @tool_* buffer that holds a run_command result returns the JSON envelope, and each re-read re-wraps — the stdout is unreachable'
owners:
- marius
tags:
- run_command
- output_buffer
- progressive-disclosure
---

# BUG: grepping a `@tool_*` buffer returns the envelope, not the stdout

## Summary

Chaining `grep` across buffer handles can reach a state where every read returns
the `run_command` **JSON envelope** (`{exit_code, output_id, ...}`) instead of the
stdout it describes, and each follow-up read produces another envelope one level
down. The payload becomes unreachable through the buffer API; the only escape is
to re-run the command redirecting to a real file.

**Not isolated.** The sequence below was observed live and is recorded verbatim.
Three attempts to reduce it to a minimal case did **not** reproduce. Filed as a
quirk with evidence rather than a diagnosed defect — read the Hypotheses section
before trusting any mechanism.

## Symptom (Effect)

Observed 2026-08-28, working the doctor report. `@tool_4795f92d` is a
`librarian(action="doctor")` result (136367 bytes).

```
run_command("grep -c 'work/claude/codescout' @tool_4795f92d; echo ---;
             grep -o '\"path\": \"[^\"]*\"' @tool_4795f92d")
→ @tool_479696b5   "✓ exit 0 · 127 lines"   buffered_bytes: 10273

run_command("grep -v 'work/claude/codescout' @tool_479696b5")
→ @tool_47969f3f   "✓ exit 0  (query @cmd_47969f3d)"   buffered_bytes: 11492

read_file("@tool_47969f3f")
→ 6 lines:  { "type": "generic", "exit_code": 0, "output_id": "@cmd_47969f3d", ...
             [4 of 6 lines shown]

read_file("@cmd_47969f3d")
→ 4 lines:  { "exit_code": 0,   [2 of 4 lines shown]

read_file("@tool_479696b5", start_line=1, end_line=30)
→ 4 lines:  { "exit_code": 0,   Buffer: @file_4796c316

read_file("@file_4796c316", start_line=1, end_line=25)
→ 4 lines:  { "exit_code": 0,   Buffer: @file_4796cc53
```

Six reads, four distinct handle kinds (`@tool_`, `@cmd_`, `@file_`), zero lines of
the grep output. Escaped by re-running with `> /tmp/doctor-paths.txt` and reading
the file.

## Reproduction

**REPRODUCED 2026-08-29**, on the third hypothesis — the compound command was
indeed the missing ingredient, though not for the reason guessed. Deterministic,
four steps, ~5 seconds.

```
1. librarian(action="doctor")
   -> @tool_4e1a84e6   132088 bytes        (original: 136367)

2. run_command("grep -c 'work/claude/codescout' @tool_4e1a84e6; echo ---;
                grep -o '\"path\": \"[^\"]*\"' @tool_4e1a84e6")
   -> @tool_4e1a9626   "exit 0 - 119 lines"   buffered_bytes: 10011
                                            (original: 127 lines, 10273)

3. run_command("grep -v 'work/claude/codescout' @tool_4e1a9626")
   -> @tool_4e1ab2d8   "exit 0  (query @cmd_4e1ab2d6)"   11158
                                            (original: 11492)

4. read_file("@tool_4e1ab2d8")
   -> 6 lines: { "type": "generic", "exit_code": 0, "output_id": "@cmd_4e1ab2d6", ...
```

Every number lands within 3% of the original transcript. The regress continues
exactly as filed: `read_file("@cmd_4e1ab2d6")` gives the envelope again;
`read_file("@tool_4e1ab2d8", start_line=5, end_line=6)` gives a NEW 13389-byte
`@tool_*`, **larger than the six-line buffer it was asked to read two lines of**;
`json_path="$.content"` on that gives another 11131-byte handle. It does not
converge, and the reason it cannot is in the Root cause section.

**Why the earlier three attempts missed it.** All three produced output whose
lines were individually small. The trigger is not size as such, not the handle
kind, and not the compound command as such — it is that the command emitted
enough output to be handed back as an `output_id` envelope rather than inline,
and the envelope form puts all of stdout on one line. Attempt 2 (15400 B) *did*
exceed the threshold but returned inline with `stdout_shown`/`stdout_total`, so
it never took the envelope path.
## Environment

Branch `experiments` @ `14aab5ff`, linux, codescout 0.15.0, MCP over stdio,
release build from `cargo rb` at 11:55 local.

## Root cause

**Measured 2026-08-29.** Not a leak and not a nesting bug: a **structural**
impossibility for a line-oriented reader.

### The two payload kinds

`run_command` can hand back a buffer through either of two response fields, and
they hold *different things*:

| Field | Payload | Shape (measured) | Line-sliceable |
|---|---|---|---|
| `unfiltered_output` | raw text | 400 lines / 1492 B | yes |
| `output_id` (whole response overflowed) | **JSON envelope** | 4 lines / 10021 B, **line 3 = 9998 B** | **no** |

```
$ awk '{print NR": "length($0)}' @cmd_4e1ab2d6
1: 1        {
2: 17         "exit_code": 0,
3: 9998       "stdout": "21\n---\n..."     <- ALL of stdout, on one line
4: 1        }
```

### Why it cannot terminate

`read_file` addresses buffers by **line**. The entire stdout lives on line 3,
whose ~10 KB exceeds `INLINE_BYTE_BUDGET` (9000 B). So any read whose range
includes line 3 overflows and is re-buffered into a new handle — which has the
same one-enormous-line shape, so the next read does the same thing. **The
smallest addressable unit is larger than the largest returnable one**, and no
sequence of line-range reads can converge. Reads that *exclude* line 3 return
only punctuation, and still re-buffer, because the wrapping response carries its
own metadata.

### The filed hypothesis was wrong, and the correction matters

This file's leading hypothesis was that the discriminator is the handle
**prefix** — `@cmd_*` holds raw stdout, `@tool_*` holds the envelope. **Refuted
by measurement:** in the reproduced chain `@cmd_4e1ab2d6` and `@tool_4e1a9626`
are byte-identical in structure (4 lines, line 3 = 9998 B) and *both* hold
envelopes. A `@cmd_*` handle proves nothing about the payload.

The real discriminator is **which response field the handle arrived in**. The
filed diagnosis of the underlying defect survives intact — nothing in a handle
tells the caller which payload kind it holds — but the axis was misidentified,
and a fix built on the prefix would have changed nothing.

### The data is intact

`jq -e . @cmd_4e1ab2d6` parses, and extracting `.stdout` then counting entries
returns 117 — nothing was truncated or corrupted. "The stdout is unreachable" is
true only of line-oriented readers; byte- and pattern-oriented ones (`jq`,
`grep`, `awk`, `head -c`) reach it immediately. That is narrower and more
fixable than the title of this file, which is now overstated.
## Hypotheses tried

1. **Hypothesis:** any grep of a buffer whose result exceeds the 10000-byte
   threshold nests.
   **Test:** repro attempt 2 — 15400 bytes, 50 lines.
   **Verdict:** rejected — returned stdout inline. Correctly rejected, but the
   reason is sharper than recorded: exceeding the threshold is *necessary*, and
   what decides the outcome is whether the response comes back inline with
   `stdout_shown`/`stdout_total` or as an `output_id` envelope.

2. **Hypothesis:** it is specific to `@tool_*` source buffers.
   **Test:** repro attempt 3 — grepped the same `@tool_4795f92d` with a different
   pattern.
   **Verdict:** rejected, and now rejected *twice over* — the 2026-08-29
   measurement shows `@cmd_*` and `@tool_*` handles holding structurally
   identical envelopes. The prefix is not the axis.

3. **Hypothesis:** the compound command (`;`-separated, mixing `grep -c` with
   `grep -o`) is the trigger.
   **Test:** run 2026-08-29, exactly as prescribed by
   `resume-cross-machine-catalog-restore:CM-7`.
   **Verdict:** **confirmed as the reproducer, rejected as the cause.** It
   reproduces every time, but only because it emits enough small-line output in
   one shot to route the result through the envelope path. Any single command
   with the same output profile does the same; the `;` is incidental.

4. **Hypothesis:** the buffered JSON is truncated, and that is why reads fail.
   **Test:** `jq -e .` on the buffer; counted the entries inside `.stdout`.
   **Verdict:** rejected — valid JSON, all 117 entries present. The payload is
   intact and merely unaddressable by line.
## Fix

None. Do not fix before reproducing — the mechanism above is inferred from handle
names, and the repo's own record has three cases this month where a bug file's
prescribed fix was wrong in *direction*
(`docs/trackers/bug-ledger-resume-2026-08-28.md` § Method notes).

## Tests added

None. A regression test is premature without a reproduction.

## Workarounds

Redirect to a real file and read that:

```
run_command("grep -o '\"path\": \"[^\"]*\"' @tool_xxxx > /tmp/out.txt; wc -l /tmp/out.txt")
read_file("/tmp/out.txt")
```

This also sidesteps Iron Law 3 cleanly, since the redirect is not a pipe to a
trimmer.

## Resume

Run hypothesis 3: issue a compound `run_command` (`grep -c X @tool_ref; echo ---;
grep -o Y @tool_ref`) against a large `@tool_*` buffer and check whether the result
handle is `@tool_*` or `@cmd_*`, and whether reading it yields stdout or an
envelope. If it reproduces, read `src/tools/output_buffer.rs` for where the handle
kind is chosen and whether the stored payload differs by kind.

## References

- `get_guide("progressive-disclosure")` § The @ref buffer
- `docs/trackers/tool-usage-patterns.md` T-27 — *grep -n on a @tool_\* buffer numbers the buffer, not the source* (a neighbouring quirk in the same surface)
- Found during the 2026-08-28 cross-machine catalog repair; see `docs/conventions/cross-machine-catalog-resume.md`
