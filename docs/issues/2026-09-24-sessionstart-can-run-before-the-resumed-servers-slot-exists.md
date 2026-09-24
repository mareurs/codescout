---
id: b586243d43574c1b
kind: bug
status: open
title: 'BUG: SessionStart can run before the resumed session''s codescout server publishes its slot, so that SessionStart never stamps it'
tags:
- cluster/gate-keyed-on-unobservable-event
opened: 2026-09-24
owner: marius
related:
- a5054d135acacbe3
- '54a1a8011bca0358'
severity: medium
---

# BUG: SessionStart can run before the resumed session's codescout server publishes its slot, so that SessionStart never stamps it

## Summary

The companion's `SessionStart` stamp assumes the new server's rendezvous slot already exists. The comment at `claude-plugins:codescout-companion/hooks/session-start.mjs:51-52` says "MCP initialize runs before SessionStart". On an interactive `--resume`, that assumption failed on **2 of 2** observed resumes: the slot was never stamped by the SessionStart that should have stamped it.

The session id still arrives on resume, because the new server inherits it from its predecessor's slot. The **source** does not, so `a5054d135acacbe3`'s `"kept"` branch is likely unreachable after a resume.

## Symptom (Effect)

Reported by sessionId `09093108-1425-4f6d-9695-a9e3bb98ea0d` (`~/.claude-sdd`), from its own resumes, 2026-09-24:

- **Resume 2, 11:16:51Z, companion 1.20.14.** Server slot `servers/1196086.json` published at 11:16:51.394Z, carrying the inherited `hook_at` 11:05:25Z and no `hook_source`. The `SessionStart:resume` attachment was recorded at 11:16:51.499Z. The slot was never stamped; its next write was the liveness refresher at 11:17:05, which moves `hook_at` only.
- **Resume 1**, earlier the same morning: same outcome, slot `servers/3415327.json` (no `hook_source`).

## Root cause (not proven)

The peer ruled out two alternatives:
- **An empty `source`:** it was `resume`, and `session-start.mjs:77` stamps then.
- **Old code:** 1.20.14's `session-start.mjs` contains the stamping.

What survives is "the hook's scan found no slot in its ancestry". That's consistent with the hook scanning before the server wrote its slot. It is **not proven**, because the hook's start time isn't recorded, only the attachment time (.499Z, after the slot's .394Z).

## Not measured

- **Whether a fresh interactive `startup` can lose the same race.** If it can, that server has no predecessor to inherit a session from. It would stay unstamped for its whole life, because `refreshLivenessStamp` never opens the gate (invariant 1). That would put the whole session on the anonymous/blunt-clear path.
- `claude -p` startups won on every observation this session (margins 51, 62 and 86 ms).

## Fix

Not designed. Candidates:
- Have the hook record session and source in a per-session file that the server reads when it adopts. This decouples them from slot timing.
- Or have the server re-check for a late stamp.

First step: instrument the hook to log its scan time and whether it found no slot, then measure the rate over real resumes and startups.

## References

- `a5054d135acacbe3`: its `"kept"` half depends on this (see its Resume, known limit).
- `54a1a8011bca0358`: same stamping loop, different defect (it matched too much; this matches too early).
