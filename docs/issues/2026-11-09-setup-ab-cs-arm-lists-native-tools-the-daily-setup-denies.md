---
id: ba34b0fc327743ca
kind: bug
status: open
title: 'setup-ab: the cs arm lists Bash, Edit and Write that the daily setup denies, so the companion hook blocks them call by call'
tags:
- cluster/repro-env-diverges-from-gate-env
opened: 2026-11-09
---

## Summary

The cs arm of the Setup A/B study is not configured like the daily setup it is meant to stand for. `arms.py` passes `--tools Bash,Read,Write,Edit,Glob,Grep` to the cs arm (`tools = FULL_TOOLS if arm == "full" else NATIVE_TOOLS`, `arms.py` near line 453), so the native tools stay in the tool list and the companion hook has to deny them one call at a time. The full arm loads the user settings, whose `permissions.deny` removes Bash, Edit and Write from the list. Audit E counted 43 blocked native calls in cs and 6 in full, and all 14 Opus cs runs opened with a blocked native call. The hook's breaker can also let a native call through: in `astral-sh__ruff-15626.claude-opus-5-5.cs.0` the native `Edit` at stream line 80 succeeded (result at line 87) and the agent wrote "Only the last edit landed." (line 88).

## Class fit

The environment that was measured (native tools listed, then denied by a hook) differs from the environment the claim is about (daily use, native write tools absent). The class `IC-5` is a reproduction environment that is not the gating environment. This member differs from the class's build and gate members: the divergence is in a study arm's tool list, and it changes how many calls an agent needs, not whether a build passes.

## Symptom (Effect)

Extra turns on a 31 thousand token prefix for every blocked first call, and an arm whose behaviour is not the daily behaviour. In the cs-p tasks all 12 cs/full runs per task group are 1.6 to 4.3 times as expensive as their bare twins; the audit attributes 38 to 83 percent of the extra input tokens to the fixed first-turn context and the rest mostly to refused calls. This cost is part of every cs-versus-bare comparison made so far.

## Reproduction

`arms.py` lines 62 to 66 and 452 to 453 (read 2026-11-09): `NATIVE_TOOLS = "Bash,Read,Write,Edit,Glob,Grep"`, `FULL_ONLY_TOOLS = ("Skill", "Agent")`, `tools = FULL_TOOLS if arm == "full" else NATIVE_TOOLS`. Compare the `init.tools` of a cs run with a full run.

## Environment

Setup A/B harness at the study commit, arms `cs` and `full`, Claude Code 2.1.309.

## Root cause

Verified at the code: the cs arm shares the bare arm's `--tools` value. Whether daily sessions list the native write tools is taken from the full arm's user settings (`permissions.deny: [Bash, Edit, NotebookEdit, Write]`, audit D); the daily profile itself was not re-read.

## Evidence

`arms.py` as above; stream lines 80, 87, 88 of the ruff-15626 Opus cs run (read 2026-11-09); audit reports `E.md` and `D.md` under `/home/marius/.claude-kat/jobs/fbe38184/tmp/audit/`.

## Hypotheses tried

Not verified: that giving cs the same deny list closes most of the cost gap.

## Fix

Open. Candidate: a versioned arm profile in which cs lists only the tools the daily setup lists (without Bash, Edit and Write), selected by a parameter whose default is the old behaviour, so registered screens stay reproducible.

Code landed 2026-11-09, opt-in: arm profile `v2` in `8a44c4eca` (cs lists `Read,Glob,Grep`; `arms.py` `CS_TOOLS_V2`), and the registration pins the profile in `acf72c2d6` (`run.py` passes it to every arm). The default stays `v1`, and no registered run has used `v2` yet. The record stays open until a `v2` registration runs. It also rests on an open decision: whether the daily setup's deny list is the intended reference for the cs arm. Owner of the setup-ab work: session `fbe38184-5a8d-4bc3-89dd-fdd3024c909a`, which asked on 2026-11-09 that this record not be closed as fixed yet.

## Tests added

Unit tests only, in `8a44c4eca` and `acf72c2d6`: `test_arms.py` `test_v2_cs_lists_no_tool_the_daily_setup_denies` and its siblings, `test_run.py` `RegisteredArmProfileTest`, `test_freeze_screen.py` `test_freeze_pins_the_arm_profile_in_the_hashed_registration`. They check the argument lists, not a live run.

## Workarounds

None.

## Resume

Add the profile parameter to `arms.py` and a test in `test_arms.py` that the cs arm's `--tools` has no Bash, Edit or Write under the new profile.

## References

Codescout memory `research/setup-ab-audit-cs-usage-20261109`; related open bug b8c35dc28bd3e8dd (the task CLAUDE.md does not reach the arms).
