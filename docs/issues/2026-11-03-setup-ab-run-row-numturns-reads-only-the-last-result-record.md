---
id: a9f8671c8f8b4fad
kind: bug
status: fixed
title: setup-ab run.py row numTurns reads only the last result record, so a background-task turn overwrites the real turn count
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-11-09
opened: 2026-11-03
---

# BUG: setup-ab run.py row numTurns reads only the last result record

## Summary
`tools/setup-ab-eval/run.py` builds a row's `numTurns` from the last `result` record in the CLI stream. When the agent starts a background command, the CLI emits a second `result` record for a queued task-notification turn, and that record has `num_turns` 1. Two Opus rows of the Live Rust screen carry `numTurns` 1 instead of 47 and 52.

## Class fit
`num_turns` in a `result` record is correct for its own segment of the session. The row field `numTurns` states the whole run, so a correct value is published under a name that states another frame.

## Symptom (Effect)
Rows for `h4ckf0r0day__obscura-477.claude-opus-5-5.bare.0` and `.bare.1` in `live-screen/rows.jsonl` show `numTurns: 1`, with 2.5M and 3.0M input tokens and 47 and 51 API messages. The raw streams hold two `result` records each:

```
bare.0: (num_turns 47, total_cost_usd 1.5722849999999995), (num_turns 1, total_cost_usd 1.6063493999999996)
bare.1: (num_turns 52, total_cost_usd 1.9595394000000002), (num_turns 1, total_cost_usd 1.9938804)
```

The second record has `origin.kind` `task-notification`. Its text says a leftover background wait loop hit its time limit.

## Reproduction
Run an agent that starts a long background Bash command and then ends its turn. Read `stream.jsonl` and count `"type": "result"` lines. Offline check on the saved data: scan every `runs/*/stream.jsonl` under `/home/marius/work/claude/setup-ab-data/live-screen/`. Only the two runs above have more than one record.

## Environment
Claude CLI 2.1.309 as frozen in the screen registration, `--output-format stream-json`, bare arm, model claude-opus-5-5, branch `experiments`.

## Root cause
`_result_record` (`tools/setup-ab-eval/run.py:739-741`) returns `results[-1]`. `_run_one` passes it to `_cost` and to the row field `numTurns=result.get("num_turns")` (`run.py:1233`).
- `total_cost_usd` and `duration_api_ms` are cumulative across records, so the last record gives the right cost.
- `num_turns` is per segment, so the last record gives only the final queued turn.
- `duration_ms` is also per segment (895903 then 3907).

measured 2026-11-03: python scan of both streams (output above). The mechanism in `run.py` is read from source, not re-run.

## Evidence
- `rows.jsonl` rows for the two runs: `numTurns` 1, `costUsd` 1.606 and 1.994 (match the last cumulative `total_cost_usd`).
- Other 22 screen runs: exactly one `result` record each.

## Hypotheses tried
1. **Hypothesis:** the agent really took one turn. **Test:** counted API messages in `usage.main.messages`. **Verdict:** rejected (47 and 51). **Evidence:** rows.jsonl `usage` block.

## Fix

Applied 2026-11-09. New helper `num_turns(records)` in `tools/setup-ab-eval/run.py` sums `num_turns` over every `type == "result"` record. `_run_one` sets the row field from it (`numTurns=num_turns(records)`). Cost, error status and cap-hit still read the last record through `_result_record`, because `total_cost_usd` is cumulative. The helper returns None when there is no result record, and also when any result record lacks an integer `num_turns` (a bool is refused too), so a partial sum is never published as the run total.

Not changed: `duration_ms` is also per segment, but no row field reads it. The two rows already in `live-screen/rows.jsonl` keep `numTurns: 1`; the fix applies to new runs only.

Timing: the setup-ab owner (session `fbe38184-5a8d-4bc3-89dd-fdd3024c909a`) confirmed on 2026-11-09 that no frozen registration was in flight: `fresh-screen-v2/live-screen` and `live-screen-rep2` are finished (16 rows each), and the cs/full registration is not frozen yet, so the new `run.py` hash goes into it.

## Tests added

`tools/setup-ab-eval/test_run.py` `NumTurnsTest`:

- `test_a_second_result_record_adds_its_turns_and_keeps_the_last_cost`: `FakeCLI` emits a second result record (`num_turns` 1, cumulative cost 0.534). The row must carry `numTurns` 4 and `costUsd` 0.534. Before the fix it failed with `1 != 4`.
- `test_num_turns_sums_every_result_record`: no records, a non-result record, one record, two records with an assistant record between them.
- `test_a_result_record_without_an_integer_count_makes_the_total_unknown`: a missing, None, string or bool count gives None.

`FakeCLI` gained the script key `extra_result`, a dict merged over the run's result record and appended after it.

## Workarounds

For the two rows written before the fix, read them as 47 and 52 turns at analysis time, or exclude `numTurns` for rows whose stream has more than one `result` record.

## Resume

Nothing left in code. Archive this record once the fix commit is cited with its SHA and patch-id.

## References
- `tools/setup-ab-eval/run.py:739`, `:1233`
- `/home/marius/work/claude/setup-ab-data/live-screen/runs/h4ckf0r0day__obscura-477.claude-opus-5-5.bare.{0,1}/stream.jsonl`
