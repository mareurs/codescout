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

## Architectural review (Snow Lion, 2026-05-18)

`run_command_inner` is currently a 9-mode dispatcher (interactive · ack-redispatch · resolve_refs · dangerous-cmd gate · source-file block · shell-mode check · background spawn · tee injection · foreground exec). The original Strategy A added a 10th. Architectural concerns:

### Concern 1 — `inject_tee` is a parallel stage-buffering mechanism

`inject_tee` (`src/tools/run_command/inner.rs:145-186`, called at `:288`) already rewrites `... | grep FAILED` into `... | tee /tmp/unfiltered | grep FAILED`, capturing the pre-filter stream as a buffer. That is one-stage-deep pipeline buffering, in production today. Strategy A as drafted ignores this and builds a parallel mechanism — two systems for the same shape of input.

**Decision (proposed):** `pipeline=` rewrites stages into a single shell pipeline with per-stage tee taps; reuses the existing foreground-exec path. `inject_tee` generalizes from "tee the penultimate stage" to "tee every stage."

**Alternatives:**
- Strategy A (`run_command_inner` per stage) — rejected: two pipeline-buffering mechanisms in tree.
- Strategy B (greenfield `run_pipeline_inner` with own spawn) — rejected: duplicates killpg/SIGPIPE/timeout machinery.

**Consequences:**
- now easier: per-stage tee already debugged on Unix (process groups, SIGPIPE reset). Pipefail = `set -o pipefail` in shell wrapper, no Rust state machine.
- now harder: per-stage timeout impossible (single shell process); only total via existing `tokio::time::timeout`. Per-stage cancellation impossible.

**Change scenarios absorbed:** per-stage cwd (`(cd <dir> && <stage>) | tee ...`); pipefail policy change (drop `set -o pipefail` from wrapper).

**Revisit-when:** streaming-output requirement lands (`stream_tail` / live `@bg_*` peek) — breaks the single-shell-process assumption.

**Confidence:** medium. Bash-ism risk (`set -o pipefail` is not POSIX sh); per-stage control is impossible by construction.

### Concern 2 — extract `exec_one_stage` before adding any 10th mode

Nine dispatch modes in one function is past the "argues about where new features belong" heuristic. Before pipeline= goes anywhere, extract the foreground-exec block (current `inner.rs:251-394`, ~140 LOC: spawn + killpg + timeout + SIGPIPE reset + buffer-store) as `exec_one_stage`. Both current foreground path and pipeline= delegate to it.

**Confidence:** medium. Have not read the full block; coupling to surrounding `inject_tee` flow may force a larger refactor than 140 LOC.

### Concern 3 — companion hook is `command`-blind to a `command + pipeline` schema

If schema lands as `command = stage 0, pipeline = [stages 1..N]`, the IL3 hook reads `tool_input.command`, sees only "cargo test," and allows. Actual behavior is a pipeline with a log-trimmer. **IL3 enforcement becomes blind to pipelines.**

**Decision (proposed):** schema is `stages: [str]` XOR `command: str`. Top-level, mutually exclusive.

**Alternatives:**
- `command` + `pipeline` (additional stages) — rejected: semantic overload, hook blindness, every downstream consumer must learn the overload.

**Consequences:**
- now easier: contract is "one of {command, stages}"; hook + telemetry + format_compact branch once on field presence.
- now harder: caller cannot append stages incrementally — must structure as `stages` from the start.

**Change scenarios absorbed:** hook enforces IL3 on pipelines (sees `stages` directly); telemetry distinguishes single-cmd vs pipeline.

**Confidence:** high.

### Concern 4 — hook coupling-across-repos is a load-bearing signal

The companion hook (`claude-plugins/codescout-companion/hooks/il3-{deny,warn}-hook.sh`) and codescout's `run_command` schema have now co-changed twice in 48h (buffer-op whitelist 2026-05-18, this design). The "two modules always change together" heuristic says they may be one module wearing two names. If a third co-change lands, the IL3 enforcement contract probably belongs **in codescout itself** (as a built-in pre-execution gate emitting `RecoverableError` with the same hint), not in a sibling repo where it drifts.

Not a decision for this tracker — flagged for the next IL3 evolution.

**Update 2026-05-18:** server-side enforcement landed. `detect_il3_violation`
in `src/util/path_security.rs` is called from `RunCommand::call` before
`resolve_refs`. The companion hook now becomes a belt-and-braces layer
covering only Claude Code; codescout itself rejects the shape for all MCP
clients (Claude Code, Copilot, Gemini, …). The "two modules always change
together" signal pointed at this fix correctly. See
`docs/issues/2026-05-18-il3-pipe-violation-subagent.md` (fixed).

## Tracker updates

- Open #1 (schema) — leans `stages` XOR `command`, not `command + pipeline`. (Concern 3.)
- Open #7 (strategy) — add **Strategy C: shell-pipeline rewrite with per-stage tee taps**. Lean C unless per-stage timeout requirement emerges. (Concern 1.)
- New open #9 — extract `exec_one_stage` from `run_command_inner` before adding any new mode. Prerequisite for any strategy. (Concern 2.)
- New open #10 — companion hook must read `tool_input.stages` if schema lands as `stages`. (Concern 3.)

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
