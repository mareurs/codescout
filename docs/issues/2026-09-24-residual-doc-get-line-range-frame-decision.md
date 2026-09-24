---
id: d003063b9086c779
kind: bug
status: open
title: 'RESIDUAL: Decide whether to make doc(get)''s start_line/end_line file-relative (a breaking change to a tested contract) and implement if accepted'
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide whether to make doc(get)'s start_line/end_line file-relative (a breaking change to a tested contract) and implement if accepted.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md` (status `fixed`):

> start_line/end_line composability with grep/link_scan remains unfixed (deliberately, per F-128) — only headings/occurrences are file-relative now

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md` — parent
