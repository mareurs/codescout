---
id: '14fb3387e1c6d0c6'
kind: bug
status: open
title: 'RESIDUAL: Backfill friction_target for usage.db rows written before db76f69a (or mark them) so historical attribution figures stop understating'
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Backfill friction_target for usage.db rows written before db76f69a (or mark them) so historical attribution figures stop understating.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` (status `fixed`):

> Historical rows keep their NULL friction_target — no backfill was run, so every figure computed over rows written before db76f69a still understates attribution. The `command`-addressed population (438 rows) remains target-less BY DECISION, not by defect.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md` — parent
