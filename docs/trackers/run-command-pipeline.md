---
id: '16bbad9e601dc081'
kind: tracker
status: draft
title: run_command(pipeline=[...]) design
owners: []
tags:
- run_command
- pipeline
- il3
topic: null
time_scope: null
---

# Tracker — `run_command(pipeline=[...])`

## Why

IL3 banned all pipes to log-trimmers because piping discards the buffer. The deny hook was promoted 2026-05-18 after 50+ slips. But the ban has a cost: legitimate **staged filtering** of live commands has no first-class API — agents either:

1. Pipe anyway (now blocked), or
2. Re-run from the buffer (clumsy, costs a second invocation per stage).

`pipeline=[...]` is the missing primitive: each stage's stdout buffered as its own `@cmd_*`, stage k+1 reads stage k via stdin. Funnel is materialized, debuggable, queryable per-stage.

The hook fix (shipped 2026-05-18) is the **read-side** half — pipes that start from a buffer (`grep PATTERN @cmd_x | sort`) are now allowed because the capture already happened. `pipeline=` is the **write-side** half — staging on a live command.

## Design surfaces (open)

1. **Schema shape.** `pipeline: Vec<String>` (additional stages after `command`)? Or
   `command` + `pipeline: Vec<String>` where pipeline[0] is stage 1? Or
   replace `command` entirely with a stages array when pipeline given?
   Lean: `command` = stage 0 (current behavior preserved), `pipeline` = stages 1..N.

2. **Mutual exclusivity.** Pipeline cannot co-exist with:
   - `run_in_background` (would need a streaming abstraction)
   - `interactive` (no stdin loop semantics for staged pipes)
   - `@ack_*` handle as `command` (ack flow is single-stage)

3. **Timeout policy.** Total wall-clock across all stages, or per-stage?
   Lean: total. Caller passes `timeout_secs`; each stage gets `remaining = total - elapsed`.
   Simpler, matches user mental model.

4. **Pipefail semantics.** Stop on first non-zero, keep prior buffers, surface failing
   stage clearly. (Matches `set -o pipefail`.)

5. **Output shape.**
   ```json
   {
     "stages": [
       {"stage": 0, "cmd": "cargo test", "ref": "@cmd_a", "exit_code": 0, "preview": "..."},
       {"stage": 1, "cmd": "grep FAILED",  "ref": "@cmd_b", "exit_code": 0, "preview": "..."},
       {"stage": 2, "cmd": "head -20",     "ref": "@cmd_c", "exit_code": 0, "preview": "..."}
     ],
     "final_ref": "@cmd_c",
     "stopped_at": null
   }
   ```
   On pipefail: `stopped_at: 2` + truncated stages array + error stage's stderr/exit_code.

6. **Per-stage dangerous-command gate?** Each stage runs `is_dangerous_command`?
   Lean: yes — `pipeline=["cargo test", "rm -rf /"]` should be caught at stage 1.
   But `@ack_*` flow per-stage gets ugly. Initial cut: any dangerous stage → outright reject
   the whole pipeline call with a clear hint. Defer ack-handle support.

7. **Implementation strategy.**
   - **A. Reuse `run_command_inner`.** Each stage spawns via inner with synthesized
     `<stage> < <tempfile_of_prev_stage>`. Buffer stage stdout via `OutputBuffer.store`.
     Pro: reuses timeout, killpg, security checks. Con: re-runs `resolve_refs` per stage.
   - **B. New `run_pipeline_inner`.** Spawns each stage directly, pipes stdouts in memory.
     Pro: cleaner data flow. Con: duplicates process-group/killpg/SIGPIPE machinery.
   Lean: **A**. Reuse machinery; the per-stage `resolve_refs` is cheap and consistent.

8. **format_compact display.**
   ```
   ✓ pipeline 3/3 stages  (queries @cmd_a @cmd_b @cmd_c)
   ✗ pipeline 2/3 — stage 1 (grep FAILED) exit 1  (query @cmd_b for err)
   ```

## Tests needed

- Happy: 3-stage `seq 1 100 | grep ^5 | wc -l` produces 11 (one "5", "50"-"59", "5"; 11 matches).
- Pipefail: stage 1 returns non-zero, stage 2 never runs; result has `stopped_at: 1` and only 2 stage entries.
- Stdin wiring: stage k stdin is exactly stage k-1 stdout (no leakage, no doubling).
- Dangerous stage rejected before any stage runs.
- Timeout total: `pipeline=["sleep 5", "cat"]` with `timeout_secs=2` → timeout error, stage 0 buffer kept.
- Mutex: pipeline + run_in_background → recoverable error.

## Prompt rewrites

- `src/prompts/source.md` IL3 section — already has buffer-op allowance; add `pipeline=` paragraph.
- `src/tools/run_command/mod.rs` `long_docs` — add `## Staged Pipelines` section with one example.

## Resume

Hook smartening shipped 2026-05-18 (see commit on `experiments`). IL3 prompt rewrite shipped same day. Pipeline= design tracker opened, implementation pending its own dedicated session.

Open the next session with: read this tracker → resolve open items 1, 3, 6 → write `run_pipeline_inner` per strategy A → tests → prompt update.

