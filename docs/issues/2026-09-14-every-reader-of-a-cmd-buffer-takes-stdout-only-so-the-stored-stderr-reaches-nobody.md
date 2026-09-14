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

**Three surfaces read a `@cmd_*` handle, and the two obvious ones take `.stdout` alone:**

- `src/tools/grep.rs:974` — `.get(raw_path)?.stdout`
- `src/tools/read_file.rs:286` — `read_from_buffer`, `.get(path)?.stdout`

So `grep PATTERN @cmd_abc`, `read_file("@cmd_abc")` and `sed -n '1,50p' @cmd_abc` all answer
about **stdout only**, while presenting themselves as reads of the command's output. A search
that finds nothing has not shown the output does not contain it.

**The third surface is `run_command` itself, and it reads exactly the field the other two
drop** — `src/tools/run_command/output.rs`, which resolves the `@cmd_`/`@file_` token out of
the query string, fetches the entry and emits `e.stderr` with `stderr_shown`/`stderr_total`
counters. **It was gated on `needs_summary(&raw_stdout, &raw_stderr)`**, i.e. combined output
over ~10 KB (`src/tools/command_summary.rs:219`).

**That gate is anti-correlated with need.** `grep -c MARKER @cmd_abc` returns `"0\n"` — two
bytes — so the predicate is false, control falls to the short-output branch, and the stored
stderr is never consulted. A query returning 10 KB of matches gets the stderr in full. The
reader is handed the stream when their query already drowned them, and denied it when their
query returned `0`: the one case where absence and loss are indistinguishable.

This is `CLAUDE.md` § *Testing Discipline*, "loudness is a property of a PATH" — but a
sharper instance than first filed. The remedy is not missing. It exists, is correct, has a
response shape and three tests, and is unreachable by exactly the failure it exists for.
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
`docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md`
reports.

## Environment

`experiments` at `93db4b90`, codescout `v0.15.0`, 2026-09-14.
Main checkout `/home/marius/work/claude/codescout`.

## Root cause

Established by reading all three call sites — **the first filing read two and concluded from
them.** `grep.rs` and `read_file.rs` do each project `BufferEntry` to `.stdout`, as reported.
But the claim built on that, *"nothing ever reads that field"*, was false: `output.rs`
reads it.

The real cause is the gate, not a missing reader. The lookup sat inside

```rust
if needs_summary(&raw_stdout, &raw_stderr) {
    if buffer_only {
        // ... output_buffer.get(tok).map(|e| e.stderr)
```

and `needs_summary` is `(stdout.len() + stderr.len()) / 4 > MAX_INLINE_TOKENS`. Both
short-output branches below it (`output.rs`, the byte-budget arm and the plain arm) used
`raw_stderr` — **the query command's own stderr**, which on a buffer read is empty — and
never consulted the referenced entry.

**How the first filing reached a wrong cause from a correct reproduction:** the reproduction
ran `grep -c`, whose output is two bytes, so every observation was taken on the one path where
the mechanism is absent. Nothing in the output distinguishes "no reader exists" from "the
reader is gated out", and the first hypothesis was never re-tested at a size that crosses the
threshold. `CLAUDE.md` § *Bug Tracking*: run the reproduction before reading the fix plan —
this is that law applied to a plan I wrote myself.
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

**Implemented 2026-09-14.** Not one of the three shapes below — they were drafted on the
belief that no reader existed, and all three would have redesigned `grep`'s and
`read_file`'s read contract to duplicate a mechanism already shipping.

The fix is a **hoist**: the entry-stderr lookup moves out of the `needs_summary` arm to
before the branch, and all three `buffer_only` paths use it. One line of logic changes
significance; the response shape, the 20-line cap and the `stderr_shown`/`stderr_total`
counters are the existing ones.

Three deliberate limits, each for a stated reason:

