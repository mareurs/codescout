---
id: f30564da22944782
kind: bug
status: open
title: 'RESIDUAL: Add a test asserting the post-compact hook text so the corrected messaging cannot silently regress'
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a test asserting the post-compact hook text so the corrected messaging cannot silently regress.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md` (status `mitigated`):

> The MESSAGING is fixed and verified live in all three profiles; the MECHANISM is untouched. (a) the prewarm is deliberately unshipped — its prescribed form is a no-op for this bug's own Rust reproduction (PREWARM_LANGUAGES is JVM-only) and the workspace-keyed mux makes the cold window far narrower than this record assumed, so the flush still does not prewarm and a genuinely cold single-session workspace still pays. Also NO REGRESSION GUARD: nothing asserts the hook text, so the sentence can regress silently — which is why this is not archived. The original 60s timeout has still never been reproduced with the mux confirmed down; that measurement remains owed.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md` — parent
