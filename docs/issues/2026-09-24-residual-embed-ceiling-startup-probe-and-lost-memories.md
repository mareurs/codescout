---
id: '9e6043f4db849185'
kind: bug
status: open
title: 'RESIDUAL: Implement the startup probe of the effective per-request embed ceiling, and re-write the 8 detected lost memories'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Implement the startup probe of the effective per-request embed ceiling, and re-write the 8 detected lost memories.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` (status `fixed`):

> Two residues, both deliberate: Fix step 3 (a startup probe of the effective per-request embed ceiling) is still ABSENT, so a misconfigured backend is still discovered at first oversized write rather than at connect. And the 8 already-lost memories are now DETECTED but not yet re-written — the repair is named in the verify hint, not performed.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` — parent
