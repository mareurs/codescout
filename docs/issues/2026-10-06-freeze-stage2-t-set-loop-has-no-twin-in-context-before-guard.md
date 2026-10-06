---
id: b9dcc12ccc143cc7
kind: bug
status: open
title: 'BUG: the T-set loop in freeze_stage2.py has no twin-in-context_before guard, and the record that deferred it is wrong about which rows are in T'
tags:
- evals
- rule-tell
- stage2
- label-leak
- cluster/guard-narrower-than-its-name
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-10-01-residual-mined-rows-with-the-twin-already-in-context-before.md
severity: low
---

# BUG: the T-set loop in `freeze_stage2.py` has no `twin_in_context_before` guard, and the record that deferred it is wrong about which rows are in T

## Summary

`freeze_stage2.py` skips a mined row whose twin already sits inside its own `context_before`, because that row leaks the label. The skip is in the loop that feeds train, val and cal. The loop that builds the held-out T set from the same rows has no such check. Nothing leaks today, but the archived record that deferred this says "none of the ten ids is in T today", and two of the ten are in the T split.

## Symptom (Effect)

`docs/evals/data/2026-09-24-rule-tell/stage2/freeze_stage2.py`:

- Lines 133-138 (the `split == "rest"` loop) call `mp.twin_in_context_before(r)` and skip.
- Lines 168-173 (the `split == "T"` loop) build the positive from `r["context_before"]` and the negative from `r["context_after"]` with no such check.

Measured 2026-10-06 at `10e935e3`, with a short script over `mined-candidates.jsonl` and `t-split.jsonl`, using the predicate of `mine_pairs.twin_in_context_before` (whitespace-normalised containment):

```
rows by split {'T': 237, 'rest': 707}
twin-in-context_before by split {'T': 2, 'rest': 8}
T ids flagged [267, 941]
```

Both flagged T rows have the label `not-a-violation` in `agent-labels.jsonl`. The T loop's `lab[i] in mp.sel.RULES` filter (line 169) therefore drops them. Neither id appears in `frozen/T.jsonl` (39 rows): a search for `mined-267:` and `mined-941:` finds nothing.

So no leak reaches the frozen T set today. A re-label of either row to a menu rule would put a leaking positive into T.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

Run the script in the measurement above from the repository root. It reads two committed files and writes nothing.

## Environment

Linux, Python 3.14, `experiments` at `10e935e3`.

## Root cause

The guard was added to one of two loops that build mined positives from `context_before`. Commit `b45bcce4` closed the archived record `docs/issues/archive/2026-10-01-residual-mined-rows-with-the-twin-already-in-context-before.md` as "a guard on the latent defect" and listed the T loop as not done.

That record's text says: "None of the ten ids is in T today (see Evidence)". Measured above: two of the ten ids (267 and 941) are in the T split. Its other claim, that none of the ten reaches a frozen set, is true. The two are kept out by the label filter, not by the split.

## Evidence

The measurement above, and `freeze_stage2.py:136` against `:168-173`.

## Hypotheses tried

1. **Hypothesis:** a flagged row is in the frozen T file. **Test:** search `frozen/T.jsonl` for the ids. **Verdict:** rejected. The label filter excludes them.

## Fix

Not started. Options:

- Apply `mp.twin_in_context_before(r)` in the T loop and count the skips on stdout, as the first loop does.
- Or document why T is exempt, and add an assertion that no frozen T row has its twin in its `context_before`.

Add a freeze-level test either way. The first loop's skip is not unit-tested either (the archived record says so).

## Tests added

N/A — not fixed.

## Workarounds

None needed today. Re-run the measurement above after any re-labelling of T rows.

## Resume

Do the first option. It is a two-line change in one loop.

## References

- `docs/issues/archive/2026-10-01-residual-mined-rows-with-the-twin-already-in-context-before.md`, "Not done" and Resume.
- `docs/issues/archive/2026-09-24-stage2-miner-positive-context-is-the-corrected-text.md`: the parent.
- Cluster `IC-14`: the twin guard is named for the freeze and covers one of its two loops.
