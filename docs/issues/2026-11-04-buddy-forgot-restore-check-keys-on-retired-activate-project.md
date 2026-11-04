---
id: '67ee3305dce3224c'
kind: bug
status: open
title: buddy's forgot-restore check keys on the retired activate_project tool, so it can never fire
owners:
- marius
tags:
- cluster/declared-not-wired
opened: 2026-11-04
related:
- '9419db55285a2f81'
- cffe594cbf26fd72
severity: low
unverified: The zero deliveries come from a subagent's transcript scan, not re-checked; the rename history was not traced; no fix applied, no test.
---

# BUG: buddy's forgot-restore check keys on the retired activate_project tool, so it can never fire

## Summary

The `buddy` plugin's `cs_heuristics.py` has a check, `_check_forgot_restore`, that warns when an agent activates a foreign project twice without restoring home. It matches the tool name `mcp__codescout__activate_project`. That tool was folded into `workspace(action="activate")`, so the check never matches a call, and its message still tells the agent to run `activate_project('.')`.

## Class fit

This is `declared-not-wired`: the check is declared in the heuristics list and has tests that feed it an `activate_project` event, but nothing in a current session produces such an event, so the declared guard is never connected to a real call. It differs from the class's other members because the dead link is a tool rename, not a missing registration.

## Symptom (Effect)

- No warning is ever sent for the pattern it was written for (foreign activation without restoring home), although the pattern still exists: `workspace(action="activate")` is called with foreign paths.
- If it did fire, it would tell the agent to call `activate_project('.')`, a tool that does not exist.

## Reproduction

1. Read `claude-plugins:044a0d0` `buddy/scripts/cs_heuristics.py`: `_check_forgot_restore` returns early unless `event["tool_name"] == "mcp__codescout__activate_project"`.
2. Count calls in this checkout's `.codescout/usage.db` (`tool_calls`, 2026-10-05 to 2026-11-04): tool names like `activate_project`: 0. Calls to `workspace` whose input contains `activate`: 275.

## Environment

claude-plugins `buddy` at `044a0d0`; codescout on `experiments` at `4d659fe57`; 2026-11-04.

## Root cause

The tool was renamed into `workspace` and the heuristic was not updated. Inferred from the two counts and the source; the rename commit was not traced.

## Evidence

- Source read 2026-11-04: the tool-name test and the message text.
- usage.db counts above, run 2026-11-04: 0 and 275.
- A mining subagent's transcript scan found 0 deliveries of this hint and 0 of the ignored-buffer hint (not re-checked here). The same scan could not say why the ignored-buffer check never fires; that is not claimed to be this bug. Report: `docs/evals/data/2026-11-04-harness-channel-hints/hint-census/mining-report.md`.

## Hypotheses tried

N/A — the counts settle it.

## Fix

Not applied. Either key the check on `mcp__codescout__workspace` with `tool_input.action == "activate"` and update the text, or delete the check. Deleting needs a decision: the hint's rule (restore home between projects) is still true.

## Tests added

N/A — no fix applied. A fix should add a test that feeds the check a recorded `workspace(activate)` event, and the tool-name test named in the sibling bug.

## Workarounds

The `workspace` result already carries a note about returning to the home project.

## Resume

1. Decide: re-key to `workspace(action="activate")` or delete.
2. Add the recorded-event test.

## References

- `claude-plugins:044a0d0` `buddy/scripts/cs_heuristics.py` (`_check_forgot_restore`)
- sibling bug `9419db55285a2f81` (the native-Bash hint names `find_symbol`)
- `docs/evals/data/2026-11-04-harness-channel-hints/hint-census/mining-report.md`
