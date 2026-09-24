---
id: '084e2642586069b3'
kind: bug
status: open
title: 'RESIDUAL: Add serves: section declarations to the tracker-conventions guide so the first tracker-path call ships a section, not the whole 39 KB topic'
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add serves: section declarations to the tracker-conventions guide so the first tracker-path call ships a section, not the whole 39 KB topic.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md` (status `fixed`):

> Cost, not correctness. Reachability is fixed, mutation-verified, and confirmed live 2026-08-31 09:49Z. What remains: a session's FIRST tracker-path-naming call still ships tracker-conventions WHOLE (39,106 B) and no section — deliberate, since that route closed 32736ca0 — so the 26x overshoot stands on that one call until tracker-conventions adopts `serves:`. See § Resume item 1.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md` — parent
