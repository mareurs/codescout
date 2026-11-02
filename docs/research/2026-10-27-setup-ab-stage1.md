# Setup A/B Stage 1 calibration: costs, integrity, projected Stage 3 size

Date: 2026-10-29. Branch `research/setup-ab-eval-20261027`. Registration commit `2c57e37b3`.
Raw evidence (not in git): `/home/marius/work/claude/setup-ab-data/study/` (`rows.jsonl`, `batch.jsonl`, `runs/`).

## What Stage 1 was

Plumbing and calibration only. It ran 12 runs: 2 development tasks (`burntsushi__ripgrep-2576`, `astral-sh__ruff-15309`) x 3 arms (`bare`, `cs`, `full`) x 2 models (Sonnet 5.5, Opus 5.5), 1 rep each. Both tasks were Codex development tasks and are never scored. This report quotes no pass or fail result of any run, and no such result enters the study.

Frozen before the first model call (commit `2c57e37b3`, registration hash `31bcd38b07d4f3ace67eb334fcd619ff19b7754e17790326e492273e535abfa3`):

- plan hash `80ab16e6fe6b838908e74ef2f7f33faed6364c65a9d079186593ad98acad6768`, seed 20261029
- cap $25, per-call `--max-budget-usd` 2.00, reserve 2.00
- frozen Claude CLI 2.1.309 (the manifest's CLI), codescout 0.15.0 at `f9b0fc9f8`, Python 3.14.8
- codescout binary in every arm image: sha256 `06c8167b707913b08bc8403e04a4cbb9f80c83c0f63a9d76eeef3eda15f3c3c7`

The batch started 2026-10-29 21:53 and ended 22:23 with `stop: plan complete`. The operator authorized it through `GO.json` (written by the controller session on the operator's explicit instruction). The operator is the paying account.

## Result

| Measure | Value |
|---|---|
| Runs | 12 of 12 finished |
| Outcomes | 12 `dev-scored`. No `invalid-arm`, `plan-limit`, `infra-oom`, `infra-other` or `unknown-spend` |
| Spend | $4.70 of the $25 cap |
| Integrity verdicts | 12 of 12 ok |
| Effective effort | `medium` on every run (the CLI default; no `--effort` was passed) |
| Retries | none |
| Stop | `plan complete` |

## Cost per run, in USD (mean of the 2 development tasks)

| Model | Arm | Run costs | Mean | Turns | Wall time (s) |
|---|---|---|---|---|---|
| Sonnet 5.5 | `bare` | 0.082, 0.111 | 0.097 | 8, 14 | 39, 86 |
| Sonnet 5.5 | `cs` | 0.278, 0.146 | 0.212 | 13, 15 | 52, 72 |
| Sonnet 5.5 | `full` | 0.364, 0.293 | 0.328 | 27, 24 | 82, 136 |
| Opus 5.5 | `bare` | 0.327, 0.305 | 0.316 | 15, 16 | 112, 211 |
| Opus 5.5 | `cs` | 0.687, 0.556 | 0.622 | 23, 22 | 125, 207 |
| Opus 5.5 | `full` | 0.808, 0.742 | 0.775 | 24, 31 | 147, 201 |

Each cell has n = 2 (one run per task). These numbers calibrate the budget. They support no claim about which arm is better. Two patterns are consistent in every pair: Opus costs about 2.4 to 3.3 times Sonnet on the same arm (`bare` 3.3, `cs` 2.9, `full` 2.4), and `full` costs the most while `bare` costs the least.

## Integrity, validated on real streams

The integrity checker's fixtures were synthetic. Stage 1 is the first test against real `stream-json` output.

- All 8 `cs` and `full` runs show codescout calls (11 to 28 per run), 1 hook event each, and `sidechain_calls` 0.
- All 4 `bare` runs show 0 codescout calls and 0 hook events.
- `duplicateToolUses` is 0 and `toolUseShapeErrors` is empty on every run. The tool_use shape matches what the checker expects.
- Effort was recorded at the default, so the effort-mismatch rule did not stop the batch.
- No run ended `infra-oom`.

Observed, not yet explained: 4 of the 8 `cs` and `full` runs record 1 native (non-codescout) tool call. The cause was not investigated. It is a follow-up for Stage 2, to confirm that it is not a leak of a denied native tool.

## Projected Stage 3 size

Stage 3 plan from the spec: 12 selected tasks x 2 models x (`bare` x2 + `cs` x1 + `full` x1) = 96 runs.

| Quantity | Value |
|---|---|
| Cost per task and model, Sonnet 5.5 | $0.73 |
| Cost per task and model, Opus 5.5 | $2.03 |
| Projected Stage 3 at Stage 1 cost levels (12 tasks, both models) | $33.16 |
| Same, if real tasks cost 2 times more | $66 |
| Same, if real tasks cost 3 times more | $99 |
| Budget left: $200 - $4.70 | $195.30 |
| Usable with a 25% reserve | $146.47 |

All three cases fit under $146.47. So the open choice in the plan does not arise: about 12 tasks per model fit, and neither fewer tasks nor one model is needed. The main uncertainty is the multiplier. The two development tasks are small. The 6 private codescout tasks and the larger foreign tasks may cost more per run. A 3x multiplier still fits. A 4.4x multiplier would reach the usable limit.

## Limits of this report

- n = 2 per cell, from 2 tasks that the study never scores. The costs size the budget and nothing else.
- The frozen CLI is 2.1.309. The operator's interactive Claude Code was 2.1.312 at the time.
- The reused token (`/home/marius/.config/s5/token`) was pasted in a chat earlier and marked exposed. The operator allowed its reuse. It sits in each run's container env for the run's duration.
- Full-arm runs use the default effort, not the operator's usual xhigh.
- `--verbose` may inflate absolute costs. This applies to all arms.

## Before Stage 2 freezes

1. The operator confirms the definition of the cost floor F (the larger or the smaller geometric mean of the two `bare` costs).
2. The operator confirms the Python version (golden CI measured CPython 3.14.8).
3. Investigate the single native tool call seen in 4 of the 8 `cs` and `full` runs.
4. Re-check the arm-image binary hash for every selected task before the Stage 2 freeze.
