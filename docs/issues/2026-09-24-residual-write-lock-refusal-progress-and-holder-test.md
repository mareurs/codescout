---
id: c7f81780e767ee79
kind: bug
status: open
title: 'RESIDUAL: Emit progress or a starting estimate to parties refused by a held write lock, and cover the named-holder branch in tests/cross_process_write_lock.rs'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Emit progress or a starting estimate to parties refused by a held write lock, and cover the named-holder branch in tests/cross_process_write_lock.rs.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` (status `fixed`):

> No progress surface. This file's § Fix named three remedies; bullets 1 (name the holder) and 3 (stop asserting a false duration) shipped at d1b6146d, bullet 2 (emit progress, or a starting estimate, for long-running write calls) did NOT. A refused party can now identify and message the holder, which is the operational harm closed; they still cannot see how far along a 12-minute reindex is. Separately, the named-holder path has unit coverage only — tests/cross_process_write_lock.rs takes a raw flock from the test process, so it exercises the anonymous fallback branch, not the named one.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` — parent
