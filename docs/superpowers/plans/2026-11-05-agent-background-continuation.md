---
id: '6b6f3546c3761800'
kind: plan
status: done
title: Agent background continuation implementation plan
---

# Agent background continuation implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. The operator has already authorized autonomous implementation and offline verification.

**Goal:** Keep one CLI/MCP session alive across yielded turns, observe terminal background-job records, and deliver bounded continuations before cleanup.

**Architecture:** Use the pinned CLI's stream-json input/output mode with stdin open. A PostToolUse observer records background handles and their native record paths; a controller reads records from the exact live worker and sends terminal outcomes into the same CLI session. Job settlement is a separate measured property from patch acceptance and complete public validation.

**Tech Stack:** Python standard library; pinned Claude CLI 2.1.309; existing native Docker Codescout fixture.

**Spec:** Lifecycle bug bf167b83c0166a24 and the accepted direction in the functional pilot plan c3836cf55c7b4b87.

## Global constraints

- No real credentials, provider forwarding, or paid calls during this implementation.
- Preserve all prior cohorts, cost receipts, fingerprints and exclusive attempt markers.
- Reuse the isolated research worktree /home/marius/.codex/worktrees/615e/codescout.
- Keep same process, session id, native MCP process and background handles during continuations.
- Bound total elapsed time and continuations. Missing, malformed, failed or signalled jobs cannot count as successful settlement.
- Do not equate successful turn exit or successful job settlement with all required public checks having run.

## Review focus

- A yielded first turn must not close MCP or kill its pending job.
- Multiple jobs, failed exit codes, missing/mismatched records and malformed records must retain truthful outcomes.
- A continuation must retain context and an addressable original @bg handle.
- Timeout, client failure and continuation limit must clean up and report incomplete status.
- Stream turn costs are cumulative; never sum cumulative terminal totals or reset the campaign budget.

## Task 1: Generic stream lifecycle controller

**Files:** Create tools/agent_lifecycle/session.py, observer.py, test_session.py.

**Interfaces:** run_session(argv, cwd, env, prompt, session_id, out, read_record, timeout_s, max_continuations) returns a lifecycle receipt. read_record(record_path) reads from the exact worker. Observer writes one atomic JSON receipt per run_command result into the configured observation directory.

- [x] Write behavior tests for delayed completion, multiple/failing jobs, missing/malformed records, same-session message delivery, bounded timeout/turns and cumulative cost accounting; observe RED.
- [x] Implement the minimal controller and observer, then observe GREEN.

## Task 2: Native zero-cost qualification

**Files:** Create tools/agent_lifecycle/qualify.py and actual-client qualification evidence in a new scratch root.

**Interfaces:** Offline driver reuses the frozen task/runtime setup and scripted HTTP transport, with upstream and credential reads replaced by raising guards. No change to closed paid drivers.

- [x] Run a negative control showing that first-turn shutdown prevents the delayed job from completing.
- [x] Run positive controls through actual CLI/native Codescout: success and nonzero job, read the original handle after continuation, prove same session/process and subsequent context.
- [x] Verify zero forwarded provider requests, no credential reads, terminal native outcomes, and cleanup.

## Task 3: Review and preservation

- [x] Fresh independent review with actual applied mutations on load-bearing controller branches; close observed survivors and reapply them.
- [x] Run targeted regressions and the complete repository gate.
- [x] Update issue, report, plan and Codescout memory with measured results and remaining limitations; commit by exact paths with canonical Session-Id trailer. No push.


Execution checkpoint: Tasks 1 and 2 are verified. Lifecycle suite18/18; old pilot28/completion9/resume3 all green. Reviewer13 actual mutations: first4 killed/8 substantive survived/1 equivalent; exact recheck12 killed/0 substantive survivors/1 equivalent. Three Important bugs each RED→GREEN. Actual-client controls v5 all3 passed at zero provider cost; independent readback confirmed both continuation histories and all6 owned containers absent. Gate @bg_00000035 exited0 with Python/FMT/clippy/lean/default all0. Source committed f89af31e371268944b4e5e920f27a591bfe7bf75 (stablepatch-id b6635eb237a411a4bc62a3184794683a694a15a2); worktree clean. Report is docs/research/2026-11-05-agent-background-continuation.md. No paid launcher or old cohort was changed.

Final scope boundary: this plan's standalone component and offline controls are complete. Integration into a fresh paid-worker path and explicit required-check coverage remain tracked by bf167b83c0166a24 and the parent pilot plan; no paid runs are authorized by a qualification receipt alone.
