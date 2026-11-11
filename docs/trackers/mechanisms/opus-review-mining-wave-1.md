---
id: '8c40e1b7b4e51a17'
kind: tracker
status: active
title: Opus review mining — wave 1
owners:
- marius
tags:
- mechanisms
- review-mining
- measurement
time_scope: '2026-11-11'
---

# Opus review mining — wave 1 (2026-11-11)

## Scope

- Population: 671 subagents with `model: opus` and "review" in the description, in the four profiles' codescout project dirs (2026-08 to 2026-11). 663 have a report over 500 characters.
- Sample: 80 reviews, 10 per shard in 8 shards, drawn round-robin per calendar month (seed 1). A 2-review pilot used other reviews (seed 7) and is not counted.
- Miner: `haiku`, one per shard, brief `tools/miner-brief.md`. Pilot first: 11 of 13 quotes matched literally; the other 2 differed only in markdown emphasis (`*zero*`), so the verifier now ignores emphasis and all 13 match.
- Verification: each finding's `evidence` quote must occur literally in its source review (markdown emphasis ignored). 624 of 637 findings passed. 13 were dropped, 6 of them from shard-6.

## Counts (verified findings, n = 624)

| Category | Findings | Reviews with at least one (of 80) |
|---|---|---|
| test-weak | 137 | 58 |
| wrong-behavior | 94 | 44 |
| doc-drift | 86 | 44 |
| edge-case | 79 | 44 |
| other | 55 | 39 |
| spec-gap | 48 | 16 |
| report-inaccuracy | 42 | 17 |
| error-handling | 42 | 27 |
| naming-or-structure | 17 | 16 |
| concurrency-or-ordering | 15 | 10 |
| dead-or-unwired | 5 | 5 |
| scope-creep | 4 | 3 |

- Verdicts: 28 rejected, 45 approved_with_issues, 3 approved, 4 unclear (the miner mapped the reviewers' prose verdicts onto this set).
- 133 findings come from re-review reports; 314 from first reviews; 177 have no flag. Re-reviews may repeat a first-review defect, so the totals double-count some defects.
- Of 146 findings the reviewers rank critical, important, high, major or blocker: wrong-behavior 42, test-weak 41, spec-gap 13, doc-drift 13, edge-case 12.

## Observations

1. **Weak tests are the most common class and appear in 72% of reviews.** 41 of the 146 top-ranked findings are test-weak. Typical form: a mutant survives the suite (an assertion pins less than its message claims, a needle is a substring of another row, a guard has no test).
2. **Doc drift is the third class.** Typical form: a message, comment or doc keeps the old wording after a change (a "within 10 lines" message after the window grew to 12).
3. **Some defects sit in the implementer's own report.** 42 report-inaccuracy findings, for example an arithmetic rounding error, an overstated claim, or a gate that was "never shown green as one run". In the pilot, a fix report pasted probe output that the reviewer showed to be reconstructed, not real.
4. **Haiku labels 305 of 624 findings `mechanical: yes`** (a script, grep or test pattern could catch it) and 242 `implementer_preventable: yes`. By category: test-weak 100 of 137 mechanical; dead-or-unwired 5 of 5; wrong-behavior 54 of 94. These labels are the miner's opinion.

## Limits

- Labels `mechanical` and `implementer_preventable` are unverified opinions of a small model. Do not build on them before a hand check of a sample.
- 12% of the population. Reviews may differ by task type and by period (the stratification is by month only).
- Categories are the miner's choice from a closed list; `other` holds 55 findings. Miners mapped verdict wording differently (shard 7 mapped "Needs fixes" to approved_with_issues, shards 2, 4, 5, 8 mapped it to rejected), so the verdict counts are not exact.
- One miner (shard 7) reported missing 5 Minor findings of one review, and said it did not re-check its quotes by hand. The quote check passed for it anyway, but omitted findings are not detected by that check.
- Per-review counts, not per-task: a task with a fix round appears in several reviews.
- Miners could not use `Read` and `Write` (blocked by hooks) and used codescout `read_file` and `create_file`. This does not change the content read, but several reports say long reviews first came back as outlines and were read in line ranges.
- Not measured: whether any mechanism reduces the defect rate. No candidate has been tried.

## Raw data

The corpus and the per-shard JSON hold transcript text and stay in the session scratchpad, not in the repo. They can be rebuilt with the method in `README.md`.
