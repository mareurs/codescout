---
id: '4d23f609590dea62'
kind: bug
status: open
title: 'RESIDUAL: record run_command''s observed effect class alongside the declared one'
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md`
(status `fixed`), whose `unverified:` caveat named it and was the only surface reporting it.
Routed here on 2026-09-24; the parent's caveat now opens `TRACKED <this file's id>`, and
`doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves
without the work.

**The work:** record the OBSERVED effect class of a `run_command` call — what it actually did
to the tree — alongside the DECLARED one already in `usage.db.effect_class`, so a wrong
declaration is detectable after the fact.

## Parent caveat, verbatim

> Only the DECLARED effect class is recorded. The OBSERVED one (what a command actually did to
> the tree) is not, and the Fix section asks for both because only the observation survives a
> wrong declaration.

## Fix

Not started. The parent's § Fix holds the design context (#56's operator ruling: declare and
record, no lock). Re-check against HEAD first.

## References

- `docs/issues/archive/2026-09-20-run-command-never-overrides-is-write.md` — parent
