---
id: '8829a4aade8bef7f'
kind: bug
status: open
title: A labelling packet's blinding patterns miss a timestamp, UUID or API id glued to a word character
tags:
- cluster/guard-narrower-than-its-name
closed: ''
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

Replace each leading `\b` with a lookbehind that forbids only what would make the match a fragment of a
longer value of the same kind:

- timestamp: `(?<!\d)`;
- UUID: `(?<![0-9A-Fa-f])`;
- API id: no lookbehind needed, since `msg_01`/`toolu_01` is a distinctive prefix.

Check the trailing anchors the same way, because a value glued on the right has the same failure. The
module docstring (`scripts/measure/packet.py:9`) states the timestamp scope; update it too. This is a
blinding fix, not a rule change, so it needs no operator ruling.

## Tests added

None yet. Owed: for each of the three patterns, a value glued on the left and one glued on the right are
masked. A value that only resembles one is left alone: a 13-digit number next to a date, and a UUID-like
string with a non-hex character. That last case catches the widening direction. Then one mutation per
pattern.

## Workarounds

None.

## Resume

Write the failing tests in `tests/test_measure_packet.py`, then change the three patterns. Constraints are
in the Resume of `measure-packet-cuts-results-and-arguments-without-a-marker`: `packet.py` is frozen by
`run.py frame`, so batch the fix with the other `measure-packet-*` fixes, and never read a real packet.

## References

- Spec: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` L143 onwards (blindness).
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
