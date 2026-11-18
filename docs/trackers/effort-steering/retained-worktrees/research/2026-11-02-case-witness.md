---
id: '2e35ead0b88e34e6'
kind: research-note
status: archived
title: 'Historical: Offline case-selection witness diagnostic'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-02-case-witness.md
source_body_sha256: 4d70b95d177ed60fb41de3f6f83f250775165ea337003d03bd785deb44e1d244
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-02-case-witness.md
  sha256: f091ef1e5e907b4997dac09e14c49f5e3b60357dc3e0fab7864077aa1da1dfa2
  body_sha256: '4d70b95d177ed60fb41de3f6f83f250775165ea337003d03bd785deb44e1d244'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-02-case-witness.md
  sha256: f091ef1e5e907b4997dac09e14c49f5e3b60357dc3e0fab7864077aa1da1dfa2
  body_sha256: '4d70b95d177ed60fb41de3f6f83f250775165ea337003d03bd785deb44e1d244'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-02-case-witness.md

---


# Offline case-selection witness diagnostic

## Scope and method

**Valid:** dated 2026-11-02

This is a throwaway offline diagnostic on the already exposed Axum 1934 case. It compares three exact recorded public-API tests: the generic challenger, the contract-to-assertion challenger, and the known direct-fallback control. It does not generate another model test or claim blind discovery.

Each probe runs on the preserved low candidate, fixed xhigh reference and gold, first without instrumentation and then with temporary logging in source copies. The pinned image, exact test bytes, Cargo command, one-test population and role identity must agree within each pair. The unchanged fixed evaluator independently lists and executes the fixture and checks source binding. Raw ordered traces retain opaque Rust Debug values. Logging labels identify selected insertion sites; they are not full branch or contract coverage.

Private evidence root: /home/marius/.cache/codescout-research/codescout/case-witness-20261102. The standalone runner and verifier use the old read-only compatibility namespace; no old experiment bytes are intentionally changed. Disposable clones are /tmp/cw02-{candidate,reference,gold}; source inspection uses separate captured copies.

## Results

All 18 source/probe/variant evaluations completed and were independently revalidated. Each enumerated and executed exactly one test, with no ignored tests. All nine quiet/traced pairs retained outcome, fixture identity, image and command.

| Exact recorded probe | Low candidate | Fixed xhigh reference | Gold | Candidate distinct selected event labels |
|---|---|---|---|---:|
| Generic challenger | Pass | Pass | Pass | 9 |
| Contract-to-assertion challenger | Pass | Pass | Pass | 8 |
| Known direct-fallback control | Fail | Pass | Pass | 8 |

Each cell has the same verdict in quiet and traced variants. These are replays, not independent model samples. The right column counts unique emitted labels from ten declared insertion sites, not covered branches. Every candidate event label visited by the direct control is also visited by the generic challenger; only the generic additionally visits matched.tail.

Both generated probes log the internal fallback route present in the candidate extension, then enter a nested service route and log matched.tail followed by present=None. The generic extractor reads None. The structured service reads the extension directly and asserts a body ending in false; the unchanged passing fixture binds that observation. Reference/gold skip the erroneous fallback update and also reach the empty extension at this observation.

The direct control reads Some("/*__private__axum_fallback") at the candidate extractor and fails its assertion that the option is absent. Reference/gold reach the extractor with None and pass. Ordered logs distinguish a transient wrong state that is cleared from one exposed at the assertion boundary.

## Interpretation and next gate

The missed defect is not explained by failing to execute the MatchedPath update code: the generated probes execute it. A score based on the number of selected visited sites would prefer or tie the probes that miss the defect here. This is a concrete counterexample to using that score alone as a test-selection criterion.

Develop offline a bounded public-API case matrix tied to observation boundaries: identify the contract, enumerate distinct direct and composed API paths, observe the relevant state after those paths, then test assertion sensitivity. Keep topology selection separate from assertion mutation. These traces diagnose one exposed case; they do not yet supply a general automatic selector.

Before another paid comparison, freeze a reusable selection procedure and attempt limits; test boundary selection on further development cases without a case-specific answer; then freeze and run outcome-unseen matched ordinary-low/structured-low/xhigh workflows with common independent acceptance and whole-workflow costs. Do not pay for another wording-only retry on Axum1934.

## Validation and limitations

Six focused instrument tests passed after an observed red baseline. Initial preparation failures and their logs are retained. One failure was a wrong helper-module assumption before evaluator execution; another test initially assumed the candidate's removal syntax also applied to reference and gold. The instrument preserves each role's original semantics.

The prior contract-to-assertion closure was revalidated successfully, including its nested predecessor closures. No model call is scheduled by this spike. Model-client spend remains 24.269845199999999810 of the authorized 40; CPU and native orchestration dollars are not measured. This case and its known control are exposed development material; no general quality parity or external validation is established.

## Repository verification

The fresh full project gate exited 1: FMT=0, CLIPPY=0, PYTHON_LIGHT=0, DEFAULT=0, LEAN=101. Its sole reported lean failure was the unchanged test a_timed_out_filter_with_output_over_the_limit_stays_inline: after its one-second producer timeout, stdout_total was 190245 rather than the asserted 200000. An exact isolated rerun through scripts/with-slot.sh passed. This is a recorded verification limitation; the full gate is not reported green. This is the already recorded [timeout-test bug](/home/marius/work/claude/codescout/docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md), fixed elsewhere by 1910afb125440aebb993f8170ae99532b4533bd7 (patch-id e8912e70e20ceaf872eb8872457a7e48014efb34). The retained research tree still contains the old exact-count assertion. No project runtime/test code was changed to suppress it.

The spike's six focused tests and all 18 source-bound evaluations passed their own validity checks. Both quiet and traced direct-control failures are expected behavioral detection, not invalid evaluation. The verifier rechecks every expected matrix member, stored grade, source/fixture binding, log hash, fixture hash, event sequence and paired outcome. Raw matrix, unit failures, setup failure and project-gate logs are retained.
