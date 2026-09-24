---
id: '3ff543cad7b4fb22'
kind: bug
status: open
title: 'RESIDUAL: Author section-use signatures for the other nine guide topics and distinguish a missing guide file from ''no rules'' in topics_with_rules()'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Author section-use signatures for the other nine guide topics and distinguish a missing guide file from 'no rules' in topics_with_rules().

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md` (status `fixed`):

> No regression test — `scripts/` has no test harness in this repo, so nothing gates the guard itself. The evidence is an observed refusal (exit 2) and an observed pass on the ruled topic, not a check that runs when nobody is looking; a later edit could remove the guard and no gate would fire. Also NOT done, deliberately: signatures for the other nine topics remain unauthored, so nine of ten topics are still unmeasurable — the guard converts a false answer into a refusal, which is the fix, but it does not extend coverage and this file should not be read as saying the probe now works for `librarian`. Finally, `topics_with_rules()` swallows a missing or unreadable guide file as 'unmeasurable' rather than distinguishing it from 'no rules' — correct for the refusal path, but it means a deleted guide would present as a rules gap.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md` — parent
