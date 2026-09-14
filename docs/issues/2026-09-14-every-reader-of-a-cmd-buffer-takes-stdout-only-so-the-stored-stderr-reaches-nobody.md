---
id: '2546172a20a4751e'
kind: bug
status: open
title: 'BUG: every reader of a @cmd_* buffer takes .stdout only, so the stderr it stores reaches nobody'
tags:
- cluster/capped-result-presented-as-complete
- run-command
- progressive-disclosure
- output-buffer
---

## Summary

`run_command` stores both streams: `ctx.output_buffer.store(command, raw_stdout, raw_stderr,
exit_code)`, and `BufferEntry` has a `pub stderr: String` field to hold it.

**Nothing ever reads that field.** Both readers of a `@cmd_*` handle take `.stdout` alone:

- `src/tools/grep.rs` — `.get(raw_path)?.stdout`
- `src/tools/read_file.rs`, `read_from_buffer` — `.get(path)?.stdout`

So `grep PATTERN @cmd_abc`, `read_file("@cmd_abc")` and `sed -n '1,50p' @cmd_abc` all answer
about **stdout only**, while presenting themselves as reads of the command's output. A search
that finds nothing has not shown the output does not contain it.

This is `CLAUDE.md` § *Testing Discipline*, "loudness is a property of a PATH": a field
faithfully captured, faithfully stored, and reachable by no observer.

## Symptom (Effect)

A `0` from a buffer grep is indistinguishable from genuine absence. That is the same shape as
`docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md`,
whose remedy was the `truncated: Option<Truncation>` field and its self-describing sentinel —
built precisely so a capped buffer could not be misread as a complete one. That remedy does
not cover this: the buffer is not truncated, it is missing a stream.

The reader has no available workaround either. `read_file(@cmd_*, json_path=...)` is refused
for non-`@tool_*` handles by design ("buffers are raw text"), so there is no field to address.

## Reproduction

2026-09-14, main checkout, codescout `v0.15.0`. A script emitting 4000 stdout lines plus one
marked stderr line, run twice through `run_command` — once classified `generic`, once `test`
(the command string carried `cargo test`, which is what `detect_command_type` keys on):

| classification | envelope carries stderr | `grep -c WRAPPER_VERDICT_MARKER @cmd_*` | `grep -c 'stdout line 4000'` |
|---|---|---|---|
| `generic` (`@cmd_a0a4cf7f`) | **yes**, in full | **0** | 1 |
| `test` (`@cmd_a0a4a396`) | no | **0** | 1 |

The stdout control returns `1` in both, proving the buffer is populated and the selector works.

**The `generic` row is the finding.** Its envelope carries the stderr line verbatim, so the
stream demonstrably reached the server — and the buffer holding that same run still answers
`0`. The loss is at the read surface, not at capture, and it is not specific to the `test`
classification that
`docs/issues/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md`
reports.

## Environment

`experiments` at `93db4b90`, codescout `v0.15.0`, 2026-09-14.
Main checkout `/home/marius/work/claude/codescout`.

## Root cause

Established by reading both call sites, not inferred: `grep.rs` and `read_file.rs` each project
`BufferEntry` to `.stdout` at the point of materialization. Everything upstream is correct.

## Evidence

### E1 — a sibling bug's measurement was misread because of this

`docs/issues/2026-09-14-run-commands-test-envelope-drops-the-stderr-….md` rejected its
hypothesis 3 ("stderr is never captured for test-type runs") on the grounds that cargo's own
`Compiling …` lines are stderr and were present in the buffer. The verdict was right and the
reason was not: `mutation-probe.sh` runs its command as `( cd "$TREE" && "$@" ) 2>&1 | tee`, so
cargo's stderr arrives as the **script's stdout**. The buffer held those lines because they
were stdout by then — not because buffer reads return stderr.

That is worth recording as evidence rather than as a correction: the defect actively produced a
plausible wrong explanation in a careful investigation that had the right answer in hand.

## Fix

Not implemented. Three shapes, and the choice is a contract decision across three tools rather
than a local edit — which is why this was not folded into the sibling fix:

1. **Concatenate at read time** — simplest, and it silently changes line numbering for every
   existing `sed -n 'N,Mp' @cmd_*` caller. The slicing contract is in `read_file`'s own hint
   text.
2. **A labelled section** — append the stderr behind a self-describing separator, the way
   `truncation_marker` already does for a capped buffer. Same precedent, same file, and it
   keeps a bare `grep` working while making the boundary visible.
3. **A second handle** — store stderr under its own `@cmd_*` id and name it in the envelope.
   No existing read changes; costs a field and a round trip.

Shape 2 looks right on the precedent alone, but it is a guess until someone reads the callers.
**Do not resolve this by deleting `BufferEntry.stderr`**: it is the only copy of the stream
once the envelope is rendered, and on `test`/`build` shapes the envelope's copy is bounded.

## Tests added

None — nothing is fixed yet. A test pinning one of the three shapes before anyone has chosen
would be what `docs/adrs/2026-09-14-state-the-property-not-the-snapshot.md` argues against.

## Workarounds

For a command whose stderr you must not lose, and where the envelope's own copy is absent or
bounded: run it through native `Bash`, which returns both streams and is permitted here.

Do **not** read a `0` from a buffer grep as evidence of absence without first asking which
stream the string would have been on.

## Resume

Read the callers of `read_from_buffer` and `grep`'s buffer branch, decide between the three
shapes above, and check whether `@file_*` and `@tool_*` handles share the projection (they use
the same `get()`, but only `@cmd_*` entries ever have a non-empty `stderr`).
