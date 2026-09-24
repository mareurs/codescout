---
id: a230df10e15016de
kind: bug
status: open
title: 'RESIDUAL: Extend the memory tool-doc gate to check long_docs() as well as description()'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Extend the memory tool-doc gate to check long_docs() as well as description().

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` (status `fixed`):

> the long_docs() half has no regression test — the gate reads description() only, so a future long_docs drift is uncaught

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` — parent
