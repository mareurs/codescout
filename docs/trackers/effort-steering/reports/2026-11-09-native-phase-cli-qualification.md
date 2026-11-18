---
id: '06b0cff569398355'
kind: report
status: active
title: Native three-phase CLI qualification, applied review and remaining admission limits
tags:
- effort-steering
- generic-mechanism
- native-qualification
topic: effort-steering
---

# Native three-phase CLI qualification — 2026-11-09

## Question and scope

Can the generic investigate → repair → challenge controller execute through the actual pinned CLI and Codescout, consume real instruction-delivery evidence, capture the source and replay final checks without spending on a model? This is a synthetic runtime qualification. It measures neither repair efficacy nor low/xhigh equivalence nor adequate real-project onboarding.

## Implementation

In the isolated cf66 worktree on research/evidence-protocol-20261109, added native_executor.py and qualify_native.py under tools/effort_protocol. NativeExecutor reuses the existing agent_lifecycle.external.launch lifecycle, accounting, source capture and independent cleanup. Its paid path is disabled by default. The same generic phase policy and prompts are used; source names and commands occur only in the explicit qualification fixture.

The new concrete admission compares the readiness image, read-only Codescout executable bytes, admitted public source and selected source-only knowledge with the actual executor inputs, then rechecks before launch. The underlying prepared-input hash contract pins the selected executable too. Context auditing retains settled cost and safe capture if request parsing fails. Independent readiness cleanup proceeds even if trace persistence fails.

## Actual controls observed

- v1: failed context admission. Actual Skill tool result is a short launching acknowledgement; its body is a separate injected user text block. The corrected consumer requires both. Original failure retained.
- v2: context/capture/cleanup valid, cost0/forwarded0, but protocol incomplete. Cargo.lock and .gitignore were generated after baseline freezing. The fixture now includes these deterministic preparation inputs before the baseline manifest; the production guard was not weakened.
- v3: three actual native CLI invocations, investigate→repair→challenge→closed, turns3, all context/cleanup valid, cost0 and provider forwarding0. An exit1 reproduction becomes exit0 after the fixture repair; adjacent probe and final host replay execute. All seven command receipts were independently reverified from their actual source/build/log bytes after the later storage incident.
- v4, after the review corrections: the actual three-phase control again closed with valid context/cleanup and zero forwarding/cost. It uses the new concrete backend binding. The fixture reused its existing source-language configuration and existing local embedding model, with no onboarding author call.

- R continuation-control follow-up: its new native test first failed because qualification ignored the requested arm. After threading R through the qualification request and removing only scripted protocol submissions, the same test passed in 39.711s. Three actual native invocations closed with valid context/cleanup, cost0/forwarded0, fixed.txt=1, and no submission.json artifact or submission instruction in any phase. Configured phase budgets remain equal. This is a synthetic runtime check, not L/R/P/X real-task parity.

The real rule delivery check uses the CLI's user,project,local setting sources and validates global/project visible rules at each saved request. System guidance comes from a successful source-bound read; Recon requires its actual invocation acknowledgement and separately injected body. These observations establish delivery, not obedience or semantic adequacy.

## Independent review and observed mutations

A fresh stronger reviewer inspected the whole current implementation and identified three Important defects: concrete executor/readiness mismatch, audit-write cleanup bypass and post-launch context parsing losing the known phase-charge component. Each was filed immediately in main docs/issues. Local regressions were observed RED before correction. Cleanup regression uses a real sleeping child, forced evidence-write failure and an isolated Docker boundary; the child is stopped and owned-worker removal is attempted despite persistence errors.

Reviewer applied11mutants:6killed,5survived,0invalid mutation runs. The five survivors were missing tests for .git exclusion, supervision cleanup, background-evidence completeness, artifact capture completeness and continuation bounds. After adding direct counterexamples, the same five individually applied mutations were all killed; restored16-test native boundary suite passes. A separate late-binding recheck mutation first survived14tests; source/executable drift regressions then killed it, and restored16tests passed. These are selected mutants, not whole-program mutation coverage.

No second review was substituted for the fix tests. Missing cohort selection/accounting, efficacy and real-project adapters were explicitly outside this review's checkpoint verdict.

## Public handoff follow-up and applied review

