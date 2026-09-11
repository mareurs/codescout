---
id: '1d944ec938b7ba2d'
kind: bug
status: open
title: '228 subagent transcripts exist as byte-identical copies across 2-3 of this machine''s three CC profile dirs, origin unestablished'
tags:
- subagents
- transcripts
- cross-profile
- doc-vs-code
topic: subagent guide delivery / transcript corpus hygiene
---

## Symptom

While re-deriving the parent/subagent guide-injection join for
`docs/issues/2026-08-31-subagents-receive-guides-their-parent-already-holds.md` (Investigation
2026-09-11), a corpus scan across this machine's three Claude Code profile dirs
(`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) found **228 of 1,606 distinct
`(parent-session-uuid, agent-transcript-filename)` keys present under 2 or 3 of the three
profile roots simultaneously**, with **byte-identical content** (verified by md5, not just
matching size) and **different inodes** (verified via `ls -li` — not a hardlink or symlink;
genuinely separate copies on disk).

Example: `agent-a4328088388be50da.jsonl` under parent session
`c95ba99b-17b2-4fe8-a8a5-afed8deb49c6`, project `-home-marius-work-claude-codescout`, exists
identically (13,039,494 B, md5 `43c6dd7e...`) under all three of
`~/.claude/projects/.../subagents/`, `~/.claude-sdd/projects/.../subagents/`, and
`~/.claude-kat/projects/.../subagents/`.

## Why this matters

Any corpus-scale measurement that walks all three profile dirs (as
`scripts/probe_guide_injection.py` and the ad-hoc join in the 2026-09-11 investigation both
do) triple-counts these 228 sessions unless explicitly deduped by
`(parent-session-uuid, filename)` rather than by path. For the guide-injection join this did
not materially change the final rate (98.1% raw vs 98.4% deduped), but that is not
guaranteed for every future measurement over this corpus, and nothing currently warns a
reader that raw per-profile counts include this duplication.

## What is NOT established

- **Origin unknown.** Whether this is a deliberate backup/sync step, a manual copy done at
  some point, a Claude Code behavior under some condition (e.g. profile migration), or
  something else. Not investigated — out of scope for the guide-injection bug this was
  found while working on.
- **Scope unknown beyond subagent transcripts.** Whether main-session transcripts
  (`<project>/<uuid>.jsonl`, no `/subagents/`) show the same duplication was not checked.
- **Whether this is still happening now** (i.e., an active ongoing sync) or is residue from
  a past one-time event. Not checked — would need mtime comparison across the duplicate
  triples, which was not done here.

## Reproduction

```python
# dedupe key: (parent_uuid, agent_filename); group by key across the three profile roots;
# report keys with >1 distinct profile; verify content equality via md5, not size.
```
Full script used: ad-hoc, not committed — see the investigation section above for the
exact logic (group subagent transcripts by `(parts[idx-1], p.name)` where `idx` is the
`"subagents"` path component; compare md5 across the group).
