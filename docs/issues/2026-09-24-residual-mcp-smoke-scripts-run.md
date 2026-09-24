---
id: '7f97ee751bf72bc4'
kind: bug
status: open
title: 'RESIDUAL: Fix the MCP smoke script''s get_symbols_overview call (nonexistent tool) and run the smoke scripts once'
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Fix the MCP smoke script's get_symbols_overview call (nonexistent tool) and run the smoke scripts once.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md` (status `fixed`):

> the symbols-parameter half is fixed but unrun; the get_symbols_overview half is not fixed at all

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md` — parent
