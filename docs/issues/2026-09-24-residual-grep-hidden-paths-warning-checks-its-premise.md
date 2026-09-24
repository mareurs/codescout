---
id: '9278fa8a4e651b43'
kind: bug
status: open
title: 'RESIDUAL: Make grep''s hidden-paths completeness warning check that hidden pruning could explain the zero before asserting its remedy'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make grep's hidden-paths completeness warning check that hidden pruning could explain the zero before asserting its remedy.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md` (status `fixed`):

> the second defect named in the title is NOT fixed in general: the hidden-paths completeness warning still asserts its remedy without checking that hidden pruning could explain the zero. The reported misattribution can no longer occur, because the glob case now errors before any walk runs, but the narrowing candidate in Fix remains unimplemented.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md` — parent
