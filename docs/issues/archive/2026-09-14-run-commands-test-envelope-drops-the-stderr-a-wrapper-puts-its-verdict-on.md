---
id: 5b9e544f1882f56e
kind: bug
status: archived
title: 'BUG: run_command''s test envelope drops the stderr a wrapper puts its verdict on, and the surviving fields read as a different verdict'
tags:
- cluster/capped-result-presented-as-complete
- run-command
- probes
- progressive-disclosure
---

## Summary

`run_command` renders a command's result through one of several shapes. When it classifies the
run as `type: "test"`, the envelope carries `exit_code`, `output_id`, `passed` and `ignored`
and **no `stderr` field at all**. When it classifies the same overflowing output as
`type: "generic"`, `stderr` is returned in full.

For any wrapper script that runs a test command and reports its own finding on stderr, the
finding is dropped. `scripts/mutation-probe.sh` is exactly that script, and the loss is total
rather than partial: its verdict is deliberately **not** encoded in the exit status —
`docs/PROBES.md` pins that, and the script says so at `:288` — so `exit_code` cannot carry it
either.

The result an agent is handed is `{"type":"test","exit_code":0,"passed":0}`, which reads as a
clean run, for an invocation the tool itself classified **INCONCLUSIVE** and explained in four
lines.

## Symptom (Effect)

2026-09-14, mutation-testing a guard added this session:

```
scripts/mutation-probe.sh --file docs/templates/session-log.md \
  --find '<literal>' --replace '<literal>' \
  -- cargo test --lib prescriptive_recipes_teach_append_entrys_one_call_form
```

returned:

```json
{"type": "test", "exit_code": 0, "output_id": "@cmd_a03ab2f5", "passed": 0}
```

What the script actually wrote to stderr, and what never arrived:

```
mutation-probe: NOTE — 1 other .rs file(s) are dirty in the shared tree and
  are NOT carried into the isolated worktree, which builds them at HEAD.
mutation-probe: ARMED  mode=isolated  file=… tree=…
mutation-probe: INCONCLUSIVE — the runner started and selected 0 tests, so nothing
  could have caught this mutation. … the test is in a file this worktree built at
  HEAD because it is uncommitted — only the mutated file is carried across.
```

Every line is correct, and the `NOTE` is the precise diagnosis of why the run proved nothing.
The only surviving signal was `passed: 0`, which is indistinguishable at a glance from a filter
that selected a fast test.

**Why this is worse than a missing field.** `passed: 0` with `exit_code: 0` is the byte-identical
rendering of a *surviving mutant* — the finding the probe exists to produce. So the envelope does
not merely omit a verdict, it supplies a plausible wrong one. The reader is one habit away from
recording "SURVIVED" for a run in which no test executed.

## Reproduction

Two runs, same session, differing only in classification:

1. **`type: "generic"`** — 4000 stdout lines plus one stderr line, overflowing into a buffer:

   ```
   python3 -c "
   import sys
   for i in range(4000): print('stdout line', i)
   print('VERDICT: this is the stderr line that matters', file=sys.stderr)"
   ```

   → envelope carries `"stderr": "VERDICT: this is the stderr line that matters\n"` **in full**,
   alongside an elided stdout (`--- 3970 lines omitted ---`). So overflow alone does not drop
   stderr.

2. **`type: "test"`** — any `cargo test` invocation, this session's five observations
   (`@cmd_a03ab2f5`, `@cmd_a03e0638`, `@cmd_a03fc2b2`, `@cmd_a0432d57`, `@cmd_a04504ad`): **no
   `stderr` key present in any of them.**

The wrapper's lines are absent from the `@cmd_*` buffer as well:
`grep -c mutation-probe @cmd_a03ab2f5` → `0`, with `grep -c 'running 0 tests' @cmd_a03ab2f5` →
`1` as the control proving the buffer is non-empty and the selector works.

