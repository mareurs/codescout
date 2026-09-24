---
id: fb147f71da8e15ab
kind: bug
status: open
title: 'RESIDUAL: Add a write-time/CI gate refusing a terminal-status or archived bug file that lacks the fix SHA + patch-id pair'
tags:
- cluster/record-asserts-an-unchecked-completion
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a write-time/CI gate refusing a terminal-status or archived bug file that lacks the fix SHA + patch-id pair.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` (status `mitigated`):

> 10 of the 63 archived records were ALREADY unrecoverable when this was mitigated — their objects are gone from the object DB — and no patch-id can restore them. The 53 recoverable ones were back-filled, but nothing gates a FUTURE archive that omits the pair at write time; detection rests on `doctor`'s `terminal_status_without_fix_anchor`, which is run manually. `patch-id` also dies under squash, since a union diff hashes differently.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md` — parent
