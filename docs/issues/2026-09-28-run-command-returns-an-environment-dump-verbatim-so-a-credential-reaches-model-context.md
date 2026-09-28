---
id: '8df0779550c5b5d8'
kind: bug
status: open
title: 'BUG: run_command returns an environment dump verbatim, so a credential in the env reaches model context and the transcript'
tags:
- cluster/unclassified
- security
- run_command
- credentials
---

**Valid:** dated 2026-09-28

## Summary

`run_command` returns an environment dump verbatim. So a credential in the process environment (`GITHUB_TOKEN`) reaches the model's context and lands in plaintext in the session transcript. Nothing refuses, redacts or warns. The dangerous-command gate covers destructive commands (`rm -rf`, …) and not disclosure.

## Symptom (Effect)

During the system1 base-rate measurement SDD run, an implementer subagent ran an env-shaped command through `run_command` in an earlier fix round. Its subagent transcript (`~/.claude-sdd/projects/-home-marius-work-claude-codescout/3c5b02df-b6ce-45f5-9d03-1194e38465c0/subagents/agent-a449f80a65fad3f27.jsonl`) contains `GITHUB_TOKEN=ghp_…` 4 times: in 1 `mcp__codescout__run_command` tool_result whose command matched `env`/`printenv`/`/proc/*/environ`, and in 1 later text message. The value was never printed while this was investigated. Only counts of the prefix pattern were read.

## Reproduction

NOT reproduced on purpose, because reproducing it re-discloses the credential. The mechanism is visible from the shape: `run_command` passes stdout through, and the dangerous-command gate does not match env-dump commands.

## Environment

codescout MCP `run_command`; Claude Code profile `~/.claude-sdd`; a subagent of session `3c5b02df-b6ce-45f5-9d03-1194e38465c0`.

## Root cause

`run_command` treats all command output as data to return. The dangerous-command gate is keyed on destructive effects only. A command whose effect is DISCLOSURE (an environment dump, a credential file read) is neither refused nor redacted. Its output then goes to the model provider as context and into the harness transcript, and that transcript is copied into measurement corpora.

## Evidence

- A count-only attribution over the transcript: 1 run_command tool_result with an env-shaped command, and 1 text message.
- 0 matches in the SDD workspace, the session scratchpad (including scratch corpora), and committed paths.
- For the measurement: the transcript belongs to an EXCLUDED session (spec Amendment 7 (a)4), so it never reaches the Codex judge.

## Hypotheses tried

None; the issue is a missing guard, not a malfunction.

## Fix

Not fixed. Candidate shapes, not yet chosen:
- (a) Route env-dump commands (`env`, `printenv`, `set` with no arguments, `cat /proc/*/environ`) through the dangerous-command `@ack` gate.
- (b) Redact credential-shaped values (`gh[pousr]_…`, `github_pat_…`, `sk-…`, `AKIA…`) in `run_command` output before it is returned or buffered.

(b) also covers a credential that appears in any other output. Both need tests that never embed a real secret.

## Tests added

None yet.

## Workarounds

The SDD dispatch rule is: never print environment variables, and read a single named variable only when needed, never a credential. The operator should rotate the exposed token.

## Resume

Choose (a) and/or (b), then implement with synthetic secrets in tests.

## References

- `src/tools/` (`run_command`, dangerous-command gate)
- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/progress.md` (SECURITY INCIDENT entry, gitignored)
