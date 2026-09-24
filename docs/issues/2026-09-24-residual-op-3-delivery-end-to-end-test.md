---
id: '6c50a7804a8e077c'
kind: bug
status: open
title: 'RESIDUAL: Write an end-to-end test driving memory(action=''write'') through call_content and asserting OP-3 delivery (the OP-4 sibling test in record 47 shows it is writable)'
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Write an end-to-end test driving memory(action='write') through call_content and asserting OP-3 delivery (the OP-4 sibling test in record 47 shows it is writable).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` (status `fixed`):

> OP-2 is NOT covered and never can be by this mechanism. It declares `Serves: Agent, Task`, which are Claude Code HARNESS tools; codescout has no such tools (verified against the served surface and `ls src/tools/`), so they never enter call_content and no selector_key work can route them. For OP-2 the selector_key gap named as this bug's root cause is not the binding constraint. Of the three triggered rules: OP-3 now routes; OP-4 has its routing precondition only and still cannot fire for an independent reason (see related); OP-2 is structurally unreachable. Also NOT verified end-to-end: the fix is covered by two tests meeting at a verified point — the real tools return Some(key), and route() delivers given a Some selector — but no test drives a real memory(action="write") call through call_content and asserts the OP-3 block appears.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` — parent
