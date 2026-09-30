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

Ruled by the operator 2026-09-30: a short banner line, with the full rule in the recall legend. The banner is
exactly at its screen caps (24 lines, 79 columns, pinned by `test_banner_fits_one_screen_and_is_plain_ascii`),
so the four-line wording suggested here would have redded that test.

- Banner item 5, one line (76 columns): `y only if you remember the outcome. Just curious = n. No lookups.`
- `LEGEND_RECALL`, three lines, printed before the first recall prompt of every case and again after an invalid
  answer (`_ask_until`): `y` only if you already remember how the case turned out, from before this session;
  wanting to know is not remembering, answer `n`; never look it up (no transcript, git log or tracker); if the
  packet alone cannot settle the case, that is label `u`, not a lookup.

The `u` advice sits in the legend and not on the recall line because the recall prompt accepts only `y` or `n`:
`u` is a label at the labels prompt.

Not changed: `RECALL_PROMPT` and the `_echo` line. Both still read as "remember how this turned out", which the
legend now qualifies at the moment of asking; say so if the echo should carry "from before this session" too.

`label.py` is not one of the four files `run.py frame` hashes, so no re-freeze is owed for this change.

## Tests added

In `tests/test_measure_label.py`:

- The golden banner (item 5) and `L_RECALL` were changed first and watched red: five failures, all the old
  production text against the new golden text.
- `test_the_recall_rule_answers_each_state_a_labeller_can_be_in` names each state a labeller can be in and the
  reply the legend gives it: remembers, y; only curious, n; tempted to look, refused, with label `u` as the
  alternative. A shape test buys arrival, not answerability, so the states are enumerated.
- The path to the labeller is covered by the existing `test_a_legend_is_printed_above_each_prompt` and
  `test_invalid_answers_say_what_was_wrong_and_reprint_the_legend_once`, both of which pin `L_RECALL`.

**Mutation run, isolated worktree, 7 mutants, 7 KILLED, 0 survived** (read off unittest's summary, the probe's own
verdict parse being cargo-only): the curiosity rule flipped in the legend and in the banner; the banner grown past 79
columns; the legend no longer delivered to the guide channel; the lookup ban dropped; the alternative label changed
from `u` to `n`; the remember-only clause dropped. 807 measure tests pass.

## Workarounds

Tell the operator the rule directly before they label.

## Resume

Confirm the wording with the operator, then edit `scripts/measure/label.py` (banner at L76, legend at
L85) and `tests/test_measure_label.py`. Never open a real packet. Before committing under `scripts/`, run
`scripts/with-slot.sh cargo test --test committed_paths` (R39).

## References

- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
- Estimate that uses the flag: `scripts/measure/estimate.py` (`without_recall_flagged`).
