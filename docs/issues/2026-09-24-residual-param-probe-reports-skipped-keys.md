---
id: '7c8a5ba864bc8398'
kind: bug
status: open
title: 'RESIDUAL: Report the count of param keys the probe skips (accepts_any_json / unlabelled) so the sweep''s coverage is visible'
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Report the count of param keys the probe skips (accepts_any_json / unlabelled) so the sweep's coverage is visible.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md` (status `fixed`):

> The prescribed second half (emit `checked N of M labelled pairs`) was judged obviated by the parser fix rather than implemented — reasoning in the Fix section. Residue: keys skipped for `accepts_any_json` or for carrying no `<action>:` label remain uncounted anywhere.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md` — parent
