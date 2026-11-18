---
id: cc7c5651c2c5d5b2
kind: research-note
status: archived
title: 'Historical: Population transfer: cheap acceptance does not establish equal quality'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-01-population-transfer.md
source_body_sha256: 74e9d3318ec5145f695af5f00e9affe62324601afc93b9d8ebb1d66450364426
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-01-population-transfer.md
  sha256: '8e86e4c459a2a6d197836e571b6039fef82762cb3486b061a644fdad8b034ac6'
  body_sha256: '74e9d3318ec5145f695af5f00e9affe62324601afc93b9d8ebb1d66450364426'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-01-population-transfer.md
  sha256: '8e86e4c459a2a6d197836e571b6039fef82762cb3486b061a644fdad8b034ac6'
  body_sha256: '74e9d3318ec5145f695af5f00e9affe62324601afc93b9d8ebb1d66450364426'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-01-population-transfer.md

---


# Population transfer: cheap acceptance does not establish equal quality

The fixed-checker low arm met the registered cost/acceptance criterion, but did not establish equal quality to xhigh. All three arms passed both tasks' required tests. Separate common diagnostics found a lock-order problem in ordinary-low Tokio and an uncorrected fallback extractor behavior in both low Axum repairs. Xhigh passed both diagnostics. These diagnostics are post-outcome development evidence, not retroactive changes to the registered score.

## Registered results

| Task | Ordinary low | Fixed-checker low | Fixed-checker xhigh |
|---|---|---|---|
| Tokio4867 |25/25; $0.1095759|25/25; $0.0855713|25/25; $0.2462071|
| Axum1934 |21/21 (20required+1added); $0.0453615|20/20; $0.0436595|20/20; $0.3306896|
| Whole client cost |$0.1549374|$0.1292308|$0.5768967|

Six configured calls, all started exactly once, valid pinned Sonnet5.5/CLI2.1.309 at assigned low/xhigh; no retry, replacement, extra repair or cross-arm hints. Order A/B/C/F/E/D. Each row has complete finalpatch/source-bound task-preserving review. Task-preserving means legitimate issue implementation/no bypass, not proof of all behavioral correctness.

Fixedlow is77.59897% cheaper than fixedxhigh over these two workflows. Registered candidate=true, mechanism acceptance contrast=false: every arm accepts2/2, so there is no acceptance-gain evidence. This is a finite diagnostic, not an equivalence/rate estimate. Exact raw decimal phase .86106490000000003; prior23.269336999999999800; cumulative24.130401899999999830 of40, remaining15.869598100000000170. Client estimates include each entire investigation/check/repair session. Actual billing, native orchestration/review and CPU cost are unknown.

## What the extra diagnostics show

| Common independent diagnostic | Base | Gold | Ordinary low | Fixed low | Fixed xhigh |
|---|---|---|---|---|---|
| Tokio capacity1 concurrent send/try_recv,500k sends |pass .294s|pass .328s|runtime timeout15.020s|pass .343s|pass .391s|
| Axum gold-added fallback has no MatchedPath |fail|pass|fail|fail|pass|

Tokio ordinary-low acquires tail under a held slot read lock; send2 acquires tail then slot write. The existing source comment explicitly warns of that cycle. Same publicAPI stress test on source-verified isolated copies compiles for all sources; ordinary-low exceeds15s, others finish under.4s. One run/source, no schedule instrumentation or thread-stack capture: bounded noncompletion plus source cycle supports the concern, not a deadlock-frequency or exhaustive correctness claim.

Axum both low variants replace a debug assertion with unconditional removal of stale MatchedPath. That addresses the reported nested-fallback panic, but the fallback router still sets MatchedPath. Xhigh adds a private fallback mode propagated through PathRouter and skips setting it there. A gold-added test outside required20 asserts no MatchedPath in a simple fallback. Same test bytes fail on base and both low candidates, pass on gold/xhigh. This is a pre-existing broader behavior left unfixed by low, not a newly introduced regression. Selected after source review of F/E and declared for all Axum sources before D outcome. No actor received these probe results. Neither diagnostic replaces original grades.

The result rejects promotion of the current checker paragraph as a demonstrated low≈xhigh mechanism. The useful next mechanism is a broader acceptance loop that detects missing invariants and triggers bounded repair/escalation. These now-exposed tasks can develop that loop; validation needs new prospective tasks and the entire workflow's client cost.

## Feedback and measurement

All4fixed sessions made2actual exact-command checker attempts, after excluding the precise unpaidB/Econtrol UUIDs. Initial failing listing+execution output is present in every primary transcript. C/F also received complete successful output; B/E piped success to tails. A also authored and observed a failing regression, so only-fixed-had-feedback is false. Tool-use counts, checker attempts and complete feedback delivery are separate measurements.

Actual nested listing/execution receipts bind current source, fixed fixtures, argv/oracle, process status and full test population. Required/full/additional are distinct; D's extra passing test is included in full21 without changing required20. Independent charge reader uses all6configured slots explicitly (its old default omittedF/E), calibrated on prior3raw receipts with ratio1 plus all6synthetic/missingFcharge cases. No failed/capped charge was omitted.

