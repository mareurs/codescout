---
id: '826403c929037379'
kind: research-note
status: archived
title: 'Historical: Source-bound required and additional acceptance populations'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-01-fixed-acceptance-population.md
source_body_sha256: 78261cc64015a2fdaf26dde5b608708c32bcfe2c7767fdde328a81e30d5f7f23
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-01-fixed-acceptance-population.md
  sha256: '9823a1c374911730f120dbbcf531a0e60aec6b47e1e80477ae2e16142b290480'
  body_sha256: '78261cc64015a2fdaf26dde5b608708c32bcfe2c7767fdde328a81e30d5f7f23'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-01-fixed-acceptance-population.md
  sha256: '9823a1c374911730f120dbbcf531a0e60aec6b47e1e80477ae2e16142b290480'
  body_sha256: '78261cc64015a2fdaf26dde5b608708c32bcfe2c7767fdde328a81e30d5f7f23'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-01-fixed-acceptance-population.md

---


# Source-bound acceptance populations

The new component handles legitimate agent-added tests while preserving every fixed requirement. It enumerates the same captured source under fixed fixtures, requires all configured fixed cases and validates the complete execution against that enumeration with the unchanged strict parser. An additional failing test rejects acceptance. Required and additional counts remain separate.

This is an unpaid measurement-instrument improvement. It does not retrospectively repair the stopped transfer cohort or establish a low/xhigh mechanism result.

## Implementation and scope

[Design](../superpowers/specs/2026-11-01-fixed-acceptance-population-design.md); [plan](../superpowers/plans/2026-11-01-fixed-acceptance-population.md). New module: tools/reliable-agent-eval/external_feedback/fixed_acceptance.py. The passed-in adapter executes both stages with oracle=True and no author-artifact export; existing adapter and parser files are unchanged.

The evaluator checks actual process/log/source receipts, exact command suffix, oracle/timeout flags, patch bytes, metadata and expected/actual source inventories. It checks capture/configuration and both optional fixed-input patches before and after each stage. The population receipt records required, enumerated and additional names before execution. The strict existing classifier verifies full names, headers, footers, statuses and process agreement.

Supported scope is one Cargo libtest binary. Unsupported/malformed or multiple listing populations fail visibly. A compile failure is a rejected outcome with null test counts, never an invented empty passing population. This is receipt validation through the existing trusted adapter, not cryptographic attestation of arbitrary adapters or atomic exclusion of hostile filesystem writers.

## Actual unpaid development controls

| Captured source | Required cases | Additional cases | Full observed result | New helper outcome |
|---|---:|---:|---|---|
| Exact saved C-L xhigh Axum1730 | 7 | 1 | 8passed,0failed | accepted |
| Exact saved A-L ordinary-low Axum1730 | 7 | 0 | 7passed,0failed | accepted |
| Clean Axum1730 base | 7 configured | unavailable | E0277 compile failure during enumeration | rejected; null counts |

These are actual offline Docker checks, not unit-fixture claims. Their source/process/log/fixture hashes and runtime identity are preserved in private fixed-acceptance-population-20261101/controls and real-controls.json. The retained exact C-L eight-case log and source/patch SHA metadata are also committed as an exposed regression fixture.

C-L was used to develop this instrument. These checks provide no external validation, unseen-task transfer, quality-rate estimate or comparative model result. Original C-L outcome remains evaluator-invalid in the sealed old phase.

## Tests and observed review

Initial eight new missing-feature cases were observed failing before implementation. A later fixed-input drift test also failed before its guard was added. The inherited baseline discovered41tests:39passed,2defaultDocker skips. Before fresh review the complete relevant suite discovered54:52passed,2skipped.

One fresh strongest-model review actually applied six selected mutations in isolated copies. Original result:4killed,2survived,0invalid. The survivors removed expected-source inventory comparison and fixture_patch stability hashing. Baseline consuming probes rejected these defects while the mutants accepted them. Parent read exact diffs and full passing survivor logs before writing the two new consuming regressions; current guards were already correct.

One corrective pass added independent expected-inventory drift and second-fixture drift at both listing and execution. Final complete suite:56discovered,54passed,2skipped. Reapplied same six selected mutations:6killed,0survived,0invalid. Copy-local imported and executed subject paths, exact diffs and complete logs are retained. M4/M5 now fail with ValueError-not-raised assertions for both stages; M2/M6 runtime-error kills occur after successful subject execution, not at import or harness setup. This is selected mutation sensitivity, not exhaustive correctness.

The review's first audit-harness attempt incorrectly rejected controlled Git -C calls; it was retained as invalid and excluded. Original reviewer evidence was not overwritten. Two default skips require pinned Docker fixtures: test_real_container_rejects_changed_expected_source and test_failed_cargo_returns_snapshot_without_changing_source. The three real development controls above are recorded separately and do not silently count those skipped tests as executed.

Coverage findings: [expected inventory](../issues/2026-11-01-fixed-acceptance-expected-source-regression.md), [second fixture input](../issues/2026-11-01-fixed-acceptance-fixture-patch-regression.md).

## Project verification

Full project gate exited0 after1307s: PYTHON_LIGHT, FMT, CLIPPY, LEAN and DEFAULT all returned0. It used explicit process-local RUST_TEST_THREADS=4 and CODESCOUT_WIP_ATTRIBUTION_TIMEOUT_MS=120000 plus a fresh short TMPDIR. No shared configuration or Rust source changed. Prior U attribution failures remain in the stopped-cohort report; this is configured verification, not a production attribution-timeout fix.

Printed gate limits remain: three heavy Python suites excluded; server-stack not compiled; two local ONNX weight tests skipped; librarian absent in LEAN and covered only in DEFAULT. The gate started before final test-only additions; the final56-test suite and full six-mutant replay separately verified those additions against unchanged runtime.

The exact retained raw Cargo log ends with a blank line. Default git diff --check reports that data-fixture byte; command-local core.whitespace=-blank-at-eof passes without altering the raw log or shared Git configuration. The fixture SHA is checked by its consuming regression. No runtime or source-format warning was waived.

## Budget, preservation and next experiment

No new model-client calls occurred. Latest immutable stopped-phase anchor: fixed-checker-transfer-20261101/final-outcome.json SHA25643e17cd10ce2f7a9b648b2ba36611e5c72e49685406ffd08f58738827d91393d; cumulative23.269336999999999800 of authorized40, remaining16.730663000000000200 in model-client estimates. Native orchestration/review, actual billing and CPU costs remain unknown.

Old phase local commit1da056a7b52033eaa0d132ab9c8ac2497e018bad, patch-id2b3233aec692612897c970f0e3b162fa877ecf84. Its repeat verifier checks3092physicalfiles,71symlinktargets and13Gitobjects while preserving invalidity; preceding seal overlap608physicalfiles,7098links,12Gitobjects matches exactly.

New isolated worktree /home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101, is retained. No merge/push or model registration is part of this component change. Any next paid cohort needs a separate prospective registration using the new closed budget anchor and unused tasks. Exposed Axum evidence cannot become holdout validation by renaming the phase.
