---
id: '4f6741e9a5b2ad4d'
kind: research-note
status: archived
title: 'Historical: Finite cohort results — verification procedure did not repair the oracle'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-02-finite-cohort-results.md
source_body_sha256: 64d2e00834bf0465558390c82ba79c339cccf9df2fc25d7a2a308c6248af9560
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-02-finite-cohort-results.md
  sha256: '337c418aaed6409da60953d37ebcd970686844ffc77feb3122bbc829ff795a07'
  body_sha256: '64d2e00834bf0465558390c82ba79c339cccf9df2fc25d7a2a308c6248af9560'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-02-finite-cohort-results.md
  sha256: '337c418aaed6409da60953d37ebcd970686844ffc77feb3122bbc829ff795a07'
  body_sha256: '64d2e00834bf0465558390c82ba79c339cccf9df2fc25d7a2a308c6248af9560'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-02-finite-cohort-results.md

---


# Finite cohort results — verification procedure did not repair the oracle

**Valid:** dated 2026-11-02

## Decision and what changed

**HOLD promotion of the structured bundle.** The fixed six-session pilot completed without retry, replacement, invalid protocol, cap stop or unknown charge. Ordinary low, structured low and fixed xhigh each pass **1 of 2 task endpoints**: all pass Tokio6603; all fail uutils6575's raw filename output requirement.

Structured low produced actual before-fix tests and assertion mutation/restoration at lower model-client cost than xhigh, but ordinary low matched its endpoint score at still lower cost. The unresolved failure is the expected-output oracle: structured low wrote an exact assertion for the wrong filename bytes. Its assertion negative control worked and the solution still failed independent acceptance.

This is a useful prospective residual defect, rather than another selected Axum/ripgrep retry. The new task solutions were generated only after [registration](2026-11-02-finite-cohort-registration.md), common fixtures, controls, review and actual freeze. Two tasks/one session per arm/task do not establish statistical parity or causal efficacy.

## Fixed population and results

Same Sonnet5.5 CLI, isolated source/config/session trees, common Cargo-only shell authority; low for L/S and xhigh for X. Exact order T-L,T-S,T-X,U-X,U-S,U-L. Independent acceptance was never fed back to any of these sessions. Both components must pass.

| Slot | Recorded effort | Original full target | Private target | Endpoint | Client USD, rounded | CLI-reported wall seconds |
|---|---|---:|---:|---|---:|---:|
| T-L | low | 89/89 | 3/3 | pass | 0.061802 | 46.006 |
| T-S | low | 89/89 | 3/3 | pass | 0.102714 | 151.374 |
| T-X | xhigh | 89/89 | 3/3 | pass | 0.163063 | 176.300 |
| U-X | xhigh | 64/65 | 1/1 | miss | 0.270810 | 144.782 |
| U-S | low | 64/65 | 1/1 | miss | 0.130439 | 393.046 |
| U-L | low | 64/65 | 1/1 | miss | 0.104368 | 126.544 |

Tokio has85 required dataset names and89 fully enumerated original-target names; all four additional names also pass. Its private target has3 test functions, with multiple boundary iterations inside them. Uutils has65 runnable original names after three predeclared source-ignored exclusions, plus1 private test function containing four inputs. Every graded population has zero ignored names. Do not count loop inputs as libtest functions or treat a checksum-only component pass as the endpoint.

All12 terminal component grades were independently re-parsed with exact source/log/fixture binding. Final audit rechecked registered sessions, raw/saved hashes and costs, actual recorded request effort/model/protocol and reached evaluator stages. Six valid outcomes, three endpoint passes, three behavioral misses; no not-run slots.

## Cost and latency

| Workflow | Accepted tasks | Mean whole-session model-client USD |
|---|---:|---:|
| Ordinary low | 1/2 | 0.08308510000000001 |
| Structured low | 1/2 | 0.116576650000000005 |
| Fixed xhigh | 1/2 | 0.21693635 |

On this cohort, structured low's mean client cost is46.2623% below xhigh and40.3099% above ordinary low. Those are observed session costs including model self-correction/tool-result context, not population estimates. Structured low also took longer than either other arm in the CLI-reported durations; client dollars are not a latency measure.

