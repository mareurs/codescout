---
id: '8c17b1dbcf88a097'
kind: bug
status: open
title: 'Codex: counterexample miner omits cross-fold overlap among new candidates'
tags:
- cluster/guard-narrower-than-its-name
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
