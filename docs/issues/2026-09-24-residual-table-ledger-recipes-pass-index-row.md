---
id: '47d5ae6a2b23f87a'
kind: bug
status: open
title: 'RESIDUAL: Migrate table-keeping ledger recipes (docs/TAXONOMY.md, tracker-conventions) to pass index_row + index_after_line, or gate appends that omit them'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Migrate table-keeping ledger recipes (docs/TAXONOMY.md, tracker-conventions) to pass index_row + index_after_line, or gate appends that omit them.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md` (status `fixed`):

> The window is closed for callers who USE the new parameters; it is not closed for callers who do not. `index_row` + `index_after_line` are opt-in, so any ledger whose appends omit them keeps the original two-call window unchanged. Nothing migrates the 21 table-keeping ledgers' callers, and no gate requires the parameters — a recipe in docs/TAXONOMY.md or get_guide("tracker-conventions") that still prescribes the second call will keep producing the window. The original file's other unverified: also still stands — the window is now observed on a second ledger (bug-fix-session-log:F-118, this session) but "every table-keeping ledger has it" remains reasoned from the protocol rather than measured per ledger.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md` — parent