Exact phase client total **0.83319620000000003 USD**. Prior closed total24.269845199999999810; new cumulative **25.103041399999999840 of40**, remaining **14.896958600000000160**. The7.20 phase ceiling was not approached. Primary receipt arithmetic and the separate raw reader agree. Native orchestration/review, CPU and invoices remain unmeasured; no all-in dollar claim. The sequential driver completed in1922s, including independent grading/audits beyond CLI session durations.

## What the verification procedure actually did

L issued one completed Cargo shim execution on each task, after its production change. It did not establish a pre-fix behavioral failure. The U-L run omitted exact invalid filename output bytes.

T-S has five completed captured Cargo executions: behavioral failure101 with tests only, normal pass0, cargo check0, assertion-negative-control101, restored pass0. The negative control changes only the bounded post-receive is_empty assertion; unbounded still passes. Normal, restored and terminal patch SHA match. U-S has four: before-fix behavioral101, normal0, changed-checksum assertion101, restored0. Its normal/restored patch SHA match. These are actual applied controls, not a reasoning-only mutation lens.

Neither S run exposed the requested explicit contract card in inspected visible assistant text or captured test/final patches. Both used an alternative public API (unbounded channel; MD5 untagged). No separate variant was attempted. The frozen design says at most one, so zero is recorded without inventing a lower bound or excluding a slot. Adherence is partial on the visible-card requirement; all outcomes remain in denominators.

T-X independently tested a production revert: its four new tests fail and the source is restored. U-X's first Cargo101 was a wrong package-feature command, with exactly the same source patch across all three executions. It is a tooling correction, not evidence of behavioral source repair. Completed captured Cargo executions: L2, S9, X8; these count trace directories with complete process receipts, not every attempted/denied tool call.

The registered relative-cost/non-worse-score conditions hold and baseline behavioral repair was observed in S. They do not supply an incremental quality gain over ordinary low or repair the shared residual defect. The two-task result remains a diagnostic hold rather than a reliability promotion.

## The oracle failure, measured as bytes

Every U artifact passes the added private checksum/size/status fixture but fails the original test_cksum::test_non_utf8_filename. The original test compares complete stdout bytes. The three models fixed OsString parsing and args_os, then retained lossy filename display.

| Observation | Invalid filename byte representation |
|---|---|
| Public input and GNU stdout | FF |
| Structured-low expected string | U+FFFD, UTF8 bytes EF BF BD |
| Independent original fixture | Requires original FF |

U-S explicitly asserts the whole wrong string, then changes only4294967295 to4294967294 for its negative control. That proves the assertion consumes the changed checksum expectation; it does not establish correctness of the filename expectation. U-X's new tests check the output prefix and explicitly leave raw output as a separate change. U-L also uses a prefix and explicitly skips the exact filename bytes.

A separate GNU9.12 byte capture on the **same existing public empty-file input** confirms stdout preserves FF. This is oracle calibration, not an added candidate acceptance case or revised scoring rule. Its binary identity, exit/status and full stdout hex are retained. No test/fixture or model prompt changed after the paid outcomes.

Our added private fixture is also prefix-based by design, so it misses this residual formatter defect despite base-fail/gold-pass calibration. The original dataset component provides the needed independent discrimination. Base/gold calibration establishes those controls; it does not prove sensitivity to every remaining contract.

## Next mechanism and boundary

The next concrete mechanism to test is **public reference comparison in the native representation, followed by at most one bounded low repair**, with independent held-out acceptance and the initial solve/check/repair costs counted together. For this CLI, compare raw GNU stdout bytes; do not infer a reference contract from terminal glyphs. A comparable rule for other tasks needs a justified public oracle or metamorphic relation.

Any repair of this already observed uutils artifact must be a **separate exposed diagnostic**, not an extra slot, fresh transfer claim or rescued score in this pilot. Do not repeat the same prompt bundle, increase effort, add replacement seeds or expose the private acceptance to the current six sessions. No next paid phase has started or been registered.

