---
id: c035180192023734
kind: bug
status: open
title: 'RESIDUAL: Extend the cross-host high-water guard and entry_defined_twice detection to params ledgers (params allocation path and ''| PREFIX-N |'' index tables)'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Extend the cross-host high-water guard and entry_defined_twice detection to params ledgers (params allocation path and '| PREFIX-N |' index tables).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` (status `mitigated`):

> Three open gaps, not one. (1) Peer-ahead direction: detection is complete but prevention is partial by construction and stays so — the guard catches only the direction where THIS host is ahead. `@{upstream}` is a remote-tracking ref, stale until someone fetches, so a peer who allocates and pushes while this host has not fetched still collides undetected, and an unpushed peer commit is unreachable by any local check. That is why status is `mitigated`, not `fixed`. (2) Params-ledger gap, concrete surface: both components cover PROSE ledgers only. Component B's guard sits inside the `a.entry_collection.is_none()` branch, so the params allocation path has no upstream guard. Component A is built on `entry_sections` (`## PREFIX-N — Title` headings), so a params ledger whose body carries an index TABLE rather than headings yields no EntrySection and a duplicate there is neither prevented nor detected. This is not closed by 'rows are machine-local': a params ledger's `entry_high_water_<PREFIX>` frontmatter AND its `| PREFIX-N |` index table are BOTH committed and BOTH merge — `body_claimed_indices` (`src/librarian/catalog/augmentation.rs:1282-1292`) counts index rows toward allocation, so the committed surface collides exactly like the prose one and is simply uncovered by either component. (3) Detector's real-world track record: a whole-branch review ran entry_defined_twice against this repo's corpus (1451 markdown files, 37 declared ledgers) and found 3 findings, all false positives, all sub-headings repeating their own entry's token (e.g. `### A-28` nested under `## A-28`); those are now excluded (0cb617cc) by dropping a definition strictly deeper than, and inside the span of, an earlier definition of the same token. Post-fix measured corpus output is zero findings — the check has never yet fired on a real collision; its true-positive rate is validated only against fixtures, not against a real occurrence.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` — parent
