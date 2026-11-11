---
id: a0d6303c8b55760e
kind: bug
status: open
title: 'BUG: Opus review in subagent-driven development finds many defects the implementer ships; no correction mechanism exists'
tags:
- cluster/unclassified
opened: 2026-11-11
owner: marius
related: []
severity: high
---

# BUG: Opus review in subagent-driven development finds many defects the implementer ships; no correction mechanism exists

## Summary

In `superpowers:subagent-driven-development` runs the review goes to Opus. Opus finds many defects. The implementer subagent shipped each of those defects. Nothing feeds the defect classes back to the implementer, so the same classes may repeat. This issue tracks mining the review transcripts to find a correction mechanism: a prompt rule, a checklist, a hook, a test pattern or a script that catches a defect class before the review.

The idea came from the user on 2026-11-11. Work lives in `docs/trackers/mechanisms/`. That folder holds experiments, observations, mechanisms and scripts.

## Class fit

No class in the roster fits: the defect is a missing feedback loop between reviewer and implementer, not a code defect. The sanctioned escape hatch `cluster/unclassified` is used rather than forcing a fit. Revisit once the mining names the defect classes.

## Symptom (Effect)

Opus reviews report many findings per task, for example spec gaps, weak tests and unhandled edge cases. A re-review round often follows. Cost: Opus review tokens, fix-round tokens and wall-clock time. The Opus review is valuable; the aim is not to remove it but to make the implementer ship fewer of the defects it finds.

## Reproduction

Run `python3 -I docs/trackers/mechanisms/tools/extract_reviews.py <out_dir>` (read-only). On 2026-11-11 it found 671 Opus review subagents with a description matching "review" across the four profiles, about 6.2 MB of reports.

## Environment

Claude Code transcripts under `~/.claude*/projects/-home-marius-work-claude-codescout/<session>/subagents/agent-*.jsonl` with `*.meta.json` (`model`, `description`). The report is the `SubagentHandback` tool call in 521 of 671 cases, else the last assistant text.

## Root cause

Unknown — under investigation. Hypothesis, not verified: a small number of defect classes (for example weak tests or unwired code) account for most findings. The mining must show or refute this.

## Evidence

See `docs/trackers/mechanisms/README.md` for the mining results and the evidence trackers. Nothing is established yet.

## Hypotheses tried

N/A — mining not finished.

## Fix

Not started. Candidate mechanisms go into `docs/trackers/mechanisms/` as `M-N` entries with a measured effect before any is promoted into the implementer prompt or a hook.

## Tests added

N/A — no code change to the product. The mining scripts are measurement tools, not product code.

## Workarounds

None. The Opus review itself is the current correction.

## Resume

Start at `docs/trackers/mechanisms/README.md`. Pilot and first haiku wave are recorded there.

## References

- `docs/trackers/mechanisms/README.md`
- `docs/trackers/mechanisms/tools/`
