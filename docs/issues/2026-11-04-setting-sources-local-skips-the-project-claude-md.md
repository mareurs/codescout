---
id: b8c35dc28bd3e8dd
kind: bug
status: open
title: '--setting-sources local skips the project CLAUDE.md, so eval arms never saw the task CLAUDE.md'
owners:
- marius
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-11-04
related:
- cffe594cbf26fd72
severity: medium
unverified: The CLI source was not read; tested on 2.1.309 only. setup-ab checked on one task (cs-p01), one model, three arms; see Symptom.
---

## Summary

On Claude Code 2.1.309, `--setting-sources local` stops the CLI from loading the project `CLAUDE.md`. Two eval harnesses in this repo pass that flag and put a `CLAUDE.md` in the agent's workspace, assuming the agent reads it. It does not.

## Class fit

This is `declared-not-wired`: a `CLAUDE.md` placed in the workspace declares context for the agent, but the harness flag means that context is never wired into the model's input. It differs from the class's other members because the unwired link is not codescout code but a Claude Code CLI flag whose name (setting sources) does not say it also governs memory files.

## Symptom (Effect)

- Hint channel eval (`docs/evals/harness-channel-hint-authority-preregistration.md`, id `cffe594cbf26fd72`), arm A3: the user-endorsement declaration appended to the task's `CLAUDE.md` never reached the model. 0 of 25 A3 transcripts contain it. A3 ran as a second sample of A1, and the user-channel half of that eval's Q2 is untested (AM-9).
- No run of that eval saw codescout's own `CLAUDE.md`.
- setup-ab (`tools/setup-ab-eval/arms.py`): `SETTING_SOURCES = "local"` for the bare and cs arms and `FULL_SETTING_SOURCES = "user,local"` for full. Neither includes `project`. Checked on 2026-11-04 in the stored study runs `cs-p01.claude-sonnet-5-5.{bare,cs,full}.0` under `setup-ab-data/study/runs/`: the task `CLAUDE.md` was not loaded in any of the three arms (see Evidence). The effect on setup-ab's results is not assessed. Only one task, one model and one run per arm were checked.

## Reproduction

In a fresh git repository whose `CLAUDE.md` says "The project verification token is CLAUDE_MD_TOKEN_5X.", with the frozen CLI 2.1.309 (sha256 `4d704a8a…`) and `claude-haiku-5-5`, ask "What is the project verification token? Reply with the token only, or NONE":

- with `--setting-sources local --strict-mcp-config --tools ""`: `NONE` (2 of 2 runs);
- the same without `--setting-sources`: `CLAUDE_MD_TOKEN_5X` (1 of 1).

Script: the session scratchpad `channel-probe/run_claudemd.sh` (not in git; the steps above are complete).

## Environment

Linux, Claude Code 2.1.309 (frozen copy), model `claude-haiku-5-5`, 2026-11-04. Not tested on 2.1.318.

## Root cause

The CLI decides which memory files to load from the setting sources. `local` alone excludes the project source, so the project `CLAUDE.md` is skipped. Inferred from the probe; the CLI source was not read.

## Evidence

- The probe outputs above.
- `grep 'endorsed by the user'` over the hint channel eval's transcripts: A2 25/25 (the declaration is in a tool result there), A3 0/25, every other arm 0.

- setup-ab check, 2026-11-04. `/testbed/CLAUDE.md` exists in the task image `sab-arm-cs-p01:f9b0fc9f8` (`docker run --rm --network none --entrypoint cat <image> /testbed/CLAUDE.md` prints `# Working in this repository` with the build and test commands).
- `grep -rl 'Working in this repository'` over `config/projects` of `cs-p01.claude-sonnet-5-5.bare.0`, `.cs.0` and `.full.0`: 0 files in each arm.
- Positive control for the same method: `grep -rl 'Four Claude Code Instances'` (a phrase from the frozen user `CLAUDE.md`) finds 1 file in the `full` arm, which uses `--setting-sources user,local`, and 0 files in `bare` and `cs`. So the transcript does record a loaded `CLAUDE.md`, and the task `CLAUDE.md` is absent even in `full`.
- Limit: a transcript grep shows what the model received, not why. The cause is inferred from the earlier probe.

## Hypotheses tried

N/A — the probe answered the question directly.

## Fix

Not applied. The hint channel eval's runner must load the project `CLAUDE.md` in every arm before A3 can be rerun, which changes what all arms see; that is an operator decision recorded in the eval's next amendment. Adding `project` to the sources may also load the tree's tracked `.claude/` settings and hooks, which must be checked first. setup-ab needs its own decision.

## Tests added

N/A — no fix applied yet. A fix should come with a probe in the runner's smoke step that asserts a token from the task `CLAUDE.md` reaches the model.

## Workarounds

Deliver per-task context through the prompt or a hook instead of `CLAUDE.md` while `--setting-sources local` is in use.

## Resume

1. Decide the source set for the hint channel eval's rerun (`project,local`?) and check what the tree's `.claude/` would then load.
2. Done for one task (cs-p01, Sonnet 5.5, three arms): the task `CLAUDE.md` was not loaded. Decide whether setup-ab adds `project` to its sources, and whether earlier setup-ab results must carry a caveat. The task `CLAUDE.md` only holds the build and test commands, so the effect may be small, but it is not measured.

## References

- `docs/evals/harness-channel-hint-authority-preregistration.md` § AM-9
- `tools/hint-channel-eval/run.py` (`stage_run`, `--setting-sources local`)
- `tools/setup-ab-eval/arms.py` (`SETTING_SOURCES`, `FULL_SETTING_SOURCES`)