## Verification, preservation and recovery

Prepayment commit85ce65670fdd671827077d43670bcf6fbb9c1bf3, patch-id3bba4ccabaca8dba092ef2537d116ca704b1d8d4. Actual freeze181files SHAfc87d1c5f0c06c14ac737c2becfe2d31eb67402a2c31fbccfec3763f896ef549; final verification confirms the identical manifest and all declared bytes, plus65 selected source identities.

Fresh full repository gate: PYTHON_LIGHT/FMT/CLIPPY/LEAN/DEFAULT all0; printed heavy Python/server-stack/local ONNX limits remain. Focused helper9/9, worker12/12, audit8/8. One fresh strong review found four Important defects, fixed in one observed RED/GREEN pass. Initial six applied mutants:2killed4survived0errors; four reapplications after consumer tests:4killed0survived0errors, exact restorations. [Formatter boundary](../issues/2026-11-02-finite-cohort-host-formatter-bypasses-private-path-denies.md), [early misses](../issues/2026-11-02-finite-cohort-audit-assumes-execution-for-early-misses.md), [protocol audit](../issues/2026-11-02-finite-cohort-audit-trusts-protocol-outcome-annotation.md), [consumer guards](../issues/2026-11-02-finite-cohort-guard-tests-miss-consumer-call-sites.md) retain separate fixed records. No deferred separate Minor.

All67 recorded adapter-container IDs are absent from docker ps -a; no broad prune or peer-container deletion occurred. Generated control-source trees had16314 regular files and420 literal symlinks. A tar archive preserves their bytes/modes/link strings; full archive inverse matches the original inventory, independently calibrated against GNU find+sha256sum+readlink, before removing only those generated trees. Archive SHA65291081546ae36561a2054b19664a436e37edae3c865376b073516193ab116a. Physical closure can therefore use regular files while original link identities remain preserved in the archive/inventory.

Evidence root **/home/marius/.cache/codescout-research/codescout/finite-cohort-20261102**; retained V/worktree/native ledger. Key files: analysis/final-audit.json, metrics.json, adherence.json, gnu-raw-filename.json, container-cleanup.json, source-archive-check.json, source-archive-inventory.json; review and mutation-controls retain all failures/restorations. Prior C paid closure and prior T sealed publication were reverified unchanged.

The publication/complete seal is recorded **after this result commit**, binding exact historical result/hub/plan/issue bytes rather than a future mutable hub. Canonical read-only verification after closure: python3 -B <evidence-root>/close.py verify. Do not initialize LSP in sealed evidence or author roots. No merge/push; no real model job remains running.

## Rulings retained from execution

- Reuse and retain V/native ledger, overriding workspace deletion: costs disk space.
- Overlap offline preparation with the fresh gate before task-start2: chronology is less tidy; logs are retained.
- Exclude only three source-declared ignored U tests before model outcomes: graded coverage excludes those names explicitly.
- Remove only U --nocapture after interleaving refusal: success-output capture differs; failure logs and cases remain.
- Place the one technical review before payment: later closure work has author verification and pinned/calibrated prior engine rather than another reviewer.
- Leave empirical outcomes/adherence/charges to execution: all were subsequently measured here.
- Do not presume future commit/freeze/seal exists: actual emission/verification gates are retained.
- Public-fixture chronology rests on the executor's account: independent chronological proof remains absent.
- General statistical parity is outside two tasks: no generalization is claimed.
- Invoice/native/CPU dollars remain unknown: model-client scope is explicit.
- Comprehensive hostile-agent/OS/permission-engine testing is outside scope: concrete host command routes were removed; residual isolation assurance is bounded.
- Old experiments/full gate were not independently rerun by the reviewer: controller gate and selected old closures were verified; unrelated older defects remain outside review.
- At-most-one variant permits zero: adherence could be interpreted permissively, but no endpoint/cost denominator changed.
- Archive only generated unsealed control sources after complete inverse and GNU calibration: recovery now requires extraction; file bytes/modes and literal targets were checked before deletion.

Deferred minors: none.