- **`buffer_stderr` is NOT fed into `needs_summary`.** That predicate decides whether a new
  buffer ref is minted; widening its input would move the buffering threshold for every
  caller. The fix changes what a buffer query *reports*, never what it *stores*.
- **The 20-line cap applies only on the `buffer_only` path.** An ordinary command's short
  stderr stays uncapped, byte-identical to before — the cap is a property of reading someone
  else's stored stream, not of stderr generally.
- **A latent under-budgeting was corrected in passing:** the byte-budget arm subtracted
  `raw_stderr.len()` from the stdout budget, which on a buffer query is `0` and was never the
  text being emitted. It now subtracts what is actually emitted.

`grep`'s and `read_file`'s buffer branches are **unchanged and still stdout-only.** That is
now a documented limit rather than a defect: `run_command` is the surface that carries the
stderr, and the § *Workarounds* note below still applies to the other two.

<details>
<summary>The three shapes proposed before the third reader was found (not taken)</summary>

1. **Concatenate at read time** — silently changes line numbering for every existing
   `sed -n 'N,Mp' @cmd_*` caller.
2. **A labelled section** behind a self-describing separator, as `truncation_marker` does.
3. **A second handle** — store stderr under its own `@cmd_*` id.

All three were read-contract changes to `grep`/`read_file`. None was needed.
</details>

**Do not resolve the remaining stdout-only reads by deleting `BufferEntry.stderr`**: it is the
only copy of the stream once the envelope is rendered, and on `test`/`build` shapes the
envelope's copy is bounded.
## Tests added

Two, in `src/tools/run_command/tests.rs`.

**`buffer_query_below_summary_threshold_still_surfaces_stored_stderr`** — new. Pins the
reproduction itself: `grep -c NOSUCHTOKEN @cmd_*` against an entry holding a marked stderr
line. It asserts the count is `0` **first**, as a control — without it the stderr assertion
would also pass on a response that accidentally crossed the summary threshold and took the
other branch, which is the branch that already worked.

**`run_command_buffer_only_within_limit_no_truncation_fields`** — amended, and the amendment
is the more interesting half. That test already **constructed the failing case** (30 stdout
lines, 15 stored stderr lines), and its own comment already named the gate —
*"needs_summary returns false, so we fall through to the short-output branch"*. It was green
throughout. Its three assertions were `truncated`, `stdout_shown` and `output_id` all
`is_none()`: absence assertions, every one true whether or not the stderr is surfaced.
`CLAUDE.md` § *Testing Discipline* law one — monotone under removal — with the refuting case
already in the fixture and no assertion pointed at it. A positive `contains("err15")` is what
discriminates.

**Mutation-verified**, not merely added: see § *Fix provenance*.
## Workarounds

**Fixed for `run_command`.** A buffer query now carries the entry's stored stderr at any
size, capped at 20 lines with `stderr_shown`/`stderr_total` when there is more.

**Still true of `grep` and `read_file`:** both take `.stdout` alone from a `@cmd_*` handle.
So `grep PATTERN @cmd_abc` issued through **codescout's `grep` tool** still searches stdout
only. Routing the same query through `run_command("grep PATTERN @cmd_abc")` — which is the
form `server_instructions` Iron Law 3 actually prescribes — now surfaces the stderr beside
the result.

Do **not** read a `0` from a codescout-`grep` over a buffer as evidence of absence without
first asking which stream the string would have been on.
## Resume

**Closed for the reported surface.** What is deliberately left open, and is not debt from
this fix:

- `grep.rs:974` and `read_file.rs:286` remain stdout-only. Changing either is a read-contract
  decision (line numbering, `sed -n 'N,Mp'` slicing) with no reported instance behind it.
  File a new bug with a reproduction before touching them — the three shapes in § *Fix* are
  drafts, not a plan.
- `@file_*` and `@tool_*` handles share the same `get()`, but only `@cmd_*` entries are ever
  stored with a non-empty `stderr`, so the projection is not lossy for them.
