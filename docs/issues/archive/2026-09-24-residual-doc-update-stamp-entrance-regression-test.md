---
id: 65ac43dda6368e09
kind: bug
status: fixed
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

Added `a_doc_update_leaves_its_new_content_embeddable_by_an_ordinary_run` in `src/librarian/tools/update.rs`. It drives the real `update` call, asserts the trap state as a PRECONDITION (row `file_sha256` equals disk, content not stamped embedded), then runs `index_repo_sync` with both force levers false, exactly as `index_repo` calls it, and expects the content to be queued.

Re-checked against HEAD first: the caveat still held (no test entered the trap via the STAMP).

The mutation the parent described, `update` also calling `set_embedded_sha256` with the new content's hash, is KILLED. **What that does and does not show:** the kill came from the PRECONDITION assertion, not the final queue assertion. So the stamping regression is guarded; the final assertion is not shown to catch anything the precondition does not (the indexer's embed decision is the sibling test's territory, `index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it`). The test was green on the shipped code, so there was no pre-fix red; the mutation stands in for it.

## References

- `docs/issues/archive/2026-09-04-doc-update-stamps-the-content-hash-without-rebuilding-chunks.md` — parent


## Fix provenance

- **SHA:** `464e36af465aa7e0f185a43a7ad8aec6e62a7cc1` (`experiments`)
- **patch-id:** `586cd910cf3c4f8d4de4addd2490b4f5385db453`
