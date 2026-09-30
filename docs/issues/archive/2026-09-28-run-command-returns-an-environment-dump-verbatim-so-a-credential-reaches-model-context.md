---
id: bc0cb248b224d1dd
kind: bug
status: archived
title: 'BUG: run_command returns an environment dump verbatim, so a credential in the env reaches model context and the transcript'
tags:
- cluster/unclassified
- security
- run_command
- credentials
closed: 2026-09-30
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

**Fixed. Operator ruling 2026-09-30: (b), redact by value shape, on the returned output AND the buffer; (a) not done.** New `src/util/redact.rs` (`redact_credentials`, `note_in`): a value shaped like `gh[pousr]_` + 36 or more alphanumerics, `github_pat_` + 50 or more, `\bsk-` + 20 or more of `[A-Za-z0-9_-]`, or `AKIA`/`ASIA` + 16 uppercase/digits becomes `<redacted-credential>`. Applied at every place output text becomes a response or a buffer entry, found by reading the code and not by assuming one chokepoint:

- **foreground decode** (`inner.rs`, the two `from_utf8_lossy` calls feeding `handle_successful_output`): the inline response, the `@cmd_*` buffer and test compaction see only scrubbed text;
- **the tee capture** (`output.rs`, Step 6.5): the UNFILTERED stream, read from a file, so it never passed the decode. `env | grep PATH` showed one line inline while the buffer behind `unfiltered_output` held the whole environment;
- **interactive mode** (`interactive.rs`, new `interactive_response`): scrubbed once on the accumulated output, never per chunk, because a token split across two reads is whole in neither;
- **the compact summary** (`format_run_command`): renders `⚠ N credential-shaped value(s) redacted from this output`, since that renderer shows only fields it reads and an edited response must say so.

The response carries `redacted_credentials: N` (summed across sites) when N > 0, and no key otherwise. A background job's log is read back only through `run_command` (a `@bg_*` handle substitutes into a shell command), so `cat @bg_x` is covered by the foreground decode.

**Limits, stated because silence would read as safety:** (1) only the four shapes above: a JWT, a bare hex key or `PASSWORD=hunter2` is NOT covered; (2) the raw log FILE of a background job stays unscrubbed on disk (pinned by a test so it cannot drift silently); (3) a fixture that merely QUOTES a token-shaped string is redacted too; (4) the elicitation prompt in interactive mode shows process output to the human operator and is left as is; (5) the fix reaches a running server only after `cargo rb` and `/mcp`. **The exposed token in the old subagent transcript still has to be rotated by the operator.**

Original candidate shapes:
- (a) Route env-dump commands (`env`, `printenv`, `set` with no arguments, `cat /proc/*/environ`) through the dangerous-command `@ack` gate.
- (b) Redact credential-shaped values (`gh[pousr]_…`, `github_pat_…`, `sk-…`, `AKIA…`) in `run_command` output before it is returned or buffered.

(b) also covers a credential that appears in any other output. Both need tests that never embed a real secret.

## Tests added

`src/util/redact.rs` (14 unit tests): each shape at and one character under its bound, every GitHub prefix (and `ghx_` refused), a token longer than the minimum consumed whole, `sk-ant-api03-…` with hyphens, `sk-` inside `risk-…`/`task-…`/`desk-…` left alone, AKIA/ASIA with uppercase-only and a longer run left alone, an env dump keeping its shape, several values counted, idempotence, borrowed-and-unchanged when clean, `note_in` summing and writing nothing for zero.

`src/tools/run_command/tests.rs` (9 wiring tests, one per route): stdout, stderr, a buffered ~14KB output (the buffer holds the marker and not the token), the tee-only capture behind a filter that hides it, the inline and tee counts adding, no key on clean output (both the plain and the tee shape), a background log read back through `cat @bg_x` (and the raw log asserted to still hold the token), the interactive response with a token split across two reads, and the compact summary rendering the notice singular and plural. Every secret is printed by `printf '%036d'`, so no command or file contains a token-shaped literal.

Mutations, one per site, in isolated worktrees (23): gh minimum, gh prefix set, PAT minimum, `sk-` boundary/minimum/hyphen class, ASIA, AWS trailing `\b` and length, match count, `note_in` sum and zero-guard, the foreground count/stdout/stderr scrub, the tee count/content/`note_in`, the compact notice and its plural, the interactive scrub and count. **All KILLED, none survived.** Three first returned INCONCLUSIVE (no test-count line; the probe carries the whole working tree, peers' edits included) and were re-run singly with the count line present: all KILLED.

## Workarounds

The SDD dispatch rule is: never print environment variables, and read a single named variable only when needed, never a credential. The operator should rotate the exposed token.

## Resume

Choose (a) and/or (b), then implement with synthetic secrets in tests.

## References

- `src/tools/` (`run_command`, dangerous-command gate)
- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/progress.md` (SECURITY INCIDENT entry, gitignored)

## Fix provenance

- **SHA:** `4e34fa3aa831cb0f9583e1d86465a0d58d17a9dd` (`experiments`)
- **patch-id:** `0fca886990209ed50cef3ca567b394bda5ed492b`
- **Gate:** run in an isolated worktree of `HEAD` plus only this change (the shared tree was red for other sessions' uncommitted `edit_code.rs`/`server.rs` work): clippy 0, lean 0, default 0 (10,451 passed, 0 failed), `cargo fmt --all -- --check` 0.
