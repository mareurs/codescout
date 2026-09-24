---
id: '3b4fce62b0289f20'
kind: bug
status: open
title: 'RESIDUAL: Take option 2 (document unpinned-concurrent activation as unsupported) and/or option 3 (code guard against a subagent activate displacing the home default, blocked on per-caller identity)'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Take option 2 (document unpinned-concurrent activation as unsupported) and/or option 3 (code guard against a subagent activate displacing the home default, blocked on per-caller identity).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md` (status `mitigated`):

> Root cause (global mutable default_workspace_root, no per-caller identity) is NOT addressed — options 2/3 in Fix remain open. This closes the specific incident trigger (a Workflow script briefed to call activate) by strengthening the existing, already-correct-but-underweighted guidance; a subagent that ignores the briefing can still reproduce the original failure.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md` — parent
