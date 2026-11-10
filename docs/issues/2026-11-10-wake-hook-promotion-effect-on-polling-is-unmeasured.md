---
id: '08f1bb0cb68ec514'
kind: bug
status: investigating
title: sessions poll background jobs although a wake hook exists, and the effect of the promotion is unmeasured
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-10
unverified: The effect of the new hint, bootstrap bullet and subagent rule on polling is not measured. Re-run the mining from memory research/wake-hook-promotion-20261110 on or after 2026-11-17.
---

# BUG: sessions poll background jobs although a wake hook exists, and the effect of the promotion is unmeasured

## Summary

A background job started with `run_command(run_in_background=true)` is ended by a companion hook that wakes the idle session. Sessions often read the job, sleep, or schedule a wake instead of ending the turn. On 2026-11-10 the spawn hint, the activation guide and the subagent reminder were changed to promote the wake. Whether that changed behavior has not been measured. This bug stays open until the measurement is repeated.

## Class fit

The old spawn hint named a next step, "Read output with run_command(tail -50 @bg_N)", from the shape of the response. It did not consult the state it describes, which is whether a wake hook is armed. The route returned real data, so an early read looked like progress. That is the trait of IC-22. It differs from the class's other members in that the hint was true on every harness, and only its omission of the wake misled.

## Symptom (Effect)

Last 7 days before 2026-11-10, four profiles, paired by `@bg_` handle: main sessions 523 starts, 440 woken (84%), 375 clean flow (72%), 87 early reads before the wake (47 within 3 calls of the start). Subagents 101 starts and 0 wakes by design, 58 reads and 88 sleeps. `Monitor` calls 0; `ScheduleWakeup` 95.

## Reproduction

Not a defect with a repro. It is a rate. The mining scripts and the baseline are in codescout memory `research/wake-hook-promotion-20261110`.

## Environment

Claude Code 2.1.319 to 2.1.322, codescout-companion hook `wake-on-job-end.mjs` (Claude Code only), codescout `experiments`.

## Root cause

Established: the spawn hint told the model to read the job and never said a wake follows, and the wake recipe sat in a guide that most sessions fetched late. Not established: whether that explains the early reads. Some are legitimate progress checks, and 83 main-session starts got no wake for a reason that was not found.

## Evidence

- Baseline and method: memory `research/wake-hook-promotion-20261110`.
- Fix commits: codescout `22cc87a0f` (hint and bootstrap bullet), claude-plugins `c607dcd` (subagent rule). Memory commit `740ea56bb`.

## Hypotheses tried

- The Monitor rule in the profile `CLAUDE.md` files competes with the wake: refuted for behavior, because no session called `Monitor`. The rule is rewritten anyway.

## Fix

Shipped 2026-11-10, effect unmeasured: the start hint says to end the turn when the hook is active; one Phase 0 bullet in `project-activation-bootstrap`; one line in the subagent reminder; the four profile `CLAUDE.md` rules name the wake.

## Tests added

None for the behavior change. The cold-emission ceiling test and the tool-surface tests pass with the new text. N/A for the measurement: it is a transcript count, not a test.

## Workarounds

None needed.

## Resume

On or after 2026-11-17, re-run the two scripts in the memory with `cut` set to a fixed epoch after the rebuild (about 2026-11-10 11:30 local). Compare like with like against the baseline. Close as `fixed` if clean wake flow is well above 72% and early reads within 3 calls are well below 47, on at least 200 paired main-session starts. If the result is flat, try a spawn-time line from the companion hook itself; it is true without the "if the hook is active" condition. Check first whether `wake-on-job-end.mjs` can emit context at spawn. Peer sessions on the old binary dilute the window until they run `/mcp` or restart.

## References

- Memory `research/wake-hook-promotion-20261110`.
- `src/tools/run_command/inner.rs` (`spawn_background_command`), `src/prompts/guides/project-activation-bootstrap.md`, `src/prompts/guides/progressive-disclosure.md` § Background jobs.
- `claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs`, `claude-plugins/codescout-companion/hooks/subagent-guidance.mjs`.
