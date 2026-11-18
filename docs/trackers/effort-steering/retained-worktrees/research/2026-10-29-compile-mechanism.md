---
id: '8e492cb6e6aa0ddb'
kind: research-note
status: archived
title: 'Historical: 2026-10-29-compile-mechanism'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-29-compile-mechanism.md
source_body_sha256: 7eaf1e0aaee3c47eb0d0c4d7c76eacd4d0bfc6c374e7aa54f7e673863a7c65ff
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-29-compile-mechanism.md
  sha256: '7eaf1e0aaee3c47eb0d0c4d7c76eacd4d0bfc6c374e7aa54f7e673863a7c65ff'
  body_sha256: '7eaf1e0aaee3c47eb0d0c4d7c76eacd4d0bfc6c374e7aa54f7e673863a7c65ff'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-29-compile-mechanism.md
  sha256: '7eaf1e0aaee3c47eb0d0c4d7c76eacd4d0bfc6c374e7aa54f7e673863a7c65ff'
  body_sha256: '7eaf1e0aaee3c47eb0d0c4d7c76eacd4d0bfc6c374e7aa54f7e673863a7c65ff'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-29-compile-mechanism.md

---

# Precise compiler instruction versus enforced compiler feedback

## Result

All eight revised conversations completed validly. Independent parent acceptance was ordinary low1/2, precise-instruction low2/2, enforced-feedback low2/2 and fixed-xhigh2/2. G produced one observed low-effort compiler-feedback repair; its other run needed no intervention. Both I runs repaired compiler errors before first Stop. This identifies a working compiler-class feedback mechanism in these traces, with two observations per arm on one known task—not a general equivalence or reliability result.

Mean model client estimates were approximately I$0.3801, G$0.3685 and X$1.2503 per call. Eight revised calls cost$4.6732616; cumulative client estimates are$15.2172051 of$20, including the preserved invalid first-protocol charge. Native orchestration/review dollars are unknown. One disclosed protocol restart and no revised paid retries; every quality failure and charge is retained.

## Comparison and fixed procedure

The [original design](../superpowers/specs/2026-10-29-compile-mechanism-design.md), [plan](../superpowers/plans/2026-10-29-compile-mechanism.md) and [disclosed protocol revision](../superpowers/specs/2026-10-29-compile-mechanism-revision.md) define a bounded mechanism diagnostic on the known cs-p20 reindex-progress task. This is one historical task, not external transfer validation.

| Arm | Effort | Added intervention |
|---|---|---|
| L | low | Ordinary task prompt |
| I | low | Explicit instruction to run and fix cargo check --offline --all-targets |
| G | low | One enforced Stop block returning a failing all-target compiler diagnostic |
| X | xhigh | Ordinary task prompt |

All arms receive the same source, behavior-description prompt, protected TASK.md, tools and silent Stop compiler observer. Only I receives the command-specific suffix. Only G may receive one failed-check feedback block; it remains at low effort. Controls never receive observer feedback. Thus an inactive G is evidence of available checking, not observed automatic correction.

The fixed prospective order is 1-G, 1-I, 1-L, 1-X, 2-G, 2-L, 2-I, 2-X. Each call uses a fresh profile/session and a fresh one-commit source history. The original historical archive already contains the evaluator and its graft; preparation strips those exact bytes before initializing author Git history. The parent alone restores the frozen evaluator for grading.

Claude CLI 2.1.309 and claude-sonnet-5-5 are pinned. Actual model and effort come from raw results and request transcripts. The source fixture contains 2107 retained historical files plus TASK.md. Runtime uses four available CPUs, two Cargo jobs, the historical offline Cargo home and leased shared caches. Each author has 1200 seconds and a soft client allowance L $1 / I $1 / G $1.50 / X $2. Before a block, admission reserves its complete pending allowances plus $1 against the cumulative $20 cap. Invalid, capped, unknown-charge or timed-out calls stop the revised campaign without a paid replacement. Valid quality failures remain outcomes.

The same tool whitelist permits source edits, cargo test/check, git diff/status and rustfmt. Other Git commands are denied. These restrictions and shared caches affect behavior and latency; this is setup isolation on a shared host, not adversarial OS isolation.

## Independent acceptance and measurement

Root reads each complete saved patch and records a review bound to its SHA-256: existing assertions, protected requirements, dependencies and existing public signatures must remain intact. This review checks constraints rather than proving all behavior. Parent grading reconstructs that exact patch in another fresh base, verifies its complete source inventory, runs default-feature all-target compilation, injects the frozen evaluator and compiles all targets again, then parses the complete literal population of two new behavioral tests and seventeen preserved tests. Ordinary compiler exit101 is a quality failure with zero behavioral tests executed.

Offline calibration of the revised fixture observed base two failures plus seventeen passes, and golden reference nineteen passes. The actual Cargo launcher/Stop path was tested without a model on successful source, intentional compiler failure and restored source: exits0→101→0 and decisions allow→block→allow. The controller and real preflight share the recorded /usr/bin/cargo binding.

