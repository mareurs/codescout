---
id: c079fb93eb7fece8
kind: bug
status: open
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

The wrapper's lines are absent from the `@cmd_*` buffer too, not merely from the envelope:
`grep -c mutation-probe @cmd_a03ab2f5` → `0`, with `grep -c 'running 0 tests' @cmd_a03ab2f5` →
`1` as the control proving the buffer is non-empty and the selector works. Cargo's *own* stderr
(`Compiling …`) **is** in that buffer, so the loss is not "stderr is never captured".

## Environment

`experiments` at `a9860298`, codescout `v0.15.0`, 2026-09-14T17:4x+03:00. Main checkout
`/home/marius/work/claude/codescout`; the probe's worktree
`codescout.worktrees/mutation-aa272bed-…`.

## Root cause

**Not established, and the file deliberately stops short of naming one.** What is established is
the discrimination: the `generic` renderer preserves stderr under overflow and the `test`
renderer emits no stderr field. Whether the `test` path drops the stream, parses only the
harness-shaped portion, or stops capturing when the inner command exits, is a question for
whoever reads `format_run_command` and the classifier beside it — three readings consistent with
these observations, and choosing between them from the outside would be a guess wearing a
finding's clothes.

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
   **Verdict:** rejected — cargo's own `Compiling …` lines are stderr and are present.

## Fix

Not implemented, and the cheap form is likely right: carry `stderr` in the `test` envelope as the
`generic` one does. It is small by construction — a harness writes its results to stdout, so the
stderr of a test run is the wrapper's commentary and the compiler's, which is what a reader wants
when the news is bad.

A second, independent mitigation belongs to the script rather than the tool, and does not depend
on this being fixed: **`mutation-probe.sh` could echo its verdict line to stdout as well.** That
survives any renderer, costs one line, and does not disturb the deliberate decision to keep the
verdict out of the exit code. Whoever owns the script should decide; it is not obviously theirs
to pay for another tool's rendering.

Do **not** answer this by teaching agents to re-run failed probes with `-- true`. That changes the
command whose behaviour is in question.

## Tests added

None. The observation is about a rendering path this session did not read, and a test pinning a
shape before anyone has chosen it would be the thing `docs/adrs/2026-09-14-state-the-property-not-the-snapshot.md`
argues against.

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
- `docs/issues/2026-09-14-the-append-entry-recipes-still-teach-the-two-call-form-the-fix-replaced.md`
  — the task this was found during; the probe run whose verdict was lost was checking that file's
  regression guard, which turned out to be decoration.
