---
id: d5acda1995c73ffe
kind: bug
status: investigating
title: Subagents are auto-injected guide topics their parent already holds — 84% of measured subagent sessions, ~2.84 MB for one topic, and the workspace-state guide says it cannot happen
tags:
- guides
- subagents
- guide-ledger
- prompt-surface
- doc-vs-code
---

## Symptom

A subagent session receives an auto-injected `tracker-conventions` guide body when its
**parent session already received the same topic**. Measured 2026-08-31 across 87 subagent
transcripts on two machines:

| | n | parent also had it | rate |
|---|---|---|---|
| all subagent sessions | 87 | 73 | **84%** |
| Agent-tool subagents, session start < 2026-08-27 | 76 | 62 | 82% |
| Agent-tool subagents, session start >= 2026-08-27 | 5 | 5 | **100%** |
| Workflow-spawned agents (all pre-2026-08-27) | 6 | 6 | 100% |

At 38,870 B for this topic, the 73 double-deliveries are **~2.84 MB** — for one topic, in
one sampled corpus, over roughly 26 days.

## Why this is filed as a bug rather than a cost

`get_guide("workspace-state")` § *Subagent semantics* states the opposite outright:

> Subagents that share the parent's MCP server share: […] the same `guide_hints_emitted`
> set (**parent-triggered hints don't re-fire for subagents**)

So either the contract is stale or the mechanism is not doing what it documents. Both are
findings; they need different fixes, and nothing distinguishes them today.

## Reproduction / how it was measured

`scripts/probe_guide_section_use.py` builds the session list; the double-delivery join is:
a subagent transcript lives at `<project>/<parent-uuid>/subagents/[workflows/<wf>/]agent-*.jsonl`,
so the parent transcript is `<project>/<parent-uuid>.jsonl`. Check whether the parent file
also contains the **opening** marker
`auto-injected get_guide('tracker-conventions') — first call` — opening form only, per the
2026-08-27 parsing contract. Session dates come from each transcript's own first
`timestamp` field, never mtime.

## What is NOT established

- **n = 5 post-fix.** The 100% post-2026-08-27 rate is 5 of 5. It is consistent with no
  improvement, and it is far too small to conclude the fix is ineffective. **Do not read
  it as one.** This is the whole reason the file is `investigating` rather than `open`
  with a diagnosis attached.
- **The mechanism is unverified.** If a subagent genuinely shares the parent's MCP server
  process, it shares the in-memory `guide_hints_emitted` set and re-injection should be
  impossible — which is what the guide asserts. That it happens anyway suggests either a
  separate server process per subagent, or a re-arm on some path. **Not investigated.**
  Reading the source is the next step, and per the project's own rule a claim about tool
  behaviour needs the call run and the real output read, not the source alone.

## Related, and why this is the OTHER direction

`docs/issues/archive/2026-08-26-subagent-guide-fetch-starves-parent.md` (fixed) is the
mirror image: a subagent's fetch marked a topic delivered for the whole session, starving
the **parent**. Its fix shipped 2026-08-27 client-side in the companion plugin
(`codescout-companion:d47dea4`, patch-id `c30242187d28052a672baeadbfd26048861f4fdd`):
`agent-guide-snapshot.mjs` (PreToolUse, matcher `Agent`) copies the ledger file before a
dispatch and `agent-guide-restore.mjs` restores it after, undoing the subagent's writes
while preserving the parent's.

That fix protects the parent. It does not claim to stop the subagent receiving anything,
and this file is not a regression report against it. **Its own text names Workflow-spawned
agents as uncovered** — a different dispatch mechanism, not the `Agent` matcher — and the
table above finds those at 100%, consistent with that exclusion.

**The constraint that shapes any fix** is recorded in the same file and in
`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § *Out of scope*:

> no MCP client, Claude Code included, sends any per-request caller identity today, so the
> server cannot tell a subagent's tool call apart from its parent's

So a server-side "don't inject into subagents" rule is **not implementable in codescout**.
Any remedy is client-side, in the companion plugin, where the `Agent` PreToolUse /
PostToolUse hooks already exist and already manipulate this exact ledger file.

## Why it matters beyond the bytes

Measured the same day (`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
§ *The DISTRIBUTION probe ran*), subagents engage **7.5%** of the `tracker-conventions`
bytes they receive against main sessions' ~55%, and 38 of 87 engage none of it. So this is
not merely repeated delivery — it is repeated delivery into the population least likely to
use it.

