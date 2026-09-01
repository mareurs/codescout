---
id: '2b2dec92f0996cf3'
kind: bug
status: fixed
title: unfiltered_output_lines counts the source, not the buffer — and greps over the truncated buffer answer silently-partially
tags:
- run_command
- progressive-disclosure
- buffers
- observability
- cluster/capped-result-presented-as-complete
---

# `unfiltered_output_lines` describes the stream, sits next to a handle, and the truncated buffer it points at answers greps silently-partially

**Found:** 2026-08-27, from `claude-plugins`, by a consumer of this API.
**Affects:** `src/tools/run_command/output.rs` — the `unfiltered_ref` path.
**Relationship to prior work:** direct follow-up to
`docs/issues/archive/2026-08-26-unfiltered-output-ref-carries-no-size-signal.md`
(fixed `c172fe10`). **That fix is not wrong and should not be reverted.** This bug is
about the gap it left.

## Summary

`c172fe10` added `unfiltered_output_lines`, deliberately computed on the raw tee capture
*before* inline-storage truncation — "so a truncated buffer still reports its true size,
not just what fit inline." Its regression test
(`unfiltered_output_line_count_survives_inline_truncation`) pins exactly that.

The consequence is a response where a number describing the **stream** sits adjacent to a
handle serving a **truncated buffer**, and **no field anywhere reports how many lines the
handle will actually serve.** `unfiltered_truncated: true` says *that* it is short, never
*how* short. Learning the served size still costs a `wc -l @ref` — the blind round-trip
the parent bug set out to eliminate.

The harm is not the misread number. It is that **greps over the truncated buffer return
partial results with no in-band marker**, so a `grep -c` past the cut returns `0`, which
is byte-identical to *genuinely absent*.

## Symptom (Effect)

```
run_command("seq 1 20000 | awk '{printf \"ROW-%05d padding padding padding padding\\n\", $1}' | tail -5")
→ { "stdout": "ROW-19996 …\nROW-20000 …",
    "unfiltered_output": "@cmd_426b932a",
    "unfiltered_output_lines": 20000,
    "unfiltered_truncated": true }
```

Reading the handle back:

| field / probe | value |
|---|---|
| `unfiltered_output_lines` | `20000` |
| `wc -l < @cmd_426b932a` | **238** |
| `head -1` | `ROW-00001 …` |
| `tail -1` | `ROW-00239 …` |
| `grep -c '^19999$'` | `0` |
| `output_id` | **absent** |

That last row is the trap's second half. The pipeline's *final* output was 5 lines, so it
fit inline and **no `output_id` buffer was minted** — leaving the truncated handle as the
only handle in the response. The trimmer that made the output small is the same thing
that removed the reliable way to inspect it.

Two further facts, measured, that the fix should not disturb:

- **The tee captures the pipeline minus its final stage.** `seq 1 1000 | awk '{…}'` put
  `seq`'s `1..1000` in `unfiltered_output` while `output_id` held the awk result;
  `seq | awk | tail -5` put the *awk* rows there.
- **The LHS runs once, not twice.** `( echo X >> log; seq 1 300 ) | tail -2` appended to
  `log` exactly once — it is a tee, not a re-execution. No double-side-effect hazard.

## Reproduction

```
seq 1 20000 | awk '{printf "ROW-%05d padding padding padding padding\n", $1}' | tail -5
```

Then `wc -l` the returned `unfiltered_output` ref and compare against
`unfiltered_output_lines`. Any unbounded producer into a trimmer reproduces it.

## Environment

- Claude Code 2.1.247, codescout `experiments`, measured 2026-08-27.

## Root cause

Two facts, each individually defensible:

1. `unfiltered_output_lines` is computed pre-truncation, by design (`c172fe10`).
2. The buffer the handle resolves to is post-truncation and carries **no in-band record**
   that it was cut.

Nothing in the response names the served line count, and nothing in the *buffer* marks its
own end as artificial. So every downstream read — `grep`, `wc`, `tail`, a slice — silently
answers about a prefix while presenting as an answer about the whole.

The inline path already solves the analogous problem: an over-budget `stdout` summary
embeds a literal `--- 970 lines omitted ---` marker mid-output. The mechanism exists in
this codebase; it just does not extend to the unfiltered buffer.

## Why the IL-3 guard did not stop this — and why that makes the reporting fix load-bearing

The obvious objection to this bug is "IL-3 already forbids that pipe; fix the caller."
Measured 2026-08-27, the guard **cannot** catch the shape that produced the incident.

The gate reads the LEFT side of the pipe against a **name list**. Its own block message
enumerates it: `cargo/npm/pnpm/yarn/python/pytest/go/mvn/gradle/rg/fd`, recursive `grep`,
`find` without `-maxdepth`, and `git` without an output limiter.

A **local script or executable is on no list**, so it is classified bounded and allowed:

| command | verdict |
|---|---|
| `git log --oneline \| head -5` | **BLOCKED** (IL3 violation) |
| `./scripts/check-versions.sh \| tail -3` | **allowed**, tee attached |
| `./tests/run-all.sh 2>&1 \| tail -30` | **allowed** — 624 lines, truncated buffer, the incident |

