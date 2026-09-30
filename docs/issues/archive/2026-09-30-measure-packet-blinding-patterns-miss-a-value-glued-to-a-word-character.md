---
id: c9e52d2caee49df8
kind: bug
status: archived
title: A labelling packet's blinding patterns miss a timestamp, UUID or API id glued to a word character
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-30
opened: 2026-09-30
owner: marius
related:
- measure-packet-placeholders-merge-distinct-ids-into-one-token
severity: medium
---

# BUG: a labelling packet's blinding patterns miss a timestamp, UUID or API id glued to a word character

## Summary

`_blind` masks UUIDs, timestamps and API message/tool ids so a packet cannot be traced back to its
session. All three patterns start with `\b`, and a word boundary does not exist between two word
characters. So a value glued to a preceding letter, digit or underscore (`run2026-09-30T12:00:00Z`,
`x1111…`, `idmsg_01…`) passes through unmasked. The walkthrough saw this for a time-like value, and the
same mechanism covers the other two patterns.

## Symptom (Effect)

Walkthrough item 6 (relayed, no packet content):

```
A time-like value embedded inside a longer token was not masked, a small gap in the time masking.
```

## Reproduction

From `scripts/measure/`:

```
python3 -c "
import packet as p
for s in ['run2026-09-30T12:00:00Z', 'id_2026-09-30T12:00:00Z', 'x11111111-2222-3333-4444-555555555555',
          'idmsg_01ABCDEFGHIJKLMNOPQRSTUVWX', 'v1.2026-09-30 12:00:00', 'a msg_01ABCDEFGHIJKLMNOPQRSTUVWX']:
    print(repr(s), '->', repr(p._blind(s)))"
```

## Environment

`experiments` at `ee313cce`; python3.

## Root cause

- `_UUID_RE` (`scripts/measure/packet.py:45`), `_TIMESTAMP_RE` (`scripts/measure/packet.py:46`) and
  `_API_ID_RE` (`scripts/measure/packet.py:48`) each begin with `\b`. `_blind`
  (`scripts/measure/packet.py:104-109`) applies them in turn.
- `\b` matches only between a word and a non-word character. After `.` or a space the value is masked;
  after a letter, digit or `_` it is not.

measured 2026-09-30 with the reproduction above:

```
'run2026-09-30T12:00:00Z' -> 'run2026-09-30T12:00:00Z'
'id_2026-09-30T12:00:00Z' -> 'id_2026-09-30T12:00:00Z'
'x11111111-2222-3333-4444-555555555555' -> 'x11111111-2222-3333-4444-555555555555'
'idmsg_01ABCDEFGHIJKLMNOPQRSTUVWX' -> 'idmsg_01ABCDEFGHIJKLMNOPQRSTUVWX'
'v1.2026-09-30 12:00:00' -> 'v1.<timestamp>'
'a msg_01ABCDEFGHIJKLMNOPQRSTUVWX' -> 'a <id>'
```

## Evidence

Probe output above, plus walkthrough item 6.

## Hypotheses tried

None.

## Fix

**Landed in `1f42d84a`, patch-id `4e27ab51269db2579fe05550cfc6515b761d03e9`** (one commit shared with `f32b7d24` and `05fe7d98`). Done as below, and the trailing anchors were changed too: UUID `(?![0-9a-fA-F])`, timestamp `(?!\d)` after the seconds field.

Replace each leading `\b` with a lookbehind that forbids only what would make the match a fragment of a
longer value of the same kind:

- timestamp: `(?<!\d)`;
- UUID: `(?<![0-9A-Fa-f])`;
- API id: no lookbehind needed, since `msg_01`/`toolu_01` is a distinctive prefix.

Check the trailing anchors the same way, because a value glued on the right has the same failure. The
module docstring (`scripts/measure/packet.py:9`) states the timestamp scope; update it too. This is a
blinding fix, not a rule change, so it needs no operator ruling.

## Tests added

Three tests in `Blinding` (`tests/test_measure_packet.py`), one per pattern: a value glued on the left and on the right is masked, and look-alikes are left alone (a fifth digit before the year, a third digit of seconds, hex glued to a UUID on either side, a non-hex character inside the last group, 19 characters after `_01`). The look-alike cases are the widening direction, which the masked cases cannot catch.

Mutations (9, on a scratchpad copy): UUID lead and trail each changed to `\b` and dropped, timestamp lead to `\b` and dropped, timestamp trailing anchor dropped, API-id `\b` restored, API-id length bound 20 to 10: all KILLED. Not run: the cargo gate (Python only); left un-archived.

## Workarounds

None.

## Resume

Write the failing tests in `tests/test_measure_packet.py`, then change the three patterns. Constraints are
in the Resume of `measure-packet-cuts-results-and-arguments-without-a-marker`: `packet.py` is frozen by
`run.py frame`, so batch the fix with the other `measure-packet-*` fixes, and never read a real packet.

## References

- Spec: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` L143 onwards (blindness).
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.

## Fix provenance

- **SHA:** `1f42d84a7201ce99e624eac18b10adf41e0ed671` (`experiments`)
- **patch-id:** `4e27ab51269db2579fe05550cfc6515b761d03e9`
