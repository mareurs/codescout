---
id: '51baab12f451feb6'
kind: tracker
status: active
title: Architecture Boundary Measurement — Session Log
tags:
- architecture
- measurement
- session-log
topic: architecture-boundary-measurement
entry_prefix:
- F
- W
entry_high_water_F: 1
entry_high_water_W: 1
---

# Architecture Boundary Measurement — Session Log

## Purpose

Frictions and counterfactual wins while validating the architecture instrument. Baseline, decisions and resume state belong to [Architecture Boundary Measurement](architecture-boundary-measurement.md). This is a prose ledger; the librarian allocates IDs. Status vocabulary follows docs/templates/session-log.md.

## F-1 — Raw import inspection falsified a green probe's static population

**Valid:** dated 2026-09-13

**Observed:** 2026-09-13, after the 12-test suite and self-test passed, the first raw reference in baseline-actions.json was crate::ast::anyhow::Result. Calling resolve_relative on anyhow::Result reproduced it. A second production call reduced crate::lsp::base to crate::lsp::b.

**Expected (plan):** A passed fixture suite would permit collecting a bounded static baseline, followed by raw-reference calibration.

**Got (scouted reality):** Fixtures covered cycles, whitespace and registration controls but not external-root identity or the letters as inside identifiers. Two new tests observed two failures before their fixes; all 14 passed afterward. At the same frozen HEAD, the corrected raw edge list has 315 rows against 379 previously; the earlier figure is rejected evidence, not a before/after architecture improvement.

**Severity:** high — invented edges would have entered a durable architectural baseline despite green controls.

**Status:** promoted-to-bug-tracker

**Rests on:** docs/issues/2026-09-13-architecture-probe-invents-internal-imports.md and docs/issues/2026-09-13-architecture-probe-mangles-as-identifiers.md; scripts/architecture-boundary-probe.py; tests/test_architecture_boundary_probe.py. The architecture tracker owns baseline bounds and verification status.

**Workaround:** Validate raw reference identity on both external and internal roots before publishing totals. Keep lexical results explicitly bounded; do not claim compiler resolution.

## W-1 — Applied mutations established which probe regressions the tests actually catch

**Valid:** dated 2026-09-13

**Observed:** 2026-09-13, four isolated copies of the production probe were mutated separately: restrict cycles to length two; disable test-field masking; erase shared-prefix action fields; remove defaults from the deployed feature lane.

**Pattern:** Apply the candidate mutation, run the real test suite, then inspect the named failure rather than treating a nonzero exit alone as a kill.

**Counterfactual:** The longer-cycle mutation returned to the original faulty behavior; its specific regression failed. The other copies each failed in the corresponding field-mask, action-prefix or deployed-feature test. No source in the shared checkout was mutated for this check.

**Confirming data points:** Four candidates applied, four detected, zero surviving; each run executed 12 tests. Logs: .codescout/measurements/architecture-boundary/2026-09-13/mutation-cycle.log, mutation-field.log, mutation-action.log and mutation-deployed.log. These are bounded site checks, not a mutation score over every probe function.

**Status:** validated

**Rests on:** the retained mutation logs and isolated copies under /tmp/codescout-boundary-mutations-M3Ro6F/. Import normalization was checked separately through observed red/green regressions, not included in this mutation count.

**Promote-when:** No new promotion proposed; this applies the existing Testing Discipline law.

## Entries

Append new observations through doc append_entry. Do not treat confirming results as independent catches.
