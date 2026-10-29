---
id: '03b55d94e6d03c4e'
kind: bug
status: wontfix
title: 'RESIDUAL: Rename the T- entry prefix in researcher:docs/trackers/langfuse-tracing-roadmap.md (still T-7..T-12 on 2026-09-24)'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-29
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Rename the T- entry prefix in researcher:docs/trackers/langfuse-tracing-roadmap.md (still T-7..T-12 on 2026-09-24).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md` (status `fixed`):

> Item (4) of the Fix is still open and handed off: renaming `researcher`'s prefix in docs/trackers/langfuse-tracing-roadmap.md. Downgraded from urgent — unpadded T-1/T-2 now have no codescout definer, since tool-usage-patterns spells those T-001/T-002 — so a backfill there cannot create ambiguity. Recorded 2026-08-28 (tracker-hygiene-log:HY-22): the caveat was stated in prose only, so no query could reach it while status reads `fixed`.

## Fix

Decided 2026-10-29 by the operator: do not rename. Reason: the parent record downgraded this from urgent, because unpadded `T-1`/`T-2` have no codescout definer (tool-usage-patterns spells them `T-001`/`T-002`), so a backfill there cannot create ambiguity; and `researcher:docs/trackers/langfuse-tracing-roadmap.md` is `status: draft`. Re-checked 2026-10-29: `T-7` to `T-12` are still in that file, so the prefix still collides in name only. Reopen if codescout gains a definer of unpadded `T-N`, or the researcher roadmap leaves `draft`. The parent's caveat was changed to `STANDING` in the same step, so `doctor` does not report it again. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md` — parent
