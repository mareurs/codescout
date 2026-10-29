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
severity: medium
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide whether a worktree activation resolves .codescout/memories/ against the main checkout (overlay like the librarian) and implement the choice.

## Measured cost (2026-10-24 usage sweep)

In 2026-10-03..10-24, `memory_target_not_found` fired **98 times across 12 projects**, and it has the worst recovery of any family above 50 calls: **29.6%** of the next calls repeat the same family and **31.6%** of the agents make no successful `memory` call within the next 25 calls. The largest group is git worktrees of projects whose memories are not committed: in `mirela/eduplanner-ui/.worktrees/*`, reads of `conventions`, `gotchas`, `language-patterns` and `innovaplan-*` returned `topic '…' not found — hint: no memory topics exist yet`, so the worktree session sees **zero** memories, not a diverged set. In `codescout/.claude/worktrees/agent-*`, topics written after the branch point (`catalog-sql-hazards`, `worktree-merge-catalog-reconciliation`) were not found while 16 older topics were. This is evidence for raising the severity: the divergence is total in projects that do not commit `.codescout/memories/`.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md` (status `mitigated`):

> only the topology half is closed (1869adcb); the memory half is unfixed and is a decision rather than an implementation — a worktree activation still serves that commit's memories, so the memory set silently diverges from the main checkout. See section 'Still open — the semantic question'.

## Fix

Not started. Severity raised from low to medium on 2026-10-29 by the operator, on the file's own measurement (98 `memory_target_not_found` events in 12 projects, the worst recovery of any family above 50 calls) and the re-check below. Re-checked against HEAD `b15dd5279` on 2026-10-29: `memory(action="list")` pinned to the linked worktree `.worktrees/setup-ab-eval` returned 26 topics, while the main checkout lists 31. The five topics only the main checkout has (`bug-triage-2026-10-27`, `research/cross-harness-session-messaging`, `research/effort-steering/behavior-transfer-20261029`, `research/effort-steering/compile-mechanism-20261029`, `research/setup-ab-eval`) are not in that worktree's committed set. The divergence is still live, so the caveat was not overtaken. Not established: whether those five are uncommitted in the main checkout (likely, since git status lists some of them as untracked), or how the worktree resolves its memory root. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md` — parent
