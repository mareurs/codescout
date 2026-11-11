---
kind: tracker
status: active
title: "Evidence — run_command wait loops are two thirds of the slowest calls"
tags:
  - frictions
  - run_command
  - latency
---

# Evidence — `run_command` wait loops are two thirds of the slowest calls

Ledger entry: FR-14 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Source: the `runcommand` miner.

## Latency, all `run_command` rows with a latency

p50 94 ms · p90 10,897 ms · p99 291,621 ms · max 3,000,065 ms · average 15.4 s. The average is driven by the tail.

## The slowest 1% (512 rows)

| Kind | Rows |
|---|---|
| Wait loops (`until`, `while`, `sleep`, `pgrep`) | **344** (67%) |
| Build and test (`cargo`, the gate, `nextest`, unittest) | 125 |
| Other | 39 |
| `git` | 4 |

166 of the 344 wait loops ended at their timeout. All 344 are recorded as `success` (FR-2). Build and test rows show no errors.

## Examples

- Row 197597: 3,000,065 ms. `while pgrep -f "cargo test --lib$" …; do sleep 10; done`.
- Two timed-out rows poll `pgrep -f "cargo test --lib symlinked_temp_dir"`. `pgrep -f` matches the shell that runs the loop, so the loop never ends. The progressive-disclosure guide already records this trap.

## What the project already says

The wake-hook guidance: start a long job with `run_in_background=true` and end the turn; do not poll. The agents in these rows either did not receive that guidance (subagents get no wake) or did not apply it. The data does not say which.

## Idea (hypothesis, not tested)

Detect a poll loop in the command text (`until`/`while` with `sleep`/`pgrep`) and answer with the background form, as the pipe gate answers with the buffer form. A subagent cannot be woken, so the advice must name a foreground `timeout_secs`.

## Not verified

- The recovery after each loop.
- How many of the 344 ran in subagents.