> **CORRECTION, 2026-09-14, applied in place rather than appended.** This paragraph originally
> read *"absent from the `@cmd_*` buffer **too**"* and closed *"Cargo's own stderr (`Compiling
> …`) **is** in that buffer, so the loss is not 'stderr is never captured'."* Every measurement
> above is real and reproduces. Both inferences drawn from them were wrong, for two different
> reasons, in adjacent sentences:
>
> 1. **That `too` is a layer change wearing a continuation.** The runs above differ only in
>    classification, and classification genuinely explains the ENVELOPE difference. It does not
>    explain this one: `read_from_buffer` (`src/tools/read_file.rs`) and `grep`'s buffer branch
>    resolve `.stdout` alone, for every classification. A `generic` run whose envelope carries
>    the stderr line IN FULL still answers `0` here — the control nobody had a reason to
>    construct, because the section's framing gave none. Filed as `4c433eb615bedf68`.
> 2. **The closing sentence's premise was relabelled in transit.** `mutation-probe.sh` runs its
>    command as `( cd "$TREE" && "$@" ) 2>&1 | tee`, so cargo's `Compiling …` lines are the
>    SCRIPT's stdout by the time anything stores them. They prove nothing about stderr capture.
>    The verdict it supported (§ *Hypotheses tried* 3) still stands on other grounds; the reason
>    given here does not.
>
> Left visible rather than silently rewritten: three sessions read classification as the
> discriminator for both layers, and the wording that carried them there is the artifact.

## Environment

`experiments` at `a9860298`, codescout `v0.15.0`, 2026-09-14T17:4x+03:00. Main checkout
`/home/marius/work/claude/codescout`; the probe's worktree
`codescout.worktrees/mutation-aa272bed-…`.

## Root cause

**Established 2026-09-14 by the session that fixed it, and it is NOT one of the three readings
this file first offered** — those are kept below because ruling them out is what located this one.

The `test` path does not drop the stream, does not cease capture, and does not parse only the
harness-shaped portion. `raw_stderr` is captured in full and stored
(`ctx.output_buffer.store(cmd, raw_stdout, raw_stderr, exit_code)`), and `summarize_test_output`
receives it and reads it — but only to sum cargo's count lines out of a `combined` string. It
emits derived counts and never the stream. `summarize_generic`, reached from the same three-way
`match`, does emit it. The loss is **one missing field in two of the three summarizers**, not a
stream that went anywhere.

**A second, larger loss sits underneath it, and it is why hypothesis 2 measured the way it did.**
`BufferEntry` carries a `stderr` field; every reader of a `@cmd_*` handle takes `.stdout` alone —
`src/tools/grep.rs` and `read_file.rs`'s `read_from_buffer`. Reproduced with a synthetic script
(4000 stdout lines plus one marked stderr line):

| classification | envelope carries stderr | `grep -c MARKER @cmd_*` | stdout control |
|---|---|---|---|
| `generic` | **yes**, in full | **0** | 1 |
| `test`    | no                | **0** | 1 |

So the buffer-level loss applies to `generic` too — unnoticed only because `generic` also puts
stderr in the envelope. `BufferEntry.stderr` is written by `store()` and read by nobody:
`CLAUDE.md` § *Testing Discipline*, "loudness is a property of a PATH". Filed separately rather
than folded in here — different defect, different blast radius.

**A correction to § Hypotheses tried, hypothesis 3.** Its rejection reasoned that cargo's
`Compiling …` lines are stderr and were present in the buffer, so stderr must be captured. The
premise is right and the inference does not follow: `mutation-probe.sh` runs its command as
`( cd "$TREE" && "$@" ) 2>&1 | tee`, so cargo's stderr arrives as the **script's stdout**. The
buffer held it for that reason, not because buffer reads return stderr — they do not. The
verdict stands; the stated reason does not.

**What does not need the mechanism, because it is a composition rather than a bug in either
part.** `mutation-probe.sh` keeps its verdict out of the exit status on purpose and for good
reasons — `SURVIVED` is a real finding that exits 0, and three of its own fixtures assert
`rc == 0` while testing restoration rather than verdicts. `run_command` renders test runs
compactly, also reasonably. Each is defensible alone; together they make a whole class of
wrapper script unreadable through the tool this repo's CLAUDE.md prefers for `cargo`.

**The class.** `cluster/capped-result-presented-as-complete`: a projection of a result is handed
over with nothing marking it a projection, and the dropped field is the one carrying the
judgement. This instance adds the sharper half — the surviving fields compose into a *plausible
alternative verdict*, so the reader is not stopped by an obvious gap.

## Evidence

### E1 — the same script, same verdict path, small output

With `-- true` (no buffering) the identical script returned its verdict inline:

```json
{"exit_code": 0, "stderr": "mutation-probe: NOTE — 1 other .rs file(s) …\nmutation-probe: ARMED …\nmutation-probe: INCONCLUSIVE — no test-count line …"}
```

Same script, same stderr writes, different envelope. This is what rules out "the script did not
emit" and localises the loss to rendering.

### E2 — `docs/PROBES.md` already warns about the adjacent half

Its `mutation-probe.sh` row ends: *"The exit status is still the test command's, and a verdict is
NOT encoded there — an INCONCLUSIVE run exits 0 when its command did, so **read the verdict
line**."* The instruction is correct and, through `run_command`'s test envelope, unfollowable:
there is no verdict line to read.

## Hypotheses tried

1. **Hypothesis:** stderr is dropped whenever output overflows into a buffer.
   **Test:** reproduction 1 above.
   **Verdict:** rejected — `type: "generic"` overflowed and returned stderr in full.
2. **Hypothesis:** the script's stderr is in the `@cmd_*` buffer and only absent from the envelope.
   **Test:** `grep -c mutation-probe @cmd_a03ab2f5`, with `grep -c 'running 0 tests'` as control.
   **Verdict:** rejected — `0` against a control of `1`.
3. **Hypothesis:** stderr is never captured for test-type runs.
   **Test:** read the buffer's head.
   **Verdict:** rejected — but **the reason first recorded here was void, and it survived two
   passes of correcting this file.** It read *"cargo's own `Compiling …` lines are stderr and are
   present."* `scripts/mutation-probe.sh:233` pipes `( cd "$TREE" && "$@" ) 2>&1 | tee`, so those
   bytes **were stdout** by the time they reached the buffer; their presence is evidence about
   stdout and says nothing about stderr. The verdict stands on other grounds, which are stronger
   than the hypothesis it rejected: `raw_stderr` is captured in full and handed to the summarizer,
   which simply never rendered it. Generalised as `context-injection-session-log:F-11` — *a control
   whose subject was relabelled in transit*.

   **That this line outlived the correction is the third instance of one shape in this file**, and
   worth more than the correction. § *Root cause* was rewritten to the established mechanism and
   § *Reproduction* gained a visible CORRECTION block; both were done by a reader acting on the
   file, and neither pass reached § *Hypotheses tried*, where the same claim sat unqualified. A
   section is corrected where it is being USED, and a rejected hypothesis is the section nobody
   returns to. `OB-12` / `OB-26`.

## Fix

**Implemented 2026-09-14.** `summarize_test_output` and `summarize_build_output` now emit a
`stderr` field, via a shared `summarize_stderr` helper in `src/tools/command_summary.rs`.

| commit | patch-id | what |
|---|---|---|
| `9c2b542f` | `04b8696314d0fcb6fc72923406ff8b490b40e475` | the fix, its tests, and the two cap-probe rows |
| `a5238dc2` | `1cc77378276a0c62d2777ef24462466494904aec` | the reach test asserts absence before value, so its own message renders |
| `4388dd1e` | `835d6f84ca7bb65b77973468663d263f02eae213` | `--strict` on `mutation-probe.sh`, the independent mitigation below |

The patch-id is the durable half: `experiments` is rebased after every ship, so a
cherry-picked commit's original SHA is orphaned while the content hash survives both rebase
and cherry-pick.

**The cheap form named here — "carry `stderr` as the `generic` one does" — was wrong as
written, and the measurement that shows it is the reason this section is not a one-liner.**
`summarize_generic`'s stderr is **unbounded**, and the envelope has a threshold it does not
know about: `needs_summary` is `(stdout.len() + stderr.len()) / 4 > MAX_INLINE_TOKENS`, so
past **~10 KB combined** the whole response is re-buffered into `format_run_command`'s
one-line summary, which carries no stderr.

Bracketed 2026-09-14 on the `generic` path — the derivation rather than a single number,
because the number is what misleads:

| combined stdout+stderr | result |
|---|---|
| 9,024 B | inline, `stderr` field in full |
| 14,304 B | `{"output_id": "@tool_…", "summary": "✓ exit 0  (query @cmd_…)"}`, no stderr |

**~10 KB is a Tuesday, not a pathology.** Any `cargo` run touching a few crates clears it
in diagnostics — which is most runs of the exact command this field exists for. So copying
`summarize_generic`'s shape into the `test` path would have dropped the verdict on the
common case, with the field present and a suite green at small sizes. It also rules out the
repair a larger figure would invite: no threshold-raise works, because the input is
unbounded and the threshold is not. Bounding the field is the only answer.

(An earlier draft of this section cited a single 227 KB datapoint. That was true and priced
the defect about four orders of magnitude too rare. Corrected after sessionId
aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66 re-derived the bracket by a different method; both
sides re-measured here before the change. Recoverable at any size via
`read_file(@tool_*, json_path="$.stderr")`, and loud rather than silent — but not a fix.)

**THE TABLE VARIES THE WRONG AXIS, and saying so is worth more than the table.** The
condition sums **both** streams. Every reproduction in this file is synthetic with ~0
stdout, so all of them measured the stderr-only edge of a two-term predicate. On the command
this bug is actually about, the first term settles it alone: this session's gate run buffered
**831,766 B across 10,263 lines**, 83× the threshold, and a peer's default lane measured
515,322 B. stdout busts it before stderr is consulted at all. So for a workspace-scale
`cargo test` the summarizer is entered on **every** run, including every green one, whatever
the stderr size — "above ~10 KB of stderr" never described the real population.

**And the boundary is the part that explains the confusion.** A narrowly filtered run stays
under the threshold and is returned inline, stderr and all — `cargo test --lib …
command_summary` (49 tests, ~4.5 KB) came back this session with its `stderr` field carrying
cargo's `Compiling` lines, and § E1's `-- true` probe likewise returned its verdict inline.
That is why `mutation-probe.sh` verdicts arrived sometimes and vanished other times, and why
the `type: "test"` classification looked like the discriminator: it is not the gate. The gate
is **combined output volume**; classification only chooses *which* summarizer runs once that
gate has already been crossed, and two of the three then dropped the field.

This lands on the bounded, tail-biased design rather than qualifying it. If the summarizer is
entered on every workspace run, the `stderr` field is not an edge-case rescue — it is the
normal rendering path for the repo's most-run command, which is the load a 20-line tail is
right for and an unbounded copy would have been worst at. (Consequence identified by
sessionId aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66; the inline-boundary half measured here, and
it bounds their stronger claim that every `cargo test` is affected — filtered ones are not,
and never were.)

So the field is **bounded and tail-biased**:

- `STDERR_SUMMARY_LINE_BUDGET = 20`, deliberately the same number as `STDERR_BUDGET` in
  `run_command`'s buffer-query branch — same question, and two independent answers would drift.
- `STDERR_SUMMARY_BYTE_BUDGET = 2000`, because a line budget alone bounds nothing: one 200 KB
  line satisfies `take(20)`.
- **Tail, not head.** A wrapper writes its verdict *after* the command it wraps finishes; a
  compiler writes its diagnostics first, and the head is already mined by `first_error` and
  `failures`. A head-biased fix would satisfy every "stderr is present" assertion and drop the
  only line this bug is about.
- Anything cut is announced by a `--- stderr TAIL: … ---` marker naming what was dropped and
  noting that the `@cmd_*` buffer does not hold the rest. An unmarked tail would be this bug's
  own cluster one layer down.

**What is deliberately NOT fixed here.** The buffer-reader half (§ *Root cause*) — `grep` and
`read_file` on a `@cmd_*` handle return `.stdout` only, for every classification. It is a
separate defect with a wider blast radius and its own file; folding it in would have meant
changing a contract three tools depend on inside a fix for a missing field.

**The second, independent mitigation shipped too**, and does not depend on this: `--strict` on
`mutation-probe.sh` maps INCONCLUSIVE to exit 3. The exit code is the only channel that
survives every renderer, build and harness, so the two fixes are complements rather than
alternatives — an agent on an older binary still gets a usable signal. Off by default;
SURVIVED and KILLED keep their own statuses. See `docs/PROBES.md`.

Still true: do **not** answer this by teaching agents to re-run failed probes with `-- true`.
That changes the command whose behaviour is in question.
## Tests added

Seven in `src/tools/command_summary.rs`, one in `src/tools/run_command/tests.rs`, four in
`tests/mutation-probe.sh`.

Each guards a **direction**, not a presence — a head-biased implementation satisfies every
"stderr is present" assertion while dropping the verdict, so each asserts a presence AND an
absence:

- `summarize_test_output_carries_the_wrapper_verdict_on_stderr` — the reported defect.
- `summarized_stderr_keeps_the_tail_and_drops_the_head` — the direction claim.
- `summarized_stderr_bounds_a_single_enormous_line_by_bytes` — the re-buffer hazard above.
- `summarized_stderr_clips_on_a_char_boundary`, `…_returns_a_short_stream_verbatim_…`,
  `summarize_test_output_omits_empty_stderr`, `summarize_build_output_carries_stderr_…`.
- `rebuild_buffered_summary_preserves_the_test_envelopes_stderr` — **REACH, not logic.**
  That function reorders an envelope by copying fields into three named groups, so a key it
  does not enumerate is dropped silently. Every other test here asserts on the summarizer's
  return value, which is upstream and identical whether the key arrives or not.

**Mutations run** (`./scripts/mutation-probe.sh`, isolated worktree, one per guarded SITE):

| site | mutation | result |
|---|---|---|
| `STDERR_SUMMARY_LINE_BUDGET` | `20` → `20000` | **KILLED** — *"the head must be dropped, not the tail"* |
| `STDERR_SUMMARY_BYTE_BUDGET` | `2000` → `2000000` | **KILLED** — *"got 200001 bytes"* |
| `rebuild_buffered_summary` | exclude `"stderr"` from the copy loop | **KILLED** — message below |

The third was owed rather than done at first pass: the probe carries exactly one file into
its worktree, and the test lived in `run_command/tests.rs`, still uncommitted — so 0 tests
were selected and the probe returned INCONCLUSIVE. It said so because this session's other
fix put the verdict where it could be read. Run against `9c2b542f`, it kills.

**And running it found a defect in the test itself.** The first kill printed `called
Option::unwrap() on a None value` — true, and silent about which field went or why anyone
cares. Written as `rebuilt["stderr"].as_str().unwrap()` inside `assert_eq!`, the `unwrap`
panics before the explanatory message is reached, so the remedy text was unreachable by the
failure it explains. Fixed at `a5238dc2` (assert the absence first); the mutation now prints

```
rebuild_buffered_summary dropped the `stderr` key, so a wrapper's verdict cannot reach the
caller even though the summarizer emitted it; got
{"type":"test","exit_code":0,"output_id":"@cmd_abc123","passed":0}
```

— whose rendered envelope is this bug's own reported shape.

**A fixture detail is load-bearing and annotated as such.** The 200 stderr lines in the
direction test are ~6 bytes each on purpose, so the whole fixture stays under the BYTE budget.
Lengthen them and both caps bind, at which point mutating the LINE budget to infinity leaves
the test green — the sibling cap does the dropping instead and the head is still absent. Two
caps rescuing each other reads exactly like coverage.
## Workarounds

Read the `@cmd_*` buffer rather than the envelope, and **do not treat a `type: "test"` envelope's
`exit_code` + `passed` as a wrapper's verdict** — for `mutation-probe.sh` specifically, `passed: 0`
means *no test was selected*, never *the mutation survived*. A genuine survival reports
`rc=0, N test(s) ran` with `N > 0`.

For a verdict you must not lose, run the script with a command small enough not to overflow, or
run it through native `Bash`, which returns both streams.

## Resume

Start at `run_command`'s result classifier and `format_run_command`, and answer the one question
this file leaves open: is the `test` shape *dropping* stderr, *parsing* only the harness portion,
or *ceasing capture* at the inner command's exit? The three take different fixes. Reproduction 1
is the discriminator already built.

Do not re-derive the "overflow drops stderr" hypothesis — it is rejected above with a run.

## References

- `scripts/mutation-probe.sh` — `:288` on why the verdict is not in the exit status; `:265-275`
  the verdict branches, all of which write to stderr.
- `docs/PROBES.md` — the `mutation-probe.sh` row, whose closing instruction this defect makes
  unfollowable through `run_command`.
- `docs/issues/archive/2026-09-14-the-append-entry-recipes-still-teach-the-two-call-form-the-fix-replaced.md`
  — the task this was found during; the probe run whose verdict was lost was checking that file's
  regression guard, which turned out to be decoration.
- `context-injection-session-log:F-11` — the void reason above, generalised: a control whose
  subject was **relabelled in transit**, so the bytes examined were no longer the stream named.
- `context-injection-session-log:F-12` — § *Reproduction*'s layer inheritance, generalised, with
  its own consolidation **retracted**: it grouped three mechanisms under a claim broad enough to
  cover most mistakes, which is this repo's claim-shaped-never-topic-shaped rule failing on the
  entry asserting it.
- `observer-blindness:OB-26` — this file is its fourth measured instance, and its § *Who can see
  it* predicted the observer by role before the exchange happened: *"the party who tries to FIX
  from the stated mechanism. Not a more careful reader — a reader with a different job."* The
  confirming instance is recorded there rather than absorbed, per `CLAUDE.md` § *Testing
  Discipline*: a re-derivation that confirms is a **denominator**, never a catch.
- `context-injection-session-log:W-5` — why this file drew six corrections and a vaguer one would
  have drawn none. **A record with no corrections against it is indistinguishable between *right*
  and *unfalsifiable*.**
