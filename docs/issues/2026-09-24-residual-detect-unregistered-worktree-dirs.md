---
id: '1e153b9a3d5bb333'
kind: bug
status: open
title: 'RESIDUAL: Build a detector (doctor/probe) for .worktrees/ entries that are not registered worktrees, since both git worktree list and git status are blind to them'
tags:
- cluster/record-asserts-an-unchecked-completion
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build a detector (doctor/probe) for .worktrees/ entries that are not registered worktrees, since both git worktree list and git status are blind to them.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` (status `mitigated`):

> Mitigated, not fixed — and the residual is now closed by an UNATTRIBUTED removal rather than by the decision this file parked on. Measured 2026-09-01: `.worktrees/bench` is absent, `.worktrees/` holds only `audit-trail-t1`, and `git worktree list` reports neither — so the orphaned gitdir and the 163M are gone. WHO removed it and WHEN is not establishable: no commit in the last 60 mentions the bench worktree, and the `.worktrees/` mtime (02:24) is equally explained by `audit-trail-t1` being created in it, since a directory mtime records its last entry change and not which entry. No regression guard exists for the class: nothing prevents a record asserting an unchecked completion again, and `docs/trackers/retrieval-benchmark.md:76` agrees with reality today by accident rather than by repair. AMENDED 2026-09-02 — the 2026-09-01 reading closed a MEMBER, not the population, and the population regenerates on its own. `.worktrees/` today holds `audit-shards-t7`: 8K, absent from `.git/worktrees/` and from `git worktree list`, containing only a dead session's gitignored `.buddy/bf44ba81-4cb3-4fdc-a92b-0780646ca7b9/` and `.codescout/cc_session_id`. `audit-trail-t1` is gone and a different unregistered member replaced it inside 24h, so `the orphaned dirs are gone` was true of the instance and false of the class — the same member-vs-population cut CLAUDE.md names under Testing Discipline. The invisibility is doubly-instrumented, which is why nobody trips over it: `git worktree list` reports registrations and cannot see it, `git status` honours `.gitignore:133 .worktrees/` and cannot see it either — two correct instruments whose blind spots coincide, so agreement between them is one blind spot counted twice. Unlike the 2026-08-30 bench case, positive identification IS available and unused here: the residue names its own session id on disk, so the author is given rather than inferred from a directory mtime.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md` — parent
