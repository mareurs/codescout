---
id: '043c453c34469482'
kind: bug
status: open
title: 'RESIDUAL: Measure whether envelope metadata pushes otherwise-inlinable heading-scoped sections over the 9 KB inline budget, and suppress preview.headings if it does'
tags:
- cluster/hint-composed-without-the-request
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Measure whether envelope metadata pushes otherwise-inlinable heading-scoped sections over the 9 KB inline budget, and suppress preview.headings if it does.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` (status `fixed`):

> The preview.headings suppression proposed in the original Fix section was NOT measured and NOT done - whether envelope metadata pushes otherwise-inlinable sections over the 9KB inline budget is still unknown. Nothing in the shipped fix depends on it; recorded because a reader may otherwise assume the whole Fix section landed. Also: the gate's default lane showed 1 failure at commit time (tests/issue_clusters.rs::every_declared_class_has_an_index_row), which was a peer's uncommitted work on issue-clusters.md - three classes declaring a Slug with no Index row yet, all three verified absent from HEAD - and not this change.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` — parent
