---
kind: bug
status: fixed
title: IL3 pipe-violation in subagent context
slug: il3-pipe-violation-subagent
opened: 2026-05-18
closed: 2026-05-18
last_observed: 2026-05-18
---

# IL3 pipe-violation in subagent context

## Symptom

Subagents spawned via `Agent({subagent_type: "general-purpose", ...})` invoke
`run_command` with piped output (`cargo X 2>&1 | tail -50`) despite IL3
forbidding it. The companion hook
(`codescout-companion/hooks/semantic-tool-router.sh`) does not appear to
intercept the violation in subagent context.

Observed during baseline scoring for
`docs/evals/run-in-background-discoverability.md` (2026-05-18):

- **P1 subagent** ran `cargo build --release 2>&1 | tail -50` as its first
  `run_command` call.
- **P6 subagent** ran `cargo test 2>&1 | tail -50` as its first
  `run_command` call.

Both completed without the hook intercepting.

## Reproduction

```python
Agent({
  description: "il3-repro",
  subagent_type: "general-purpose",
  prompt: "In the codescout repo, run `cargo build --release 2>&1 | tail -50` via run_command and report the exit code."
})
```

Expected: hook blocks the call with the "IL3 violation" error.
Observed: command runs, agent reports exit code.

## Root cause

Not yet investigated. Hypotheses:

1. The `PreToolUse` hook fires only for the host process's `Bash` /
   `Grep` / `Glob` tools, not for MCP `run_command` calls — IL3 in
   `source.md` is a **prompt instruction**, not a hook-enforced rule. The
   subagent sees the same prompt, but follows it imperfectly under task
   pressure.
2. Subagent context strips some part of the injection (the buffer-ref
   primer that the IL3 rule depends on) so the rule reads as guidance
   without a concrete alternative.
3. Hook IS hosted by the parent session and does not propagate to
   subagent-spawned MCP traffic — would be a hook coverage gap.

Read `codescout-companion/hooks/semantic-tool-router.sh` against
`run_command` traffic flow before picking a hypothesis.

## Evidence

- `docs/evals/run-in-background-discoverability.md` — baseline P1, P6 rows.
- Subagent JSON returns:
  - P1: `{"first_run_command_args":{"command":"cargo build --release 2>&1 | tail -50","timeout_secs":600}}`
  - P6: `{"first_run_command_args":"cargo test 2>&1 | tail -50"}`

## Hypotheses tried

None yet.

## Fix

Server-side IL3 enforcement landed in `run_command_inner` via `detect_il3_violation` (`src/util/path_security.rs`, called from `RunCommand::call` before `resolve_refs`). The check fires regardless of caller (top-level session, subagent, future MCP clients). Companion `PreToolUse` hook remains as a Claude-Code-specific belt-and-braces layer; can be sunset once telemetry confirms parity.

Live verification: `cat items.txt | grep apple` now returns `RecoverableError` with the IL3 hint. Tests in `src/util/path_security.rs::tests::il3_*` (11 cases) and three rewrites in `src/tools/run_command/tests.rs` (live pipes now assert IL3 rejection, not `inject_tee` capture).

The `inject_tee` mechanism remains for buffer-op pipes (`grep PATTERN @cmd_xxx | sort`) — IL3 allows those, and `inject_tee` still tees the intermediate stage.
## Tests added

None yet. A regression test would spawn a subagent and assert the hook
fires; needs companion-plugin test harness.

## Workarounds

- Top-level session: hook fires reliably (verified mid-session multiple
  times in this work stream).
- Subagent: prompt the subagent explicitly that pipes are forbidden — but
  this just relocates the trust into the prompt.

## Resume

Cold-start instructions for picking this up:

1. Read `codescout-companion/hooks/semantic-tool-router.sh` to confirm
   which tool names it gates.
2. Check the companion's `SubagentStart` hook — does it inject the
   `PreToolUse` rule for subagents?
3. Reproduce with the repro above and inspect hook logs.
4. If the hook is not subagent-aware, the fix is either:
   - Extend the hook to gate `run_command` calls (host-tool agnostic), or
   - Move the IL3 enforcement into codescout itself — `run_command` rejects
     piped invocations server-side with the same error message.
   Server-side enforcement is the right place (agent-agnostic per
   CLAUDE.md "Design Principles") — hooks are Claude-Code-specific.
