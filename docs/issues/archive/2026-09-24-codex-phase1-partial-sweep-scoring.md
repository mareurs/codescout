---
id: ad8aa199f2f5cfa5
kind: bug
status: archived
title: 'Codex review: phase-1 Score A accepts incomplete rule sweeps as clean texts'
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-24
owner: marius
severity: medium
---

# Codex review: phase-1 Score A accepts incomplete rule sweeps as clean texts

**Valid:** dated 2026-09-24

## Observed

Reviewed phase-1 selector through commit `3011b9b8`. `scripts/phase1-span-selector.py` function `report_corpus` groups supplied rows by case/side and excludes explicit errors, but never validates that each group contains every registered rule exactly once. The `--report` path accepts arbitrary saved JSONL. An interrupted or truncated export can therefore report clean negatives with missing decisions.

## Reproduction and result

Offline probe executed the actual AST-extracted `fired` and `report_corpus` functions, without imports that could invoke a model. Passed one row: `{"case":"synthetic-clean","side":"negative","text_detectable":"yes","rule":"one-of-22","verdict":"NO","gold":[]}`.

Observed: `1 texts, 1 rows, 0 errored`; negative any-fire `0/1`; fires/text `0.00`; excluded `0`; function returned `0`. Twenty-one rule decisions are absent. This is a synthetic reproduction of the scoring defect, not evidence that any published run is incomplete.

## Expected

Validate registered case/side/rule membership and uniqueness before reporting complete-corpus metrics. Distinguish missing rows and missing texts from explicit errors and real NO decisions. Refuse incomplete inputs or explicitly label partial results and exclude incomplete sweeps.

## Scope

No production or peer files changed; no model calls, full Rust gate, or mutation suite run. This report files the observed scorer defect only. Semantic correctness of the copied claim span is a separate evaluation gap, not established by this reproduction.

## Fix

**Fixed in `0fef5562`** (patch-id `0668562a7669dcf3dac496c0d9137a5519328f0e`), the commit after the `3011b9b8` this review read. `report_corpus` now builds the expected set as `collections.Counter(RULES.keys())` for every (case, side). A group missing or duplicating any rule is printed as `INCOMPLETE … — EXCLUDED`, is counted in the `excluded` column, and makes the report **exit 2**. A first draft used `Counter(RULES)`, which counts the dict's *values*; a complete-sweep control caught it before commit.

**Verified 2026-09-24 by re-running this file's own reproduction through the shipped `--report` path.** The same one-row negative now reports `1 incomplete`, `missing [act_on_artifact, cannot_happen, closed_population, contradiction …] — EXCLUDED`, excluded `1`, **exit 2**. Before the fix it reported negative any-fire `0/1` and exit 0.

**Class retagged** from `cluster/unclassified` to `cluster/capped-result-presented-as-complete` (`IC-13`). A partial sweep scored as a whole-corpus metric is a truncated result presented as complete.

**Regression test, added 2026-09-24 in `138bdb60`:** `tests/test_phase1_span_selector_report.py` case `test_one_rule_missing_from_a_present_text_is_refused_by_name` pins the exit-2 refusal and the `INCOMPLETE` line on the committed S0 sweep. Observed red: removing `incomplete` from the exit condition, in an isolated worktree through `scripts/mutation-probe.sh`, reds that test and no other. The same commit closed a sibling hole this check could not see, whole texts missing (`docs/issues/archive/2026-09-24-codex-phase1-missing-case-groups.md`). This script is not in the Rust gate, so the Python test is its regression guard.