So the guard blocks `git log --oneline` — roughly a screen of output — while waving
through a script that runs 39 test suites. That is not a bug in the name list; a
name list cannot know what `./tests/run-all.sh` does. It is the inherent limit of
classifying by producer name.

Two consequences for this bug:

1. **"The caller should have obeyed IL-3" is not a mitigation here.** The caller who
   writes `./my-script | tail` receives no block, no warning, and no indication they
   are on the tee path at all. The first sign is a field named
   `unfiltered_output_lines` — which reports a count the buffer does not have.
2. **Truncation is the only remaining signal, so it must be trustworthy.** For an
   unrecognized LHS, the guard has already declined to judge; `unfiltered_truncated`
   is then the *sole* evidence that the producer was unbounded after all. That moves
   Option C from a nicety to the actual fix — the one place the system can still tell
   the truth about a partial read.

A fail-closed guard (treat an unrecognized executable as unbounded) is the alternative,
but it would block a large amount of legitimate `./script | head` use for a class of
command that is usually small. Fixing the reporting is the cheaper and more honest
correction: let the pipe through, and make the resulting buffer say what it is.
## Evidence — the wrong conclusion this produced downstream

In `claude-plugins`, a session ran `./tests/run-all.sh 2>&1 | tail -30` and grepped the
returned unfiltered handle:

- `grep "▶" @cmd_…` → **17** suite headers, last at buffer line 231
- `grep "agent-guide" @cmd_…` → one line, from a permissions test
- `grep -c FAIL @cmd_…` → `0`

Conclusion drawn and stated to the user: *"`run-all.sh` runs only `tests/test-*.sh`; the
hooks-dir suites are permission-checked, not executed."*

**False.** Line 12 of that script globs `codescout-companion/hooks/*.test.sh`; a clean run
reports **39** suites and the suite in question is among them. The response had said
`unfiltered_output_lines: 624` over a buffer holding ~231 lines, with
`unfiltered_truncated: true` alongside.

Recorded as `claude-plugins:roster-audit-session-log:F-14`.

**The upstream cause there was the consumer's**, and this bug does not claim otherwise:
piping unbounded output into a trimmer violates Iron Law 3, and both compliant paths give
a complete answer (redirect to a file; or run bare and grep `output_id`, verified complete
at 1000/1000 lines). The reason to fix this anyway is that **an IL-3 violation is the only
way to reach this code path at all** — the tee exists precisely to serve piped-to-trimmer
commands. So "the caller broke IL-3" describes every user of the feature, not a
misbehaving subset, and cannot be the reason to leave the reporting unsafe.

## Fix options

**Option A — name the served count.** Add `unfiltered_buffered_lines` (post-truncation)
beside `unfiltered_output_lines` (pre-truncation). Cheap, additive, leaves `c172fe10`'s
field and test intact. But it is one more adjacent number to read, and this bug exists
because a reader took an adjacent number for a description of the handle.

**Option B (recommended) — make the buffer self-describing.** Append a sentinel as the
final line of the truncated buffer:

```
--- TRUNCATED: 238 of 20000 lines buffered; 19762 omitted ---
```

The warning then travels *with the data*: `tail` shows it, `wc -l` counts it, any slice
trips over it, and a grep that returns nothing at least sits next to something explaining
why. Same device the inline summary path already uses.

**Option C — pair B with a truncation echo on reads.** When a tool resolves an `@ref`
whose buffer is flagged truncated, attach a one-line notice to *that tool's* result. This
is the only option that makes `grep -c … → 0` non-silent, and that is the failure mode
that actually produced a wrong conclusion.

A and B are independently landable. C is the one that closes the observable.

## Tests to add

- A truncating fixture asserting `unfiltered_buffered_lines` equals `count_lines` of the
  stored buffer, and that it **differs from** `unfiltered_output_lines`. Assert
  `unfiltered_truncated` is present first so it cannot pass vacuously on a fixture too
  small to truncate — the same guard
  `unfiltered_output_line_count_survives_inline_truncation` already uses.
- For B: the sentinel is the buffer's last line and names both counts.
- For C: a grep against a truncated ref carries the truncation notice. **This is the
  load-bearing one** — the only test that would fail today for the reason the bug was
  actually reported.

## Fix

Shipped on `experiments` in **`c1cb0022`** — patch-id
**`a7f22eb241771dcb37fba6c92aa8cba2f14b479f`**. **All three options landed**, A and B
because they are cheap and additive, C because it is the only one that closes the
observable this bug was reported for.

### The primitive underneath all three

`BufferEntry.truncated: Option<Truncation>`, recording `kept_lines` and `total_lines`
at **store** time. This is what the fix actually needed and what the file did not name:
`unfiltered_truncated` lives on the *response*, is minted once, and is gone by the next
turn — while the handle keeps working for the rest of the session. A caller who greps
the ref three turns later had nothing to consult. The flag had to move onto the buffer.

Both numbers are stored rather than one derived at read time from the buffer's own
bytes. Deriving `kept_lines` by counting stored lines and discounting a sentinel would
make the report a function of the artifact it describes — and the sentinel is written by
this same mechanism, so an off-by-one would be invisible in exactly the direction that
matters.

