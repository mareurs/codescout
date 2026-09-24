---
id: '36a0d5c7cb3fd4db'
kind: bug
status: open
title: 'RESIDUAL: Add with_hint corrected-call suggestions to the 8 required-param failure sites that name their action but offer no corrected call'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add with_hint corrected-call suggestions to the 8 required-param failure sites that name their action but offer no corrected call.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` (status `fixed`):

> Out of scope and deliberately not done: the 8 already-adequate sites name their action but carry no `with_hint` corrected call, so they satisfy clause 2 only partly. The 9 repaired sites are live-verified (2026-08-28) — see § Tests.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md` — parent
