---
id: '8ba3a7dcbaac9777'
kind: bug
status: open
title: 'RESIDUAL: Make edit_code(replace) refuse (not warn) on shrink, guard create_file overwrite, and make the class gate check the guard runs on the right operands'
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make edit_code(replace) refuse (not warn) on shrink, guard create_file overwrite, and make the class gate check the guard runs on the right operands.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md` (status `mitigated`):

> Advisory only - edit_code(action=replace) WARNS and does not refuse, so a caller who ignores the warning still loses the code. src/tools/create_file.rs (overwrite: true) remains unguarded, now recorded in the class gate's EXEMPT list with its reason rather than silently. The class gate proves each surface's module CONTAINS a guard call, not that the call runs on the right operands.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-10-the-shrink-guard-covers-three-prose-write-paths-and-not-the-code-one.md` — parent
