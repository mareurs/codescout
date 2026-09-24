---
id: c269b760403ff5c9
kind: bug
status: open
title: 'RESIDUAL: Install Git for Windows in the wine CI image and drop the 22-test cfg(windows) skip block'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Install Git for Windows in the wine CI image and drop the 22-test cfg(windows) skip block.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md` (status `mitigated`):

> Windows-only fix, never exercised on this host — its tests are cfg(windows), and the wine CI image still ships without Git for Windows, so the 22-test skip block stands. Root cause (install Git in the image, drop the skip block) is unaddressed, which is why this is mitigated rather than fixed.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md` — parent
