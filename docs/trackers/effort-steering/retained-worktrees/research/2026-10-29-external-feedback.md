---
id: '72865f18786bb53b'
kind: research-note
status: archived
title: 'Historical: 2026-10-29-external-feedback'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-29-external-feedback.md
source_body_sha256: b764fad0157f10145d06ce0e5ba6401cd7ab4b4a26924ed96d81fc515a814099
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-29-external-feedback.md
  sha256: b764fad0157f10145d06ce0e5ba6401cd7ab4b4a26924ed96d81fc515a814099
  body_sha256: b764fad0157f10145d06ce0e5ba6401cd7ab4b4a26924ed96d81fc515a814099
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-29-external-feedback.md
  sha256: b764fad0157f10145d06ce0e5ba6401cd7ab4b4a26924ed96d81fc515a814099
  body_sha256: b764fad0157f10145d06ce0e5ba6401cd7ab4b4a26924ed96d81fc515a814099
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-29-external-feedback.md

---

# External behavioral-feedback calibration — 2026-10-29

The two registered low calls pass their finite upstream acceptance targets at the first Stop. Neither receives feedback, so this phase does not establish the mechanism's correction effect. The fixed stopping rule therefore leaves the disjoint axum holdout and fresh xhigh comparison unstarted. A separate input probe finds a real crash in both the low bat candidate and upstream gold, showing why upstream acceptance alone cannot certify robust behavior.

## Registered outcomes

| Call | Task | First/final F2P | First/final compatibility | Delivered feedback | Client USD |
|---|---|---|---|---|---|
| C1 low | ripgrep-2576 | 1/1 both | 86/86 both | 0 | 0.1600911 |
| C2 low | bat-1892 | 1/1 both | 80/80 both | 0 | 0.20421690000000003 |

Both calls are valid claude-sonnet-5-5 low invocations on privately pinned CLI2.1.309. Each has one successful Stop and identical first/final patch hashes. Parent grading uses the saved exact patches in fresh containers with upstream tests restored. Complete diffs were reviewed for task preservation and bypass; each candidate adds regressions without removing assertions. C1 conservatively falls back to the existing capture-based matcher; its performance impact is unmeasured. C2 adds custom suffix plumbing to existing syntax detection.

New client estimate:0.36430800000000003. Cumulative:16.105745499999999865 of the approved20, leaving3.894254500000000135. These are CLI client estimates, not billed totals; native orchestration/review dollars remain unknown. There is no new xhigh cost or latency comparison.

Decision: calibration_ceiling_no_holdout. These are two calibration tasks from public historical repositories, not two independent holdout successes. Public benchmark exposure in model training is unknown. The preregistered axum-691 holdout identity was fixed before outcomes, but no holdout model call was made. No calibration failure or paid call was dropped, replaced or rerun.

## Independent qualification of green results

C1 ran cargo test --all and observed exit101 because the offline image lacks cc v1.0.79. The unchanged initial source in a separate container reproduces the same missing-dependency error. This wider optional command has no completed behavioral population; do not report full ripgrep suite success. Its fixed upstream regression-family target remains86/86.

C2's own cargo test returned0. A post-hoc boundary probe, motivated by the complete diff, then crossed candidate/upstream-gold with nonempty/empty --ignored-suffix input. Both nonempty controls exit0. Both empty inputs exit134 with stack overflow. This uncovered defect is shared by gold; it is neither evidence that low is worse than xhigh nor an official benchmark failure. The primary acceptance verdict is preserved, and the original paid candidates are unchanged. [Reproduction and open issue](../issues/2026-10-29-bat-empty-suffix-aborts-candidate-and-gold.md).

The useful next calibration input is this observed natural failure, with a separately frozen boundary oracle and repair protocol. Repeating upstream-only ceiling tasks or silently retrofitting the original acceptance target would not measure a correction effect.

## Evaluator controls and amendments

The public dataset SHA256 is eb0674a958e91763a2aeb1f9b481efeea7ffce43b6205077d89465a1ebc1188a. Original record, gold, upstream test patch and immutable official image ID are retained for each fixed task. Independent full-target base/gold controls are ripgrep85pass/1fail→86pass/0fail; amended bat79/1→80/0; axum123/2→125/0. Exact one-function F2P commands independently reproduce base failure and gold success.

The original bat full target aborts on base and gold. A serial control localizes duplicate ownership of one PTY descriptor. The one-line stdout descriptor-duplication amendment preserves every assertion and all80 functions and is applied identically to bat initial sources and parent evaluators. Original invalid controls remain retained. [Fixture issue](../issues/2026-10-29-bat-external-fixture-owns-pty-twice.md).

