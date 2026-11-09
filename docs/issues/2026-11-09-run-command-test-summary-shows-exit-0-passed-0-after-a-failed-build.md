---
id: '762b5810f84398cf'
kind: bug
status: open
title: 'run_command: a filtered `cargo test` of a failed build is typed `test` with exit_code 0, passed 0 and an empty stdout'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-11-09
---

## Summary

When an agent runs `cargo test … | tail -N` through `run_command` and the build fails to link, the reply is `{"type":"test","exit_code":0,"passed":0,"stdout":"","unfiltered_output":"@cmd_…","unfiltered_output_lines":249,"producer_exit_code":101}`. Read at a glance this is a test run with zero passes and a clean exit. The failure is only in `producer_exit_code`, and the error text is in the unfiltered buffer. Seen in the Setup A/B study runs: 8 of the 28 cs/full runs in tasks cs-p01, cs-p04 (for example `cs-p01.claude-opus-5-5.cs.0`, stream line 221) and cs-p08 (line 398).

## Class fit

`exit_code` is correct in the frame of the filter (`tail` exited 0) but the field is named as if it described the command the agent typed. The class `IC-24` is a value that is correct in one frame and published under a name that states another. This one differs from the class's librarian-adjacent members: the wrong frame is a shell pipeline, and the second frame (`producer_exit_code`) is present but only after the reader has already concluded the run was clean.

## Symptom (Effect)

The agent spends an extra call (a `grep` over the `@cmd_*` buffer) to discover that the build failed. In 8 runs this was the only cost. A model that does not make that call would read `passed: 0, exit_code: 0` as "no tests ran, no error".

## Reproduction

Observed in transcripts, not minimised. The simple case `cargo test --manifest-path /nonexistent/Cargo.toml 2>&1 | tail -5` returns `exit_code: 0`, the error line in `stdout` and `producer_exit_code: 101` (checked 2026-11-09), so the field split is documented behaviour and works. The empty-stdout, `type: test` shape appears when the producer is a failed `cargo test` whose last lines carry no test summary: the summariser still types the reply as a test run and reports `passed: 0`. Not yet reproduced outside the study transcripts.

## Environment

Codescout at the pinned study commit f9b0fc9f8 (`/opt/sab/bin/codescout start --debug`), Claude Code 2.1.309, task images of the Setup A/B study.

## Root cause

Unknown — under investigation. Hypothesis (not verified): the test-output summariser is chosen from the producer's command text (`cargo test`) and is run over the filtered stream, not over the producer's stream.

## Evidence

`/home/marius/work/claude/setup-ab-data/study/runs/cs-p01.claude-opus-5-5.cs.0/stream.jsonl`, line 221 (the reply above) and line 222 (the follow-up `grep … @cmd_8fdf02e1`). Audit reports B.md and C.md under `/home/marius/.claude-kat/jobs/fbe38184/tmp/audit/` list the other runs.

## Hypotheses tried

None yet.

## Fix

Open. Candidates: when `producer_exit_code` is non-zero and differs from `exit_code`, put it first in the reply and do not type the reply `test` with `passed: 0`; or report the filter's frame under a name that says so.

## Tests added

N/A — not fixed yet; a regression test is part of the fix.

## Workarounds

Run the producer bare and query the `@cmd_*` buffer, as the guide advises.

## Resume

Reproduce the empty-stdout shape with a real link failure, find where the reply type is chosen, then decide the fix.

## References

Codescout memory `research/setup-ab-audit-cs-usage-20261109`.
