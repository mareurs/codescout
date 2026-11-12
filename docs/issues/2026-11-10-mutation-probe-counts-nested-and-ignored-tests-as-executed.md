---
id: e4b663a4f2145663
kind: bug
status: mitigated
title: mutation-probe labels selected parent and nested test counts as tests that ran
tags:
- testing
- cluster/unclassified
closed: 2026-11-12
opened: 2026-11-10
owner: marius
severity: low
---

# BUG: mutation-probe overstates the executed top-level test count

## Summary

The probe reports21 test(s) ran for the observed fixture mutation, while the final top-level cargo summary is17passed,2failed,1ignored. The count is a sum of every visible running N tests line, including a nested child harness and an ignored top-level selection. It is not a count of completed top-level tests.

## Class fit

The IC-1..IC-24 roster was read during this work stream. Keep cluster/unclassified rather than force a fit: this is a measurement-population mismatch in a shell instrument, with selected parent and nested stdout counted together. It differs from the archived non-cargo verdict issue, where no count line existed at all.

## Reproduction

The actual strict mutation command and environment are retained in [fixture review execution](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/review/fixture-review-execution.json), first mutation disable-unavailable-retry. [Actual raw log](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/review/fixture-mutation-disable-unavailable-retry.log): final top-level summary at76, helper verdict at79. It contains a captured child harness failure as well as the parent summary.

## Environment

ECC resolver follow-up, base5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3 plus frozen two-file candidate; tests/observation_store.rs. Inner source-bound slot1 lease. The script itself is unchanged by the candidate.

## Root cause

Inspected scripts/mutation-probe.sh:366-367: ran_lines counts all anchored running N tests lines; executed sums their N values. The final verdict interpolates that sum as test(s) ran. The parser neither selects the outer harness nor subtracts ignored cases. The observed21 is therefore a parse of selected stdout counts, not the final top-level executed population. No false mutation verdict is established by this finding.

## Evidence

Independent reviewer explicitly used the last top-level cargo summary rather than the helper's count. The actual mutation was killed by observed held-lock and original two-writer assertions, so the numeric defect does not invalidate this cycle's kill verdict. This report does not claim a universal executed count across hidden child harnesses.

## Instrument snapshot

[Post-observation source and count binding](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/instrument-count-boundary.json) verifies the unchanged script bytes against base, parses the actual visible selections[20,1], and records the last top-level17passed/2failed/1ignored summary. The source snapshot is post-observation material, not a frozen pre-action repair input.

## Fix

The label now says what the number is. The verdict reads `N test(s) selected over M 'running' line(s)` and one more line says the sum includes ignored and nested-harness tests and points to the runner's last `test result:` line for the top-level figure. The parser and the verdict guards are unchanged: zero versus non-zero still decides INCONCLUSIVE. A top-level-only count is not built, because the script cannot select the top-level harness from stdout it does not control.

Case 10b of `tests/mutation-probe.sh` pins the sum (lines `2` and `1` read as `3 over 2`) and the absence of the old wording. Cases 10 and 11 follow the new label.

Unverified: no real `cargo test` run with a nested child harness was made against the new label; case 10b feeds the parser fake runner output.

## Fix provenance

- **SHA:** `b8e79177ea7cdaad82ff2286365e06246980fa1a` (`experiments`)
- **patch-id:** `7e45b40a57d5182579099814a50f199268820fe9`

## Tests added

Case 10b in `tests/mutation-probe.sh` (new). Cases 10 and 11 were reworded to the new label. The raw fixture mutation log stays as a regression seed.

## Workarounds

Report the final top-level runner summary and exact failure assertions separately. Do not cite the helper's21 as a completed-test count.

## Resume — 2026-11-10

Mitigated on 2026-11-12 by `b8e79177e`: the label no longer claims the selected sum is a count of tests that ran. The independently inspected three fixture mutation kills are unaffected.
