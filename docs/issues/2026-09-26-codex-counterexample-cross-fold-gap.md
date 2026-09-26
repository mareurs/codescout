---
id: '8c17b1dbcf88a097'
kind: bug
status: fixed
title: 'Codex: counterexample miner omits cross-fold overlap among new candidates'
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-26
opened: 2026-09-26
owner: marius
severity: medium
---

# Counterexample miner checks new versus frozen data, not new versus new

**Valid:** dated 2026-09-26

Observed at `c69a99e4`, in the Stage 2 miner introduced by `ec12b2a1`. Stage 2 remains unregistered; no candidate draw or training contamination is claimed to have occurred.

## Mechanism

`docs/evals/data/2026-09-24-rule-tell/phase1b/mine_counterexamples.py:89–98` constructs by_fold only from frozen train/val/cal. Lines 106–120 compare each candidate paragraph with that old corpus. No check compares newly eligible or selected candidates with new candidates in other folds. Retaining manifest source groups prevents source-group splitting but does not prevent repeated eight-word content across different documents.

## Observed evidence

Ran the actual main in --count-only mode and captured its eligible pool at function return with a Python profile hook. No sampling output or repository data was written. Counts reproduce the preregistration exactly, including 20 held-out-filter drops. Codex clean texts are not yet present, as the command reports.

Across the eligible pool, 399 distinct manifest paragraphs include 64 distinct eight-word shingles shared across folds, affecting 28 paragraphs. Those are content-overlap counts, not independent incidents or demonstrated performance inflation.

A separate in-memory application of the miner's exact RNG and caps to this current pool yields a **hypothetical pre-Codex-filter draw** of 197 candidates with 5 cross-fold shared shingles, touching 11 candidates. Example shared shingle: `cargo fmt cargo clippy d warnings cargo test`, present in train candidate `cx-d_semicolon-7804-0`, val candidate `cx-d_semicolon-7939-4` and cal candidate `cx-d_semicolon-7797-0`. The future Codex-text filter may change this draw; it does not add the missing new-versus-new invariant.

## Test coverage probe

In memory only, replaced the miner's shingles function by one returning an empty set, disabling all its shingle guards. The current tests/test_phase1b_mining.py ran 8 tests, 0 failures, 0 errors. One candidate mutation applied, one survived. Tests presently cover cue selection and clean-text/document consistency, not the filtering pipeline.

## Expected correction

Before emitting candidates or final NC splits, check the augmented datasets against one another, covering both new-versus-frozen and new-versus-new overlap. Apply a preregistered deterministic drop policy while retaining source groups. Add fixtures where two otherwise-admissible new candidates in different folds share a shingle, plus a non-colliding control. No fix applied; cluster classification pending.

Related review: docs/research/2026-09-26-codex-phase1b-stage1-review.md.

## Fix

**Fixed in `96b52f0c`, patch-id `ee65c6ff9f755e52e96d05b51b2db8efada7aef8`,** before any candidate was drawn. Stage 2 is still unregistered.

- **Filter 4, `resolve_new_vs_new`.** Among candidates that passed filters 1–3, one that shares an 8-token shingle with a kept candidate in an earlier fold is dropped. The fold priority is val, then cal, then train, so train text is in neither val nor cal, and cal text is not in val. A dropped candidate never blocks a later one. A drop is never a move.
- **The final check, `check_no_cross_fold_overlap`.** The draw is refused before anything is written if a drawn candidate shares a shingle with a frozen row, or with another drawn candidate, in a different fold.
- **Count-only reproduction, before registration.** The eligible pool held 64 distinct shingles shared across folds, in 28 distinct texts, and 399 manifest paragraphs (398 distinct texts). That matches this report. Filter 4 dropped 23 candidates.

## Tests added

`tests/test_phase1b_mining.py` now has 22 tests, 14 of them new:
- **Filters 1–3:** each case gives one guard alone a reason to refuse, with a same-fold or other-rule control.
- **Filter 4:** train against val, cal against val, train against cal, collisions within one fold, a non-colliding control, and a case showing that only kept candidates block later folds.
- **The final check:** cross-fold refused, frozen other-fold refused, within-fold allowed.
- **The overlap counter.**

Nine mutations were run, each in an isolated worktree through `scripts/mutation-probe.sh`, and every one failed the suite. Clean baseline: 22 OK.

| mutation | failures |
|---|---|
| `shingles` returns an empty set (this report's probe) | 10 |
| filter 1 off | 1 |
| filter 2 off | 1 |
| filter 2 applied to the same fold as well | 1 |
| filter 3 off | 1 |
| filter 4 off | 4 |
| fold priority reversed | 3 |
| final check off | 2 |
| final check ignoring frozen rows | 1 |
