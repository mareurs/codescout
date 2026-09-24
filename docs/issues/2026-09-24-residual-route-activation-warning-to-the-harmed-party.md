---
id: b663dd5102ecb992
kind: bug
status: open
title: 'RESIDUAL: Route the concurrent-activation warning to the party who later resolves against the wrong workspace, not only the switcher'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Route the concurrent-activation warning to the party who later resolves against the wrong workspace, not only the switcher.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md` (status `fixed`):

> Only the FALSE-POSITIVE half is fixed. § Root cause names a second and worse consequence of the same gap — the warning is attached to the response of the call that PERFORMED the switch, so it reaches the switcher, while the party harmed is whoever resolves against the wrong workspace afterwards and receives nothing. fd985218 does not touch routing. The guard is now accurate and still speaks to the wrong party.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md` — parent
