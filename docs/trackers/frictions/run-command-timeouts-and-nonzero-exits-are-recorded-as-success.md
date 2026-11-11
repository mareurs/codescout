---
kind: tracker
status: active
title: "Evidence — run_command timeouts and failing commands are recorded as success"
tags:
  - frictions
  - run_command
  - instrument
---

# Evidence — `run_command` timeouts and failing commands are recorded as `success`

Ledger entry: FR-2 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`.

## What the instrument does

`tool_calls.outcome` records whether the tool delivered a result. A `run_command` that runs a failing command still delivers a result. The result carries `exit_code` and `timed_out`, but `outcome` stays `success`. `scripts/friction-probe.py` counts only `error` and `recoverable_error`, so it never sees these calls.

## Scale

Counts taken on 2026-11-11 with `id < 298900`.

```
select outcome, count(*), sum(output_json like '%timed_out%true%')
from tool_calls
where tool_name='run_command' and output_json like '%timed_out%' and id<298900
group by outcome;
```

Result: `success`, 314 rows, 149 of them with `timed_out: true`. No other outcome appears.

- The `runcommand` miner counted 102 rows whose output says "Command timed out after N seconds" (53 at the default 30 s). All 102 are `success`.
- A text-pattern match for a non-zero `exit_code` in success rows finds at least 1,734 rows. The pattern is crude, so this is a floor on matches and not a count of failed commands.
- The slowest 1% of `run_command` rows (512 rows) hold 344 wait loops. 166 of those loops ended at their timeout, and all 344 are recorded as `success`. See FR-14 for the wait loops.

## Examples

- Row 221568: `scripts/with-slot.sh cargo test --lib -- create_file_outside_every_tree` printed `exit_code` 101. The row says `success`.
- Row 233392: a `cargo test` filter after a rename failed. The row says `success`.
- Row 298670: latency 30021 ms, output `"timed_out": true`, `outcome` `success`.

## Consequence

Every error rate for `run_command` (1,265 non-success of about 50,900) leaves out failed builds, failed tests and timeouts. The size of the gap is not measured here.

## Related

`docs/issues/archive/2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row.md` (fixed) covers the tool-level timeout, where a row was missing. This entry covers the command-level timeout, where the row exists but says `success`. Whether that fix touches this path is not verified.

## Idea (hypothesis, not tested)

Add `command_exit` and `timed_out` columns to `tool_calls` and have the probe read them. Do not change the meaning of `outcome` without a look at every reader (`friction-probe.py`, pika, `analyze-usage`).

## Not verified

- How many of the 1,734 rows are real command failures and not commands that print the text "exit_code".
- Whether any reader of `outcome` expects command-level success.
