---
id: '7879e82a542864d3'
kind: bug
status: open
title: 'RESIDUAL: Author the four absent OB-12/OB-13 fields and add a gate on the ledger''s field-label form'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md
severity: low
unverified: 'The gate half is shipped and the authoring half is not. Not done: authoring the absent fields. Re-measured 2026-10-30 on 28 entries: 30 template fields are absent across 12 entries, and 0 are in the drifted single-bold-span form. The four fields this record names (OB-12 Status; OB-13 Plausible-answer property, Vigilance, Status) are still among them. Whether to write the 30 fields, or to accept `unknown` for the older entries, is the owner''s decision; the template says `Drop no field — write unknown or none yet`. The gate (commit 5c3ac45b7) is local, not pushed.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Author the four absent OB-12/OB-13 fields and add a gate on the ledger's field-label form.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md` (status `mitigated`):

> No regression test — nothing gates the field-label form, so the corpus can drift back the same way tomorrow. Four fields remain genuinely ABSENT (OB-12 Status; OB-13 Plausible-answer property, Vigilance, Status) and were deliberately not authored. The 6 repaired labels were verified by re-running the documented grep; the absent four were not repaired at all.

## Fix

Gate half shipped 2026-10-30 in commit `5c3ac45b7`: `tests/ledger_field_labels.rs` fails on any template label written in the single-bold-span form (none today), and pins the 30 absent (entry, field) pairs in a list that only shrinks, so a new absence fails and a repair must shrink the list. Each of three staged ledger mutations (a drifted label, a dropped field, a repaired field left in the pin) failed the gate meant for it. Authoring half not started: nothing was written into the ledger. The count grew from the parent's four to 30 because the ledger grew from 14 to 28 entries and older entries lack `Instances`, a later template field. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md` — parent
