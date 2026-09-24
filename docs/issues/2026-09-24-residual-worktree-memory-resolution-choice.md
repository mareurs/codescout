---
id: b259645d3d2c9214
kind: bug
status: open
title: 'RESIDUAL: Decide whether a worktree activation resolves .codescout/memories/ against the main checkout (overlay like the librarian) and implement the choice'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide whether a worktree activation resolves .codescout/memories/ against the main checkout (overlay like the librarian) and implement the choice.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md` (status `mitigated`):

> only the topology half is closed (1869adcb); the memory half is unfixed and is a decision rather than an implementation — a worktree activation still serves that commit's memories, so the memory set silently diverges from the main checkout. See section 'Still open — the semantic question'.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md` — parent
