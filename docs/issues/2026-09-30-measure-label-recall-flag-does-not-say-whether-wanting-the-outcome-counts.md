---
id: e3ca4c0b4589cb47
kind: bug
status: open
title: The label tool's recall flag does not say whether wanting to see the outcome counts as remembering it
tags:
- cluster/unclassified
closed: ''
opened: 2026-09-30
owner: marius
related: []
severity: low
---

# BUG: the label tool's recall flag does not say whether wanting to see the outcome counts as remembering it

## Summary

`label.py` asks for a recall flag: `y` if you remember how the case turned out from outside the packet.
The wording does not say that curiosity about the outcome is not recall. It also does not say that
looking the outcome up is not allowed and that there is no sanctioned way to do it. A labeller tempted to
look has no guidance, and a labeller who merely wonders may answer `y`. Either way the recall flag stops
separating the `without_recall_flagged` estimate cleanly.

## Symptom (Effect)

Walkthrough item 10 (relayed, no packet content):

```
The recall flag's wording does not say whether wanting to see the outcome counts as remembering it.
There is no sanctioned way to look at what happened next, and a reader tempted to look has no guidance.
```

## Reproduction

Read the strings below; no run is needed.

## Environment

`experiments` at `ee313cce`.

## Root cause

The four places the flag is described all define it as remembering the outcome, and none mentions
wanting to know it or looking it up:

- banner line (`scripts/measure/label.py:76`): `5. Recall: y if you remember how this turned out from
  outside the packet.`
- legend (`scripts/measure/label.py:85`), `LEGEND_RECALL`;
- prompt (`scripts/measure/label.py:204`), `RECALL_PROMPT`;
- echo (`scripts/measure/label.py:213-214`), `_echo`.

measured 2026-09-30 by reading those lines.

## Evidence

Walkthrough item 10; the strings cited above.

## Hypotheses tried

None.

## Fix

Reword the banner line and the legend. Suggested wording, which the operator should confirm:

- `y` only if you already remember how this case turned out, from before this labelling session.
- Wanting to know the outcome is not remembering it. Answer `n`.
- Do not look it up: no transcript, no git log, no tracker. There is no sanctioned lookup. If the packet
  alone cannot settle the case, the answer is `u`, not a lookup.

`label.py` is **not** one of the four files `run.py frame` hashes, so this change needs no re-freeze. The
banner is 24 lines; check it still fits the screen, and update any test that pins the banner's line count
or text.

## Tests added

None yet. Owed: the legend names both "remember" and "looking it up" (a shape test that fails if either
addressee is dropped, per the `CLAUDE.md` remedy-text rule).

## Workarounds

Tell the operator the rule directly before they label.

## Resume

Confirm the wording with the operator, then edit `scripts/measure/label.py` (banner at L76, legend at
L85) and `tests/test_measure_label.py`. Never open a real packet. Before committing under `scripts/`, run
`scripts/with-slot.sh cargo test --test committed_paths` (R39).

## References

- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
- Estimate that uses the flag: `scripts/measure/estimate.py` (`without_recall_flagged`).
