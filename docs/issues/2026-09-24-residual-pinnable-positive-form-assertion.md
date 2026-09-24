---
id: '60ac58939c731ae3'
kind: bug
status: open
title: 'RESIDUAL: Build the positive-form assertion: every name listed in the pinnable check is produced by a registered tool'
tags:
- cluster/assertion-that-cannot-fail
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build the positive-form assertion: every name listed in the pinnable check is produced by a registered tool.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` (status `fixed`):

> No regression test for the vacuity itself. `tests/tool_reachability.rs` closes the enabling condition (an unregistered `impl Tool`) but not the shape — an assertion naming a string no tool produces is vacuous by the same mechanism with no unregistered type involved. The positive form (each listed name IS produced by a registered tool, then is absent from `pinnable`) is not built. The fix was also incidental: the subject was deleted, nothing diagnosed the vacuity.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` — parent
