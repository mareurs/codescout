---
id: b9dcc12ccc143cc7
kind: bug
status: fixed
title: 'BUG: the T-set loop in freeze_stage2.py has no twin-in-context_before guard, and the record that deferred it is wrong about which rows are in T'
tags:
- evals
- rule-tell
- stage2
- label-leak
- cluster/guard-narrower-than-its-name
closed: 2026-10-16
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

Applied (experiments-only, unmerged until the lead ships; fix SHA `fa782967`, patch-id `d12ce21028e12a0124a1af323e1937bba5cc1ea5`): option 1. The T loop moved out of `main()` into `t_mined_rows(rows, split, lab, mp, n)` in `freeze_stage2.py`, which calls `mp.twin_in_context_before(r)` and skips, counting skips on stdout (`mined T rows skipped, twin already in context_before:`) and NOT in `n`, so the manifest bytes cannot move.

Verified 2026-10-16: the claim reproduced first (flagged `[(267,'T','not-a-violation'), (941,'T','not-a-violation')]`, neither in `frozen/T.jsonl`). After the change `python3 freeze_stage2.py` rewrites `frozen/*` byte-identically (`git status` clean for the data dir; sha256 lines unchanged), skip count 0, as expected while the label filter keeps both rows out.

## Tests added

`tests/test_stage2_synthetic.py::FreezeTSetTwinGuard` (3 cases, run with `python3 -I tests/test_stage2_synthetic.py`): a T row with its twin in `context_before` is skipped and counted; the same row without the twin there is kept as pos+neg (the control: a guard that skips every T row fails it); a leaking row outside T or with a dropped label is not counted as skipped. Mutation probe: deleting the guard from `t_mined_rows` makes the first case fail (rc 1); restored, all 24 + 32 stage2 tests pass.

## Workarounds

None needed today. Re-run the measurement above after any re-labelling of T rows.

## Resume

Done. Archive (`doc(action="move")` to `docs/issues/archive/`) once the gate is green on experiments.

## References

- `docs/issues/archive/2026-10-01-residual-mined-rows-with-the-twin-already-in-context-before.md`, "Not done" and Resume.
- `docs/issues/archive/2026-09-24-stage2-miner-positive-context-is-the-corrected-text.md`: the parent.
- Cluster `IC-14`: the twin guard is named for the freeze and covers one of its two loops.
