---
id: 1503dca6c81fc3d4
kind: bug
status: fixed
title: 'RESIDUAL: Fix in-session guide starvation of subagents (ledger suppression by shared session_id); the named agent_type lever does not reach the in-memory ledger'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-24
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

**Fixed — verified live 2026-09-24, no new commit.** The caveat this file was routed from was written on 2026-09-11 ahead of the parent's own § *Fix — 2026-09-11*, which shipped the live re-arm the caveat called unknown. Its remaining blocker — *"no MCP client sends per-request caller identity"* — was retired on 2026-09-14 by the principal stamp (`docs/adrs/2026-09-14-a-subagent-is-a-principal.md`): a subagent's calls carry `<session_id>/<agent_id>`, and `adopt_request_conversation` (`src/server.rs:1255-1298`) serves each principal its own ledger.

**Live probe, 2026-09-24, session `774ba049-d97c-443a-b31d-f662a9cb6a1e`.** The parent held `project-activation-bootstrap`, `symbol-navigation`, `progressive-disclosure` and `tracker-conventions`. A fresh `general-purpose` Sonnet child made three `symbols(path=…)` calls. Read from the child's own transcript, not its report: call 1 carried `project-activation-bootstrap`, call 2 **`symbol-navigation` — a topic the parent held** — and call 3 nothing. The child's principal ledger (`guide_hints/774ba049-…_<agent>.json`) holds exactly those two. Not starved.

**Two mechanisms each suffice, and this probe does not separate them.** The child's principal ledger started empty (adoption), and the snapshot hook's re-arm request — `guide_rearm/3393665-aec45ce254028293.json`, carrying the parent's four topics, created 15:06:44.97Z — was consumed by the child's own first call (gone 15:06:49.25Z). Both are cited under § *Fix provenance*. Separating them would take a mutation of one with the other in place.

**The opposite direction is not this file's work and has its own home.** The same probe's `fork` arm was re-served both guides it had inherited: `docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md`. The parent caveat's other loose end, the cross-profile transcript duplication, is `docs/issues/2026-09-11-subagent-transcripts-are-byte-identical-across-profile-dirs.md`.

**Regression tests in place:** `guide_hint_tests::a_principal_stamped_into_the_arguments_rearms_the_ledger_once` (`src/server.rs:10686`), step 3 — *"a newly asserted principal must be re-armed"* — with steps 1–2 as its positive control; and the `GuideRearmInbox` unit tests in `src/tools/guide_rearm.rs`.

## Fix provenance

- **SHA:** `5e51e72f` (`experiments`)
- **patch-id:** `25b71040d3ddfbd47d2b07a3104755c8e1655674`
- **SHA:** `claude-plugins:de7d16a`
- **patch-id:** `3d651075ea8daa4901b31c425467d47960992989`
- **SHA:** `5a260862` (`experiments`)
- **patch-id:** `da6856c700fc24bf182625eda246448cbe0d9b67`
- **SHA:** `claude-plugins:9398e3a`
- **patch-id:** `374a9ec583412cf189aea7b115c9725977036712`

## References

- `docs/issues/archive/2026-08-31-subagents-receive-guides-their-parent-already-holds.md` — parent
