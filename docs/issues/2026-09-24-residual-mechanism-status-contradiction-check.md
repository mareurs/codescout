---
id: '604bfbcecf4602d5'
kind: bug
status: open
title: 'RESIDUAL: Build a check that flags a **Mechanism status:** worklist field contradicted by a later commit'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build a check that flags a **Mechanism status:** worklist field contradicted by a later commit.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md` (status `fixed`):

> No regression test, and § *Tests added* records that as a deliberate finding rather than an omission: a test would have to assert that no `Mechanism status` field in the corpus contradicts a later commit, which is the unbuilt mechanism itself and not a test of this fix. Recurrence of the class is therefore unguarded — the corrected field is pinned, the mechanism that would keep it correct is not.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md` — parent
