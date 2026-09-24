---
id: ecd68ef1c5354365
kind: bug
status: open
title: 'RESIDUAL: Make the heading-ambiguity error report file-relative line numbers (Fix step 4), and dedupe the two ''### BL-43'' definitions in open-issue-work-queue.md'
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make the heading-ambiguity error report file-relative line numbers (Fix step 4), and dedupe the two '### BL-43' definitions in open-issue-work-queue.md.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md` (status `fixed`):

> Fix step 4 (the file-relative vs body-relative line-number frame in the ambiguity error) was deliberately NOT done and remains open. A second, unrelated defect is left unrepaired by choice: open-issue-work-queue.md defines ### BL-43 twice, and choosing the authoritative copy is a content judgement about another work stream.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md` — parent