## Validation and limits

Real controls: Tokio base24/25→gold25/25twice; Axum base19/20→gold21/21twice (required20+onegold-added). All dataset names retained. Six identical-base setups and two actual fixed-shim failure controls verified before payment. Full project gate passed Python-light/fmt/wide-clippy/lean/default, with its printed heavyPython/server-stack/ONNX/librarian coverage limits. No Rust/public-runtime change in this phase. Component suite54pass2defaultDocker-skips; phase24tests pass.

One independent fresh runtime review found one real omitted Node import and consumer test gaps. Original18tests allowed six selected applied mutations to survive. Parent read exact surviving diffs and passing outputs before one corrective pass. Final24tests pass; five exact original mutants plus one explicitly adapted Node-callsite contraction are killed, zero invalid/selected survivors. Original M4 runtime impact was unestablished; it is not retrospectively called a proved runtime defect. Current literal-static-ESM reader closure includes usage.mjs+s4-rows.mjs; sealed old helpers unchanged. No second review or exhaustive coverage claim.

Both tasks are public, old and in reused Rust repo families; training exposure unknown. Tokio had prior gold controls, no prior actor outcomes. Axum was unused only under the recorded task-directory/selected-ID predicate, not a complete transcript census.38Rust-family metadata records inspected for selection; gold added-line counts were a proxy, not measured difficulty. No training/repository holdout claim. ActorF reported a broader library logging failure and tracing-feature rerun; parent does not claim independently calibrated full-library correctness from that report.

## Closure check correction

The first full-cohort postprocessing check failed because new closure glue assumed predecessor seal.json; the actual predecessor stores its seal in final-outcome.json. Original failure retained, actual predecessor interface read, exact path corrected, same full6reconciliation now passes with predecessor hash0150fdb... . No paid runtime or prior evidence changed. Issue657125148fb5c3e4 records this RED→GREEN. Publication and seal completion are verified separately after commit.

## Evidence and preservation

Q=/home/marius/work/claude/codescout/.codescout/resident-private/population-transfer-20261101. The local retained phase glue, prompts, raw receipts, transcripts, source patches, diagnostics and review copies are private evidence, not tracked source changes. Core evaluator is already committed at7cb2254acc228f2b47d227216b5339833170aa62 (patch-id72cc869f27226c61e6f238acc961d6664b72eb57).

Primary anchors: Q/selection.json, registration.json+sha, final-outcome.json, analysis/primary-feedback-all.json, accounting-population-calibration.json, fallback-results.json, concurrency/*/result.json, arm semantic-review.json, review/report.txt, replay-parent-audit.json, gate.log. Analysis/closure.py verifies registered bytes, all6raw/session/receipt charges and grades/source review, prior closed seal and committed documentation objects. Publication/seal metadata are create-once after the normal local documentation commit. No merge/push; worktree615e and ledgers retained.

Prior component seal0150fdbdcc43c5e7dd780b47fd957a01e3c3a1b62104fb7eeb364a3d2f9b6fa0 and prior paid seal43e17cd10ce2f7a9b648b2ba36611e5c72e49685406ffd08f58738827d91393d remain immutable; no retrospective repair of prior invalid xhigh result. New trackerbe811eb280fe974a owns LMP entries; old35c4c336bad905fb still resolves to worktree9369 even with workspace615e, so it was not edited.

## Rulings made, in order

1. Keep isolatedV and old sealed roots unchanged; wrong choice would invalidate provenance.
2. Treat public/family/prior-control exposure explicitly, without a holdout claim; otherwise transfer is overstated.
3. Predeclare all-pass as a ceiling; otherwise cheap acceptance is mislabeled acceptance gain.
4. Retain audit ledgers/worktree rather than skill cleanup; disk cost buys restart verification.
5. Replace both grade producer/reparser callbacks and preserve toolchain-invalid handling; otherwise false model grades.
6. Exclude unpaid shim controls from actor-use counts; otherwise feedback adherence is inflated.
7. One strongest fresh prepayment review and one parent fix pass; avoids paying before instrument verification, still leaves unselected blind spots.
8. Interpret fixed population as unchanged REQUIRED names, allowing gold/candidate full enumeration growth; otherwise valid added tests are rejected. Preserve initial failed control and recover only absent Axumrepeat.
9. Separate five exact replayed mutants from adaptedM4 and its unestablished original runtime effect; otherwise mutation evidence is false.
10. Parent owns reviewer-declined Docker/gate/accounting; exhaustive legacy correctness/exposure/toolchains/finalizer-main regression and billing remain unclaimed; otherwise inherited gaps become assurances.
11. Do not write U-born tracker through a V workspace parameter; otherwise wrong-worktree/sealed evidence could change.
12. Keep narrow registered endpoint/task-preservation unchanged after A/E source concerns, run common diagnostics on copies and report them separately; otherwise post-hoc threshold tuning or hidden quality defects.
13. Do not promote cost criterion to equal quality after Axum diagnostic failure; stronger acceptance/repair needs new prospective validation. No extra paid repair hidden in this cohort.

No deferred minor review findings. Detailed chronological rulings and exact verification output remain Q/progress.md.