Axum's image has an ignored Cargo.lock absent from its Git archive. It is frozen as an explicit source/dependency input in both author and parent environments. Source fidelity initially rejected this difference and rejected build-cache files inside the source inventory. The adapter now keeps image build state outside source and verifies the complete source inventory, including ignored additions, deletions, symlinks and executable modes.

Real unpaid Stop preflight observes block/allow/allow with complete output and an exact changed-source roundtrip. Containers are newly created, network disabled and resource bounded; original repository history is replaced with one initial commit. Other-study model outputs, runners, mutable containers and credential files are not accessed; the existing effort-study token is reused under the user's explicit authorization. These are amended diagnostic fixtures, not an official SWE-bench score. [SWE-bench evaluation guide](https://www.swebench.com/SWE-bench/guides/evaluation/), [Multilingual construction and task scope](https://www.swebench.com/multilingual.html).

## Review and verification

A fresh design review corrected outcome-dependent holdout execution: valid G failure must not suppress X. Independent code review found attempted feedback could be mislabeled inactive and admit another calibration; the direct regression failed before the fix and passes afterward. [Admission issue](../issues/2026-10-29-feedback-attempt-is-not-inactive-calibration.md).

The reviewer actually applied8guard mutations:2killed,6survived,0invalid on the initial9tests. Direct session/state/acceptance regressions, a synthetic receipt with real local inventory and a real Docker mismatch test close the holes. Reapplying those exact8mutants to the17-test suite gives8killed,0survived,0invalid, with assertion failures and complete raw logs. This measures eight selected guards, not complete test coverage. The default standalone suite explicitly skips the Docker test unless the pinned fixture is supplied; the recorded mutation run includes it. Final standalone verification passes10public adapter tests including Docker and11private controller/closure tests. The closure additionally recomputes budget metadata from both raw receipts and checks the complete retained evidence set. Two separate applied closure-artifact controls also fail as expected when the complete-file-set or evidence-byte checks are removed; those are distinct from the eight code-guard mutants.

The final complete project gate passes: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0 and DEFAULT=0. The first gate attempt retains three named fixture-precondition failures because/tmp has an existing.git marker, whose measured mtime predates this phase. Git itself does not recognize that marker as a valid repository; the tests reject the marker's presence. Shared/tmp/.git is unchanged. The invalid attempt was interrupted after those failures, retaining its log and exact interruption record. The next complete run, using a long isolated/var/tmp root, exits1: Python, formatting and clippy pass, but the same six LSP socket fixtures fail in each Rust lane with explicit SUN_LEN/AF_UNIX path-too-long errors. This repeats the already documented temporary-path limit. A shorter isolated root is then checked for.git/.jj markers over its entire ancestor chain and successfully binds a101-byte nested Unix socket. The complete lsp::manager::tests:: regression filter passes under that root before a full gate rerun starts. The short-root run then exposes eight missing opened fields in already committed issue reports. Independent inspection confirms the omissions in parent36f3c3e3. Only those metadata fields are repaired through the catalog; all eight bodies/statuses/caveats are unchanged, and the preceding sealed583-file/20-Git-object closure still verifies. The complete existing18-function frontmatter regression passes against the staged publication after repair. [Publication metadata issue](../issues/2026-10-29-prior-issue-reports-missing-opened.md). The final complete gate runs after staging all intended files and exits0. All failed attempts remain retained. As printed by the gate, this does not cover the heavy Python suites, server-stack, librarian code in LEAN or the two local ONNX weight tests; librarian tests run in DEFAULT.

## Controller rulings and limits

Retain the private plan/evidence workspace after review so receipts and source artifacts remain auditable; the cost is local disk usage. Reassess bat eligibility using the minimal fixture amendment before registration; the risk is dependence on an amended oracle, disclosed above. No task identity was substituted.

One low call per calibration task, no delivered feedback, no fresh xhigh arm and no holdout cannot establish general low/xhigh parity, cost savings or latency superiority. The additional empty-suffix probe is development evidence, kept separate from the fixed primary population. No production integration, merge, push, global configuration change or upstream patch is claimed.

## Evidence locations

[Fixed design](../superpowers/specs/2026-10-29-external-feedback-design.md), [execution plan](../superpowers/plans/2026-10-29-external-feedback.md), [adapter and verification commands](../../tools/reliable-agent-eval/external_feedback/README.md), and tracker LEM-15. The local private root is .codescout/resident-private/external-feedback-20261029: registration.json and its hash; analysis.json; all raw receipts/transcripts, exact patches/inventories, Stop and Cargo records, base/gold controls, original and closed mutation runs, source reviews, post-hoc probes, gate logs, and the closure budget anchor. The preceding sealed anchor is behavior-transfer-20261029/final-outcome.json SHA256cc8e9598540095a68d9677fb521084474dab8c3bef1e619c1905afa8d9ea52ee.
