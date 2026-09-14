---
id: 4c433eb615bedf68
kind: bug
status: fixed
title: 'BUG: a buffer query''s stderr was gated behind needs_summary, so a 2-byte query got nothing and a 10 KB query got everything'
tags:
- cluster/capped-result-presented-as-complete
- run-command
- progressive-disclosure
- output-buffer
---

## Summary

> **CORRECTION 2026-09-14, and it falsifies this file's own FILENAME.** The stored stderr is
> **recoverable and always was**: a `@cmd_*` handle takes an **`.err` suffix**.
> `OutputBuffer::get_with_refresh_flag` (`src/tools/output_buffer.rs:289`) resolves the entry
> with `id.strip_suffix(".err")`, and `run_command`'s ref-interpolation path
> (`:674`) selects the stream on `token.ends_with(".err")`. Verified live on
> `@cmd_a11f5743`: `grep -c STDERR_ONLY_TOKEN @cmd_a11f5743.err` returns `1`.
>
> So *"the stored stderr reaches nobody"* is false, and was false before this was filed.
> **The filename still says it, and that is now a deliberate choice rather than a cost.** It
> was kept on archiving (2026-09-14) because the seven inbound citations were being repointed
> for the move anyway, so the slug could have been corrected for free — and was not, because
> renaming a record whose whole content is *"this claim was wrong"* costs the next reader the
> one token they would search for. The claim is answered in this block, immediately, above the
> body that makes it. Read the filename as a question this file answers, not as its finding.
>
> **Three sessions concluded the stream was unrecoverable on one evening and none tested a
> suffix that lives four lines from the struct field they were writing about.** The reason is
> the finding, not the mistake: `.err` appears on **no agent-facing surface** — not
> `get_guide("progressive-disclosure")`, not any `src/prompts/` slice, not
> `.codescout/system-prompt.md`. A mechanism that ships, works, and is undiscoverable by the
> agents it exists for produces confident wrong conclusions in the same direction from
> independent readers. Found by sessionId `f0b1a4c7`-adjacent peer `40130`; the
> silently-wrong-stream half is filed separately by them.

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

**And "all three call sites" was still the wrong population** — see the correction block in
§ *Summary*. Both filings searched for **consumers of the field** and neither searched for
**how a handle is resolved**, where `strip_suffix(".err")` sits four lines from the struct
whose field the bug is about. Two wrong root causes on one file, from two sessions, both
produced by scoping the search to the shape of the answer already assumed.

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
## Fix provenance

- **SHA:** `9b6f4713`
- **patch-id:** `b4ad8f93f7bf28bd6452123f4a4e6562aa5ced22`

Gate 2026-09-14: `fmt 0`, `clippy 0`, `lean 0`, `default 0` (9805 passed, 92 ignored). Both
tests read **by name** out of both lanes — `grep -c` returned `2` for each, so neither is a
default-lane-only assertion.

**Mutation-verified at the guarded site.** `scripts/mutation-probe.sh --strict`, reverting the
short-output path's `&buffer_stderr` to `&raw_stderr` — the precise pre-fix behaviour:

```
177 passed; 2 failed
  run_command_buffer_only_within_limit_no_truncation_fields
  buffer_query_below_summary_threshold_still_surfaces_stored_stderr
```

Both messages **rendered**, and the second printed the defect verbatim —
`Object {"exit_code": Number(1), "stdout": String("0\n")}`, a bare count with no stderr field.
That is the point of reading a probe's output rather than its colour: a kill proves the
assertion fires, never that the message it emits is reachable or legible.

**The probe's own verdict line was dropped by the tool reading it, which is this bug.**
`mutation-probe.sh` writes `KILLED`/`SURVIVED` with `echo … >&2`, so they are not in the
`tee`'d `$RUNLOG` — they are the process's stderr, stored on the buffer entry and invisible to
`grep @cmd_*`. The MCP server was still running the pre-fix binary. The verdict was recovered
from the exit code instead: `--strict` remaps *only* INCONCLUSIVE to `3`, the probe exited
`101`, and 179 tests executed — so not the zero-test branch, therefore KILLED. That is the
channel `--strict` exists to preserve, used for the first time on the bug that motivated it.
## Workarounds

**Use the `.err` suffix.** `run_command("grep PATTERN @cmd_abc.err")` searches the entry's
stored **stderr**. This shipped long before the bug was filed and is the correct answer to
the whole question; it is documented nowhere an agent reads, which is why three sessions
missed it.

**Do NOT use `.err` with codescout's `grep` tool or with `read_file`.** Both accept the token,
resolve it through the same `get()`, and serve **stdout** — silently, with no error, and
`grep` reports a line number from the wrong stream. Verified 2026-09-14 on `@cmd_a11f5743`:
`grep(pattern="STDERR_ONLY_TOKEN|stdout line 4000", path="@cmd_a11f5743.err")` returned
`4000: stdout line 4000`; `read_file("@cmd_a11f5743.err")` returned 4000 lines of stdout.
That is a separate defect with a separate mechanism — accepted token, wrong target — owned by
peer session `40130` and filed by them.

**Superseded:** an earlier revision of this section said to run the command through native
`Bash` for a stream you must not lose. That is still true and is now the third-best answer;
prefer `.err` through `run_command`.

Do **not** read a `0` from a codescout-`grep` over a buffer as evidence of absence without
first asking which stream the string would have been on — and note that appending `.err` does
not fix it there.
## Resume

**Closed for the reported surface.** What is deliberately left open, and is not debt from
this fix:

- `grep.rs:974` and `read_file.rs:286` remain stdout-only. Changing either is a read-contract
  decision (line numbering, `sed -n 'N,Mp'` slicing) with no reported instance behind it.
  File a new bug with a reproduction before touching them — the three shapes in § *Fix* are
  drafts, not a plan.
- `@file_*` and `@tool_*` handles share the same `get()`, but only `@cmd_*` entries are ever
  stored with a non-empty `stderr`, so the projection is not lossy for them.
