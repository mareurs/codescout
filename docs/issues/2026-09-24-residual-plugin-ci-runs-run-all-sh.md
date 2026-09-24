---
id: '5cbaa87d92772a51'
kind: bug
status: open
title: 'RESIDUAL: Make CI invoke the plugin repo''s tests/run-all.sh runner so skill-colocated tests actually run; add a test for the self-identification half'
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make CI invoke the plugin repo's tests/run-all.sh runner so skill-colocated tests actually run; add a test for the self-identification half.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md` (status `fixed`):

> The regression test covers the name read only. The self-identification half of the same commit is verified by hand, not by test — see the sibling bug file. The test is newly reachable: `tests/run-all.sh` globbed hooks only until 2026-09-03, so a skill-colocated test was discovered by nothing; the glob was widened in the same commit and has not yet run in CI, which invokes two named targets rather than the runner.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md` — parent
