---
id: '7bef7d26b56bce3c'
kind: bug
status: open
title: 'RESIDUAL: Add a regression test for the doc(update) STAMP entrance into the file_sha256==disk && embedded_sha256!=disk trap state'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a regression test for the doc(update) STAMP entrance into the file_sha256==disk && embedded_sha256!=disk trap state.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md` (status `fixed`):

> No regression test guards THIS file's entrance into the trap state. fdad1a99's guard (index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it) enters via a non-embedding RUN; doc(update) enters via a STAMP. Both produce file_sha256==disk && embedded_sha256!=disk so the same escape releases both, but that is an argument and only one entrance is observed. The 2026-09-06 reproduction covers the other, and a reproduction is not a guard: making update.rs also stamp embedded_sha256 would restore this bug with the suite green.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md` — parent