### Per option

- **A — shipped.** `unfiltered_buffered_lines` names what the handle will serve.
  `unfiltered_output_lines` keeps describing the stream; `c172fe10`'s deliberate choice
  and its regression test are untouched, as this file asked.
- **B — shipped.** `truncation_marker()` builds the sentinel, appended as the buffer's
  final line with a trailing newline. The trailing newline is not cosmetic: without it
  the sentinel's own line has no terminator, `wc -l` lands back on the content count,
  and a test asserting the two differ would pass for the wrong reason.
- **C — shipped, in both read paths.** `run_command` attaches `buffer_truncated` via a
  new `OutputBuffer::truncation_notices_in`; `read_file` attaches it at its buffer-ref
  branch. Reported as a JSON field rather than prepended to stdout, because a count or a
  hash is a value the caller parses and prose in front of it corrupts the answer.

  `truncation_notices_in` is deliberately **not** folded into `resolve_refs`' return
  tuple, which is destructured at ~20 call sites. Widening it would have spent the diff
  on churn unrelated to the defect.

  **The `read_file` half needed a second commit — `21507a26`, patch-id
  `1bc96ae356d6741c08dbcd5a30c462a8e87ab06b`.** `c1cb0022` attached the field and
  stopped; `format_read_file` builds its text from a fixed set of keys and drops every
  other field, so the notice sat in the JSON and reached no reader. An **inert field**,
  which is worse than an absent one because the code reads as done.

  It was found by a live probe against the rebuilt server, not by a test, and the reason
  is worth keeping: `run_command` returns its JSON straight through, so asserting on
  `.call()`'s `Value` is the whole surface there — `read_file` has a *second*,
  text-rendering surface on top, and nothing looked at it. The added test asserts on
  `format_read_file`'s output, and on the notice being the rendered SECOND line rather
  than merely present, so a later "just append it" refactor cannot silently undo the
  head placement. The fix is applied at a thin wrapper, not inside the body: the body
  has five return paths, the probe hit one of them, and threading it through only that
  one would have been just as invisible on the other four.

### One correction to this file's own § Tests to add

It asked for a test asserting `unfiltered_buffered_lines` **equals** `count_lines` of
the stored buffer. Landing A and B together makes that false by one — the stored buffer
also holds the sentinel. The shipped assertion is `stored_lines == served + 1`, which is
strictly stronger: it pins the relationship between the two options rather than either
alone, and would fail if a future change dropped the sentinel *or* mis-counted the
field. (Same shape as the sibling bug's Finding 1, where a prescribed fix did not move
its own cited example: a plan written before the code is a hypothesis about it.)

### CI — skip-listed on arrival, not after a red run

The four new tests reuse `seq 1 20000 | grep zzz` and `printf 'a\nb\nc\n' | grep zzz`
verbatim from the two tests this bug is a follow-up to. CI's group 6 records that those
commands **hang under wine 9.0** and are killed by `run_command`'s own 30s timeout. The
cause is a property of the COMMAND, not of what the test asserts, so inheriting the
shape inherits the hang — all four were added to the wine lane's `--skip` list in the
same commit, with a note tying them to their two predecessors' un-skip protocol.

### Tests

4 added. Each of the three positive tests was verified to **fail** with its option
reverted — a three-way revert failed exactly those three, each attributable — while
`a_complete_buffer_carries_no_sentinel_and_no_notice` stayed green, which is what shows
the warning does not fire unconditionally and that no synthetic content is injected into
a complete buffer. Each positive test asserts `unfiltered_truncated` **first**, so a
fixture too small to truncate cannot pass it vacuously.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` —
**4589 passed, 0 failed**.

## Workarounds (for consumers, today)

- Obey Iron Law 3. Redirect to a file and grep the file, or run bare and grep `output_id`,
  which is not subject to this cap.
- If you must read an unfiltered ref, `wc -l` it first and compare against
  `unfiltered_output_lines`. Disagreement means every read of it is a prefix.
- Never conclude *absence* from a grep over a buffer whose response said
  `unfiltered_truncated: true`.

## References

- `src/tools/run_command/output.rs` — `handle_successful_output`, `unfiltered_ref`
- `docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md`
  — **sibling, same session. FIXED 2026-08-27 in `18f8f9d1` (patch-id `16a9abc0`).** The IL-3 guard's error text tells the caller to "rerun
  bare and query the `@cmd_*` buffer"; when that buffer is truncated and fed to
  `git patch-id`, the result is a syntactically perfect **wrong** hash. That is this
  bug's failure mode applied to a hash function, where partial input yields a confident
  wrong output rather than a partial one — and it is the strongest argument for Option C.
- `docs/issues/archive/2026-08-26-unfiltered-output-ref-carries-no-size-signal.md` —
  parent bug; its § Fix item 2 is the deliberate decision this one qualifies
- `docs/superpowers/plans/2026-03-04-unfiltered-output-capture.md` — original design,
  §5 "look wider"
- `claude-plugins:roster-audit-session-log:F-14` — the downstream incident
