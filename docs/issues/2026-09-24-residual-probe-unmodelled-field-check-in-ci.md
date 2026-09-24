---
id: a11cf095b81c1c5d
kind: bug
status: open
title: 'RESIDUAL: Run the probe''s unmodelled-wire-field check in the test lane/CI'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Run the probe's unmodelled-wire-field check in the test lane/CI.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md` (status `fixed`):

> no CI-level guard: the unmodelled-field alarm fires only on a probe run, never in the test lane

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md` — parent