The first strengthened actual native test reproduced the missing handoff after all three invocations closed: observations absent from the lifecycle summary (RED, 105.673s). After adding a public-only export and qualifier wiring, the same consumer test passed (GREEN, 148.442s), zero forwarding/cost. Seven pure artifact tests plus one real-Docker next-actor transfer test first failed, then passed. The complete pre-review protocol suite then had 132 passes/two native skips.

Independent review applied five mutations to this separate handoff change: two killed, three survived the original eight-test lane. Three specific edge tests were added only after the mutants survived and scratch probes demonstrated the missing discrimination: source drift during empty R export, fresh export-read bytes differing from verified log bytes, and omission of challenge public-verification receipts. Exact original three mutants were reapplied against the updated 11-test lane: three killed, zero survived, zero invalid. Updated baseline and all three separately restored controls each passed 11 tests. Independent review approved the scoped handoff implementation; it did not review native/provider execution or the broad Rust gate. Parent read the baseline/mutant/restored summaries. This is selected regression coverage, not whole-program mutation coverage. [Guard issue](../../../issues/2026-11-09-public-handoff-export-guards-untested.md).

The actual first saved model requests for repair and challenge were then inspected: each contains the exact prepared public prompt in a user text block. The native test now asserts that boundary too. Running the strengthened assertions on the saved genuine requests passed; a copied request with the handoff removed failed exactly the delivery assertion. These are recorded-evidence positive/negative instrument controls, not new native invocations.

## Verification checkpoint

Latest complete protocol suite after public handoff regression fixes: 137 discovered, 135 executed/passed, two explicit native CLI skips in 106.597s. The native R test passed separately: one test in 39.711s. The strengthened actual P native consumer test passed one test in 148.442s after handoff wiring. Earlier P and R controls remain archived as their measured earlier-source scopes. The native boundary suite has 18 tests; the qualification helper/cleanup suite has three tests. No skip is counted as execution.

Full product gate finished FAILED after 2006s: Python 1311 passes/442 subtests, FMT0, CLIPPY0, LEAN 101 (5233 passed/7 failed/11 ignored), DEFAULT 101 (7870 passed/12 failed/14 ignored). It has 19 lane failure frames representing 12 unique test names. Its e9t TMPDIR was still too long for six LSP manager tests. A short external /var/tmp/ep9-01a15 focused run then passed 73 LSP tests (5 ignored) and the one memory boundary test. This does not retroactively make the full gate green.

Eight generated probe files, absent from the initial worktree status, polluted its real docs tree after the earlier in-workspace TMPDIR run. Their exact bytes were archived/read back before moving only those untracked synthetic files. The repository Class fit invariant then passed one focused test. The four attribution failures also pass in same-source focused controls with explicit empty Claude/Codex histories: four a_red_ tests pass in 0.81s (including the three failing WIP-author cases), and the directory-swap test passes separately in 0.71s. Both jobs exit 0. These controls narrow the known history dependency without measuring the exact timeout cause in the original full gate. No Rust source was changed; no broad gate rerun is scheduled.

## Evidence and preservation

Current handoff source/evidence: [public-handoff.tar.gz](../evidence/native-phase-cli-20261109/public-handoff.tar.gz), [manifest](../evidence/native-phase-cli-20261109/public-handoff-manifest.json), SHA-256 eb11e443bc8d404e4f70f7a905d9b2ff0e220d715b39cd6e26505a3e31de8c6d. Contains 1004 regular-file members, 1752781 compressed bytes, no symlinks; every member hash/mode read back. Includes native handoff RED/GREEN, actual-request replay and copied missing-delivery control, current source/tests, full protocol results and independent mutation patches/logs/copies. [Verification summary](../evidence/native-phase-cli-20261109/verification-summary.json) checks 25 named file hashes. All earlier archives stay unchanged.

Post-review R packet/prompt follow-up: [source and RED/GREEN archive](../evidence/native-phase-cli-20261109/control-followup.tar.gz), [manifest](../evidence/native-phase-cli-20261109/control-followup-manifest.json). SHA-256 93f49b71bac709d273a7b9589c5b4d34020caddcadcc4be96ac6c379fa249cae; 19 regular-file members hashed and read back. This supersedes only the checkpoint's protocol source for that small follow-up; the original checkpoint/control results stay unchanged. This snapshot precedes the later synthetic native R qualification; retain it unchanged.

