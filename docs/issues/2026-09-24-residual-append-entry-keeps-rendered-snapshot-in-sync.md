---
id: a2859e0d13968b04
kind: bug
status: open
title: 'RESIDUAL: Make append_entry/update_entry keep a tracker''s committed rendered snapshot in sync (or refuse), instead of only warning'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make append_entry/update_entry keep a tracker's committed rendered snapshot in sync (or refuse), instead of only warning.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` (status `mitigated`):

> Root cause NOT addressed — `append_entry` still writes catalog-only state, so a tracker's committed snapshot still drifts from its live rows. The 11 tests cover the new REPORTING (doctor, append_entry and update_entry now name rows that never reached the body); nothing prevents the drift, and a caller who ignores the warning reproduces the original silent divergence.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — parent