A Cargo command in an assistant message measures intent. Shim receipts record execution, arguments, origin, source-before inventory, exit and duration. Stop validity additionally checks output bytes/hashes, actual trace agreement and delivery of the feedback payload into the model transcript. A gate repair requires a failing first Stop, one delivered block, changed source, successful later Stop and observed low effort. A passing final candidate alone does not establish that chain.

The public [instrument](../../tools/reliable-agent-eval/compile_mechanism/README.md) has eighteen subprocess/parser/Git/budget tests. Independent review actually applied eleven original candidate mutations and six revision candidates; the final observed outcomes killed all eleven and all six respectively. These are finite chosen sets, not a coverage percentage. Resource failures during review were retained and excluded as mutation kills.

## Literal command counts and scoped supplemental readout

The frozen analyzer's author_requested_checks_completed_before_first_stop predicate literally requires exactly three Cargo arguments: check, --offline and --all-targets. Actual2-I adds --manifest-path naming its own Cargo.toml. Executing the frozen predicate against those complete receipts yields0, while three checks actually completed before Stop, with exits101,101,0. Calling that zero an absence of verification would confuse command syntax with command scope.

The original analyzer and its eventual output stay unchanged. The explicitly post-hoc command-scope-supplement.json records default-feature all-target checks with only an optional selector resolving to the same run's root manifest; it checks cwd/nearest manifests and rejects extra feature/scope flags and foreign manifests. Calibration executes ten argument boundary cases and the real overlap1-I literal2/scoped2 before extending to2-I literal0/scoped3. The report uses this labeled scoped readout for command-adherence descriptions. Acceptance, observer repair chains and spending are unaffected. [The counting defect](../issues/2026-10-29-compiler-command-count-argument-shape.md) remains mitigated for the frozen phase.

## Preserved invalid first protocol

The first protocol made one paid G invocation with client estimate $0.045103500000000005. Its Stop shim attempted the nonexistent /home/marius/.cargo/bin/cargo and returned127. Its stacktrace prompt also referred to an evaluator body withheld by preparation, and the author refused that task. The original registration, receipt, source and analysis remain sealed INVALID, with no quality numerator or denominator.

The revision uses the already existing behavior-description prompt and TASK.md, checks /usr/bin/cargo before payment, repeats real preflight/calibration and receives focused independent mutation review before freezing. This is one disclosed restart after instrumentation failure. The original charge remains in the revised prior $10.543943499999999865; historical source/evaluator bytes are preserved. Revised results must not be described as a matched replication of the older stacktrace-prompt cohorts.

## Exact-byte executable recovery during unpaid grading

After valid author2-L completed, its first parent-grade preflight found that the frozen /home/marius/.local/share/claude/versions/2.1.309 input no longer existed. The failed command stopped before source reconstruction or compilation and its original traceback is retained. The source patch, author receipt and charge remain unchanged.

The official2.1.309 linux-x64 manifest and downloaded293469552-byte binary both match the registered SHA-2564d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658. Root retained the binary privately and exclusively restored the missing registered pathname, with a dated restoration receipt. The default launcher remained2.1.312 and profiles/registration/model/effort were unchanged. Parent grading then revalidated all frozen inputs and reconstructed the original patch in a new log. This was an unpaid preflight recovery, with no paid author retry or additional protocol restart.

