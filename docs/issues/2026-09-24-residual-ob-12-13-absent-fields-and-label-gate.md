---
id: '7879e82a542864d3'
kind: bug
status: fixed
title: 'RESIDUAL: Author the four absent OB-12/OB-13 fields and add a gate on the ledger''s field-label form'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-30
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md
severity: low
unverified: 'The 30 absent fields were backfilled as explicit `unknown` lines (commit 6e5b57e05), not authored: no entry''s real value for Class, Blind party, Plausible-answer property, Vigilance, Instances, Status or Rests on was written. They are placeholders an owner can replace. The gate (commit 5c3ac45b7) checks that a label is present and line-anchored, not that its content is true. Both commits are local, not pushed. The Index rows of the 12 entries were not touched.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Author the four absent OB-12/OB-13 fields and add a gate on the ledger's field-label form.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md` (status `mitigated`):

> No regression test — nothing gates the field-label form, so the corpus can drift back the same way tomorrow. Four fields remain genuinely ABSENT (OB-12 Status; OB-13 Plausible-answer property, Vigilance, Status) and were deliberately not authored. The 6 repaired labels were verified by re-running the documented grep; the absent four were not repaired at all.

## Fix

Gate half shipped 2026-10-30 in commit `5c3ac45b7`: `tests/ledger_field_labels.rs` fails on any template label written in the single-bold-span form (none today), and pins the 30 absent (entry, field) pairs in a list that only shrinks, so a new absence fails and a repair must shrink the list. Each of three staged ledger mutations (a drifted label, a dropped field, a repaired field left in the pin) failed the gate meant for it. Authoring half closed 2026-10-30 in commit `6e5b57e05` as an `unknown` backfill: 30 lines at the end of 12 entries, none changed, `KNOWN_ABSENT` emptied. The template says to write `unknown` or `none yet` rather than drop a field, and `unknown` states no claim for an entry; it is not the authored value the record first asked for. The count grew from the parent's four to 30 because the ledger grew from 14 to 28 entries and older entries lack `Instances`, a later template field. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md` — parent
