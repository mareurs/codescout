---
id: '2cc00cdc9dac8462'
kind: bug
status: open
title: 'RESIDUAL: Exclude doc-example citation syntax from corpus_cited_tokens (or add a recurring re-measure of its exposure)'
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Exclude doc-example citation syntax from corpus_cited_tokens (or add a recurring re-measure of its exposure).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md` (status `fixed`):

> The counting defect in `corpus_cited_tokens` is DOCUMENTED, not fixed — doc examples of citation syntax still enter its token set. Option 1 was chosen because measured exposure was zero on 2026-08-25, which is a fact about that date's corpus rather than a property of the parser: a future ledger written in the guide-example style re-exposes it, and nothing re-measures.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md` — parent
