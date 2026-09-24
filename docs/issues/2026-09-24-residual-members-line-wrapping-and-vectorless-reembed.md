---
id: '8c86203711a9520d'
kind: bug
status: open
title: 'RESIDUAL: Implement a wrapping form for the **Members:** line (26 KB single line), and reindex to re-embed the 7 vectorless artifacts'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Implement a wrapping form for the **Members:** line (26 KB single line), and reindex to re-embed the 7 vectorless artifacts.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md` (status `fixed`):

> The 7 artifacts are still vectorless on disk: the code no longer produces the failure, but repairing the existing rows needs `cargo rb` plus a reindex, which has not been run. And the second half this file asks for -- a wrapping form for the `**Members:**` line -- is NOT done; it is now a vector-quality concern (a 26 KB line pools to one blurry vector) rather than the data-loss one it was.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md` — parent
