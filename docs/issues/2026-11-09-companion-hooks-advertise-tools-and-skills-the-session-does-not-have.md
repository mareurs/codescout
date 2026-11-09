---
id: '08eb4c18228dca64'
kind: bug
status: open
title: companion hooks advertise a Skill tool the cs arm lacks, name tools without their mcp prefix, and teach the acknowledge_risk bypass
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-09
---

## Summary

The codescout-companion hooks advertise capabilities the cs arm of the Setup A/B study does not have, and name tools in a form the model cannot call. Verified on all 28 cs runs in `study/runs`: the session-start hook prints `SKILLS AVAILABLE: - Reconnaissance — Skill('codescout-companion:reconnaissance')` (stream line 1 of `astral-sh__ruff-15309.claude-opus-5-5.cs.0`), while the `init` tool list of every cs run has no `Skill` tool (`arms.NATIVE_TOOLS` plus the MCP tools; `Skill` and `Agent` are added only for the full arm, `arms.FULL_ONLY_TOOLS`). Also reported by the audit, each with an event index in its report: hook block text writes tool names without the `mcp__codescout__` prefix, and 18 `No such tool available` errors followed (14 in full-arm Sonnet runs, 3 in cs-arm Sonnet runs, 1 in a full-arm Opus run); the block text tells the agent that `run_command` "reaches the same file with `acknowledge_risk: true`"; the SessionStart hook prints "TRACKER HYGIENE: sweep overdue" in all 24 cs-p cs/full runs; `[cs-hint]` messages cite internal bug ids (BUG-021, BUG-027); a guard footer says "ask the user to run /mcp" in headless runs.

## Class fit

The hook builds its next-step advice from its own tables (which skills exist, which tools are called what) and never from the tool list of the session it speaks to. The class `IC-22` is a hint composed from the response shape and not from the request. This member differs from the class's tool-reply members: the hint is composed at session start, in another repository, before any request exists, so the missing input is the session's capability set.

## Symptom (Effect)

Wasted turns: each `No such tool available` call costs one model turn on a 30 to 43 thousand token prefix. An instruction the agent cannot follow (`Skill(...)` with no Skill tool) is noise in the first turn of every cs run. The `acknowledge_risk` advice teaches the bypass of the guard it comes from (nushell opus full, stream line 30: `cat column.rs` with `acknowledge_risk: true`).

## Reproduction

Read `init.tools` and the first hook event of any cs run in `/home/marius/work/claude/setup-ab-data/study/runs/*.cs.*/stream.jsonl`.

## Environment

codescout-companion plugin as installed for the study (`manifest["plugins_cs"]`), Claude Code 2.1.309, arm `cs` (`--tools Bash,Read,Write,Edit,Glob,Grep`).

## Root cause

Not investigated: the hook source lives in the companion-plugin repository (`../claude-plugins/codescout-companion/`), outside this checkout.

## Evidence

Verified by the controller 2026-11-09: 28 of 28 cs runs have the Skill advert and no Skill tool. The rest is from the audit reports `A.md`, `C.md`, `D.md`, `E.md` under `/home/marius/.claude-kat/jobs/fbe38184/tmp/audit/` (stream event indices there); not re-checked one by one.

## Hypotheses tried

That the unprefixed names cause the Sonnet `No such tool` errors: plausible, not verified.

## Fix

Open. Candidates: compose the skills list and the tool names from the session's actual tool list; do not mention the `acknowledge_risk` bypass in the block text; keep tracker hygiene and internal bug ids out of project sessions that are not the codescout repository.

## Tests added

N/A — not fixed yet.

## Workarounds

None in the harness; the arm could carry the Skill tool, which changes the arm.

## Resume

Open the companion-plugin repository, find where the session-start and block texts are composed, and check what inputs they have.

## References

Codescout memory `research/setup-ab-audit-cs-usage-20261109`.