Final checkpoint evidence is preserved in [checkpoint.tar.gz](../evidence/native-phase-cli-20261109/checkpoint.tar.gz) with its [manifest](../evidence/native-phase-cli-20261109/manifest.json). Archive SHA-256: f9f26a6276519f136372b4bd38c831a9b06d8f6c16a7fd3068d69d3621a7ee72; compressed bytes 2258503. It contains 1231 regular-file entries and 252 directory entries, no symlink entries. Every file hash and member mode was read back from the archive. This is an archive-member inventory, not a count of experiments or tests.

Includes failed v1, incomplete v2, closed v3/v4, the successful native unittest, final protocol source, independent review/reproductions/mutations, the local mutant-closure evidence and completed failed verification logs. Shared model caches, Git metadata, client/native homes and SQLite/tool stores are excluded; the fixture's selected source-only project/system knowledge files are retained. The then-running full-product gate is excluded from this frozen archive; its completed FAILED result is in the later R qualification archive.

The earlier v1/v2/v3 archive remains in the worktree ledger directory with 431 regular files and read-back hashes, SHA-256 e500c4ab58dec35ab9991664312517fd90a6599a95721779968e5015c9b8cdf5. All seven v3 command receipts were independently replayed through verify_receipt against actual source/build/log bytes after the quota incident.

Completed R qualification/source and red full gate: [archive](../evidence/native-phase-cli-20261109/r-qualification-and-gate.tar.gz), [manifest](../evidence/native-phase-cli-20261109/r-qualification-and-gate-manifest.json). SHA-256 be2c5f5274aaec43238b56c812c94126d309a4dc818d579faeee9c33996eae12;556regular-file members,1193040compressedbytes, each hash/mode read back. Contains native R RED/GREEN, current protocol source, full red gate log, focused LSP/memory/Class-fit logs and preserved leaked probes. No symlinks included. The later completed attribution-control logs and hashes are recorded separately in verification-summary.json. Old archives are unchanged. Client/native homes, model caches, Git/build trees and tool SQLite are excluded. No experiment memory journal was written.

## Admission limits and next step

Actual P and R synthetic native controls now pass within the explicitly measured scope. The P/R instruction leak and subsequent qualifier arm-selection omission were corrected with observed RED/GREEN checks. These follow-ups occurred after the independent review; the latest source and failed controls are separately archived. Real-task L/R/P/X preparation and context parity remain unqualified.

Paid readiness is false. This fixture does not qualify real Rust-project immutable command adapters, a substantive source-only knowledge corpus, all workspace scopes or equivalent L/R/P/X preparation. The256MiB offline evidence executor profile is a tiny-fixture profile; real-task resource/dependency/build adapters still need qualification. Default paid blocking is not a campaign-wide budget ledger.

A further Task 2 audit found the qualifier was forwarding lifecycle summaries instead of public evidence artifacts. [Handoff correction](../../../issues/2026-11-09-native-phases-miss-public-evidence-handoff.md) now has a passing actual native consumer check: repair receives investigation clauses/observations; challenge receives both prior public artifacts. This establishes delivery, not semantic use by a real repair model. Lifecycle/cost and private host objects stay outside the public artifact. Complete UTF-8/binary streams, execution identity and source/log integrity are checked. The preserved earlier checkpoint archives predate this correction.

The synthetic handoff checkpoint is now qualified within the recorded scope. Next qualify real-task immutable execution and common preparation. Freeze outcome-blind new-task selection and cohort/accounting/stop rules before any four-arm paid pilot. Old exposed1/2tasks remain development observations. The scientific goal stays open.

## Issues

[Skill body shape](../../../issues/2026-11-09-native-phase-skill-delivery-shape.md), [preparation inputs](../../../issues/2026-11-09-native-phase-preparation-input-drift.md), [concrete admission](../../../issues/2026-11-09-native-readiness-not-bound-to-executor.md), [cleanup ordering](../../../issues/2026-11-09-native-readiness-logging-skips-cleanup.md), [known charge](../../../issues/2026-11-09-native-context-error-loses-known-charge.md), [host mount ownership](../../../issues/2026-11-09-native-model-mount-host-ownership.md), [quota](../../../issues/2026-11-09-native-verification-tmpfs-quota.md), [TMPDIR constraints](../../../issues/2026-11-09-verification-disk-tmpdir-constraints.md), [probe pollution](../../../issues/2026-11-09-in-workspace-temp-tests-leave-repository-probes.md), [R instruction boundary](../../../issues/2026-11-09-native-control-submission-instruction.md).
