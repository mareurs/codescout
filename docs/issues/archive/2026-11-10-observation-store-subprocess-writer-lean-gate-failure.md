---
id: 255f71914a9174e4
kind: bug
status: fixed
title: Observation-store subprocess fixture unwraps bounded lock unavailability
tags:
- testing
- cluster/unclassified
closed: 2026-11-10
opened: 2026-11-10
owner: marius
severity: medium
unverified: 'CLEARED 2026-11-10. Was: underlying child panic unobserved and cause not established. A deterministic external quota lock now reproduces the exact consumer failure: subprocess_producer unwrap panics on unavailable, exit101. Initial whole-gate child output remains unavailable, so that individual panic is not retrospectively proved; the fixture''s unconditional-success assumption is independently confirmed.'
---

# BUG: observation-store subprocess writer unexpectedly exited101

## Summary

The subprocess fixture assumed every append succeeds, although the store intentionally returns unavailable after its100ms lock budget. A deterministic held-lock regression reproduced unwrap panic and child exit101. This was investigated after the resolver candidate's initial full gate failed at the expected deliberate exit9 assertion; unchanged-source retry failed in both lanes. The individual original child's panic was not captured, so that historical cause remains unknown even though the fixture defect is independently established.

## Class fit

The IC-1..IC-24 roster was read. Keep cluster/unclassified: the deterministic consumer-contract mismatch is established, but the initial whole-gate child's panic was not captured. This is a test fixture's unconditional-success assumption at a deliberately bounded lock API, not an observed production-store defect.

## Reproduction

Frozen source5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3 plus only resolver /// documentation changes. Full ./scripts/gate.sh --check with a slot lease in .worktrees/ecc-resolver-followup-20261110. [Gate log](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/gate.log), lines6878-6916; test tests/observation_store.rs:two_process_writers_and_abrupt_exit, parent assertion at486.

## Actual behavior

Lean run6383passed/1failed/49skipped; default9287passed/63skipped. Parent failure leftSome(101), rightSome(9), elapsed5.495s. The child command runs ignored subprocess_producer with stdout=null; panic details from the child's test harness are unavailable in this captured failure.

Unchanged-source full retry also failed the same parent assertion in both lanes: lean6383passed/1failed/49skipped, default9286passed/1failed/63skipped. Default was FAIL+LEAK and0.629s, so a5-second barrier diagnosis from the initial duration is specifically unsupported. [Retry log](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/gate-retry.log). The passing focused command used cargo test; a focused cargo nextest control is now separate.

## Root cause

Independently confirmed consumer assumption: subprocess_producer unwraps every append, but the store intentionally returns unavailable after a100ms lock budget. The held-quota-lock regression actually produced child panic at append.unwrap(), Err("unavailable"), status101; the parent observed1pass/1fail before the fixture fix. The sibling threaded-producer case c052a7c89e3b4a4d documents the same bounded store contract. The individual earlier gate child's panic was not captured, so do not retrospectively claim its exact internal cause. Production store behavior is unchanged.

## Evidence

[Resolver follow-up report](../../trackers/ecc-integration/reports/2026-11-10-resolver-followup.md). The historical84-test call-graph baseline and seven native cases are separate populations.

Exact focused lean cargo test passed1/0 in0.54s; focused nextest passed1/0 in0.517s (17 skipped). The diagnostic full lean run, changing only child output visibility/--nocapture, passed6384/6384. A proposed lock logging edit was refused and not applied. These controls did not reproduce the individual original panic.

The separately added held-lock control did reproduce unavailable => unwrap panic => exit101 before the fixture fix: [RED](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/fixture-red.log),1passed/1failed. After bounded fixture retry, [GREEN](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/fixture-green.log),19passed/1ignored.

## Fix

Fixture now retries only unavailable within a5s per-append budget; every other error remains fatal. Original16-record, two-producer contiguous sequences and abrupt-exit assertions remain unchanged. Production store code is byte-identical to base. Source `9aee4be48e446c15a441397f877c662f265f56ac`; integration on experiments `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`; stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`.

Final full source gate passed: lean6386/49skipped, default9289/63skipped, Python1827/7skipped/554subtests, fmt/clippy/doc-test lanes passed. Independent fixture baseline/restoration19passed/1ignored; all3 actual fixture mutations killed,0survived,0inconclusive. Exact integrated-tree full gate also passed on cb7d13ff360f8fb2464a8a2b52f0f87342c00391/tree d563673b185d884dbbfe215d39cad286d32dada0: lean6387/49skipped, default9290/63skipped, Python1827/7skipped/554subtests; fmt/clippy/doc-test lanes passed with printed coverage exclusions.

## Fix provenance

- **SHA:** `cb7d13ff360f8fb2464a8a2b52f0f87342c00391` (`experiments`)
- **patch-id:** `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`

Source worktree commit `9aee4be48e446c15a441397f877c662f265f56ac` has the same stable patch-id and owned file hashes. Source and exact integrated-tree full gates passed; [identity record](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/integration.json).

## Tests added

subprocess_fixture_retries_bounded_lock_unavailability holds the real quota lock until a retry is observed, releases it, then requires all8 contiguous stored records and exit9. subprocess_fixture_does_not_retry_disabled_store requires disabled to remain fatal, no retry marker and no stored records. The existing two-process16-record test remains unchanged.

## Resume — 2026-11-10

Closed2026-11-10 after passing source and exact integrated-tree gates, independent review and observed mutation checks. Fix integrated on experiments as `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`, stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. [Report](../../trackers/ecc-integration/reports/2026-11-10-resolver-followup.md) retains raw failures, controls and provenance. BFS identity correction remains separate. The deterministic fixture assumption was corrected; the exact individual earlier gate panic remains unobserved. Production store behavior is unchanged.
