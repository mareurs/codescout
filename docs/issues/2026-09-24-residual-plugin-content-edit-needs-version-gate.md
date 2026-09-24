---
id: '3870c801cf7bdf57'
kind: bug
status: open
title: 'RESIDUAL: Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` (status `mitigated`):

> Only this instance was repaired (codescout-companion bumped to 1.16.8, all three profiles re-synced and verified). Nothing gates the NEXT content edit without a version bump — durable options a/b/c in § Fix are all unshipped — and § Root cause records that remembering failed one commit after succeeding.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` — parent
