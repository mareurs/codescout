---
id: '98311e8c2c64bab8'
kind: bug
status: open
title: 'RESIDUAL: Fix in-session guide starvation of subagents (ledger suppression by shared session_id); the named agent_type lever does not reach the in-memory ledger'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Fix in-session guide starvation of subagents (ledger suppression by shared session_id); the named agent_type lever does not reach the in-memory ledger.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md` (status `fixed`):

> UPDATED 2026-09-11 -- see that section for full evidence. STILL NOT FIXED, but the shape changed: (1) hook confirmed by direct code read to ignore agent_type -- solid, not inferred. (2) Historical corpus re-derived at n=1,026 distinct subagent sessions (deduped across a newly-found cross-profile transcript-duplication artifact), 98.4% double-delivery by the file's original test -- but a full-corpus scan found ZERO transcripts with a fork-shaped opening (replayed parent history), so this corpus may contain no material fork population at all. (3) Live-reproduced: a genuinely fresh, non-fork subagent was silently denied a topic (progressive-disclosure) the parent already had -- direct verbatim evidence, not inferred. (4) Reframe: ledger suppression depends only on delivery ORDERING within the shared session_id, identical for fork and fresh -- so the fork/fresh split is not the load-bearing variable after all. (5) IMPORTANT CORRECTION: the file's own named lever (wiring agent_type into agent-guide-snapshot.mjs) would NOT fix the starvation just reproduced -- that hook only ever mutates the on-disk ledger for the NEXT server reconnect, never the in-memory ledger the running session's server actually consults. Still unestablished: what a fix for LIVE, in-session starvation would even look like, given no MCP client sends per-request caller identity today (design doc's own Out of scope). Also unestablished: origin of the 228-transcript cross-profile duplication found during re-derivation -- separate from this bug, not yet filed.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md` — parent
