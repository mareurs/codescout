---
id: '37bfe6d301006240'
kind: bug
status: fixed
title: The labelled-sample spec says a packet has no date, but packet.py keeps bare calendar dates by design
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-30
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

Fixed 2026-10-30 by narrowing the spec, the first of the two options. The code stays as it is, so no packet hash changes. The *Blinding* bullet of `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule* now says: no session id (a uuid becomes `<uuid-N>`), no API id (`<id-N>`) and no timestamp with hours, minutes and seconds (`<timestamp-N>`); a bare calendar date is kept, in prose and in file names.

Checked against the code on 2026-10-30: the reproduction above still prints both bare dates and `2026-09-20 10:11` unchanged, and both full timestamps become `<timestamp-N>`.

The operator chose this option on 2026-10-30, when they answered "ok, go" to a recommendation that named it. The other option, widening the code, stays unchosen: the record advises against it once labels exist.

## Tests added

None new. `tests/test_measure_packet.py::Blinding::test_a_bare_calendar_date_is_kept_and_only_a_date_with_a_time_is_blinded` (line 835) already pins the code's side: a bare date and a `HH:MM` value stay, a `HH:MM:SS` value is blinded, and the module docstring says so. The spec now agrees with that test.

## Workarounds

Read the code's rule: only timestamps with a time of day are blinded.

## Resume

Decide which text changes. If the spec changes, edit that one bullet and cite this file.

## References

- `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § *Packet rule*
- `scripts/measure/packet.py` module docstring
- `docs/superpowers/specs/2026-10-24-intervention-judge-design.md` § *Disclosures*