[Official documentation](https://code.claude.com/docs/en/setup#install-on-network-storage) describes native cleanup of older versions. That policy is consistent with the observed disappearance; the deleting process is unconfirmed. [The infrastructure bug](../issues/2026-10-29-frozen-cli-version-pruning.md) is mitigated, with private executable retention required for future designs.

## Results and costs

| Arm | Accepted / launched | First Stop clean | Final Stop clean | Gate blocks | Client USD total | Mean client USD / call |
|---|---:|---:|---:|---:|---:|---:|
| L | 1/2 | 1/2 | 1/2 | 0 | 0.6756202 | 0.3378101 |
| I | 2/2 | 2/2 | 2/2 | 0 | 0.7601841 | 0.3800920 |
| G | 2/2 | 1/2 | 2/2 | 1 | 0.7369161 | 0.3684580 |
| X | 2/2 | 2/2 | 2/2 | 0 | 2.5005412 | 1.2502706 |

| Call | Parent result | Behavioral tests executed | Client USD | CLI wall seconds | Parent compiler/test subprocess seconds | Scoped author checks before first Stop |
|---|---|---:|---:|---:|---:|---:|
| 1-G | accepted19/19 | 19 | 0.2556990 | 200.513 | 204.574 | 0 |
| 1-I | accepted19/19 | 19 | 0.3474235 | 318.474 | 213.109 | 2 |
| 1-L | accepted19/19 | 19 | 0.2652021 | 319.033 | 451.276 | 0 |
| 1-X | accepted19/19 | 19 | 1.2557812 | 619.464 | 182.103 | 0 |
| 2-G | accepted19/19 | 19 | 0.4812171 | 360.654 | 186.165 | 0 |
| 2-L | compile | 0 | 0.4104181 | 144.576 | 47.543 | 0 |
| 2-I | accepted19/19 | 19 | 0.4127606 | 210.453 | 201.471 | 3 |
| 2-X | accepted19/19 | 19 | 1.2447600 | 636.889 | 182.333 | 0 |

Revised client estimates: USD4.6732616. Cumulative: USD15.2172051 /20; remaining USD4.7827949. Earlier invalid-protocol charge is included in the prior, excluded from quality fractions. Native orchestration/review cost is unknown.

All eight CLI invocations and parent instruments were valid. Seven candidates passed the same nineteen named checks;2-L failed compilation and executed zero behavioral tests. These are repeated evaluations of the same nineteen named checks on one task.

As a post-hoc descriptive cohort ratio, total launched client dollars divided by accepted candidates is L$0.6756, I$0.3801, G$0.3685 and X$1.2503. The failed L charge remains in that numerator. This ratio is not an estimate of future reliability or total workflow cost.

## Interpretation and limits

The active mechanism is a concrete feedback loop over a check whose scope exceeds the author's own checks. In2-G, two completed narrow library checks passed, the first Stop then found E0063 in an integration-test constructor, and one delivered compiler diagnostic led low to fix five integration constructors. Complete source inventories show that only those five test files changed after first Stop. The next author all-target check and final Stop passed, and the independent parent accepted19/19. This demonstrates one compiler-class repair at observed low effort, with no effort switch or stronger repair model.

A precise instruction also exposed this class before completion:1-I checked101→0, and2-I checked101→101→0; both changed their implementation and passed the parent19/19. Those are instruction-arm trajectories. They do not isolate every reason for the correction or imply the older generic verification paragraph worked. In ordinary1-L, low voluntarily repaired E0609 from a narrow library test and subsequently passed19/19. Ordinary2-L instead ended with E0063 at silent Stop and failed the independent compiler check, executing zero behavioral tests. G1 passed first Stop with no feedback; its success cannot be attributed to automatic repair.

These observations support trying exact executable checks plus bounded diagnostic feedback for this failure class. Two samples per arm on one known historical task do not establish population reliability, low/xhigh equivalence, or statistical superiority. The changed task surface and CLI prevent pooling this pilot with the older stacktrace-prompt cohorts as a matched effect.

The frozen nineteen task-contract checks do not establish complete TASK.md semantics or every preserved behavior. Source review saw differences in prewalking versus interleaving, growing multi-target totals and counter conversion; those differences are not comprehensively isolated by this contract. Both xhigh authors' broader test commands returned101, so frozen-contract acceptance must not be described as a wholly green test suite. No broad-suite base counterfactual was run here to assign every such failure to author code or fixture conditions.

CLI wall time includes model execution and its tool waits. Parent timings sum only compiler/test subprocesses; they exclude source preparation, queueing, root review, preflight, repository gate and native orchestration. Shared-cache eviction caused1-L parent compilation to build cold. Repeated denied Git/shell requests also appear in author transcripts under the common whitelist. Latency differences are descriptive, not isolated effort effects. Native orchestration/review dollars and CPU costs remain unknown; lower model client estimates cannot establish total workflow savings.

The next useful validation is a prospectively selected transfer set, rather than further repetitions of this known task: freeze existing repository checks and independent compatibility cases before author outcomes, include behavioral failures that compile cleanly, and measure activation, accepted outcomes and total workflow cost. Preserve the same distinction between an instruction that is followed, a gate that actually intervenes, and an evaluator that only selects a final candidate.

## Verification and retained evidence

Before the revised paid pilot, the required full repository gate rerun exited0 after the tracker augmentation's missing concrete append-entry recipe was repaired. It completed Python, formatting, Clippy, lean and default suites in1584.61 seconds;395 tracked source/build paths remained unchanged. The gate's printed heavy Python/server-stack/local-weight coverage limits still apply. No production Rust implementation is part of this research change.

Original private evidence: .codescout/resident-private/compile-mechanism-20261029. Revised registration, controller/preparation/tests, input archive, raw receipts/transcripts, shim traces, exact patches/inventories, scope reviews, independent parent logs and final analysis: .codescout/resident-private/compile-mechanism-r2-20261029 under the shared codescout project. The final-outcome.json closure records their hashes plus this report, local commit and stable patch-id. Final scoped verification confirms all twelve old structured rows and definition sections are preserved, including the four prior negative classifications; all eight revised CLI invocations and parent instruments are valid, source/protected inputs reconcile, and the report's relative links exist.

The new observed catalog create failure—an error left a catalog row with an empty filesystem document—is recorded separately in docs/issues/2026-10-29-artifact-create-partial-filesystem-failure.md. Its cause is unconfirmed and no production fix is claimed. Existing metadata/body were restored and reread through the catalog.
