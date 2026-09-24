---
id: d1e3857d61e301cd
kind: bug
status: open
title: 'RESIDUAL: Add a test that reds when the server-side catalog write-through install (src/server.rs:374 at filing) is removed'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a test that reds when the server-side catalog write-through install (src/server.rs:374 at filing) is removed.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` (status `fixed`):

> Liveness caveat CLEARED 2026-08-30 13:2x: re-verified after the rebuild this field asked for, on a fresh server (PID 1899149, /proc/<pid>/exe live, binary inode 6442149 built 13:25:19). This very frontmatter write is the probe — it sets a CATALOG COLUMN (owners), and the catalog reflected it with no reindex. Remaining and unchanged: the server-side install at src/server.rs:374 is covered by no test; deleting that line leaves all 8 green.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` — parent
