---
id: '37bfe6d301006240'
kind: bug
status: open
title: The labelled-sample spec says a packet has no date, but packet.py keeps bare calendar dates by design
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: ''
opened: 2026-10-24
related: []
severity: low
---

# BUG: the labelled-sample spec says a packet has no date, but packet.py keeps bare calendar dates by design

## Summary

`docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule*, bullet *Blinding*, says: "no session id, date or absolute timestamp." The code in `scripts/measure/packet.py` blinds uuids, timestamps with hours, minutes and seconds, and API ids. It keeps a bare calendar date everywhere, and its module docstring says that is deliberate. A reader who trusts the spec expects a packet with no dates. A packet can hold many.

## Class fit

This bug shows the class trait that a document states a behaviour which the code contradicts. It differs from the class's other members because the code's own docstring records the choice as deliberate, so the code is not wrong and only the spec's wording is.

## Symptom (Effect)

The spec's wording suggests a packet leaks no date. The packet text can still show a bare date, for example in a file name such as `docs/issues/2026-10-24-…`. Found 2026-10-24 while a builder check on `docs/superpowers/specs/2026-10-24-intervention-judge-design.md` read both texts. That design had to choose which of the two its "no date" test follows.

## Reproduction

Run from `scripts/measure`:

```
python3 -c "import packet; print(repr(packet._Ids().blind('see docs/issues/2026-10-24-x.md written 2026-10-24, run at 2026-10-24 10:11:12 and 2026-10-24T10:11:12Z')))"
```

Output on 2026-10-24: `'see docs/issues/2026-10-24-x.md written 2026-10-24, run at <timestamp-1> and <timestamp-2>'`. Both bare dates stay. Both full timestamps become `<timestamp-N>`.

## Environment

`scripts/measure/packet.py` (module docstring, `_TIMESTAMP_RE`, `_Ids.blind`). The spec text is in `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule*.

## Root cause

The spec's one-line summary of blinding was wider than the rule the code implements. `_TIMESTAMP_RE` requires `HH:MM:SS`, so a bare date does not match. The docstring states the choice. Whether the spec or the docstring was written first was not checked.

## Evidence

The reproduction above, and `_TIMESTAMP_RE` read at `scripts/measure/packet.py` lines 67-68. Not established: whether any labelled packet actually contains a bare date, because no packet was rendered. The controller must not read labelled packets.

## Hypotheses tried

None. The cause is a wording difference between two texts.

## Fix

Open. One of these two, the operator's call:

- Narrow the spec's *Blinding* bullet to match the code: "no session id, no uuid, no timestamp with a time of day."
- Widen the code to blind bare dates too. That changes every packet's sha256, so any label bound to a packet hash would stop verifying. Do not choose this once labels exist.

The intervention-judge design (`docs/superpowers/specs/2026-10-24-intervention-judge-design.md`) follows the code and says so.

## Tests added

None. This is a wording defect in a spec. A test would pin the code's behaviour, which `tests/test_measure_packet.py` may already do. Whether it does was not checked.

## Workarounds

Read the code's rule: only timestamps with a time of day are blinded.

## Resume

Decide which text changes. If the spec changes, edit that one bullet and cite this file.

## References

- `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule*
- `scripts/measure/packet.py` module docstring
- `docs/superpowers/specs/2026-10-24-intervention-judge-design.md` § *Disclosures*
