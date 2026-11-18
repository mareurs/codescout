---
id: f893400349fdda39
kind: index
status: active
title: Effort-steering issue index
tags:
- effort-steering
- experiment-index
topic: effort-steering
---

# Experiment issue index — 2026-11-09

Preservation scope correction: [static prompt inputs omitted by Python selector](../../issues/2026-11-09-source-capsule-omits-static-prompt-inputs.md). Corrected via additional capsule/combined manifest, without replacing original evidence.

Final native preparation caveats: [primary evidence versus guides](../../issues/2026-11-09-readiness-witness-includes-supplemental-guides.md), [zero-byte index verdict](../../issues/2026-11-09-native-index-verify-empty-files-missing.md), [coverage guard test gaps](../../issues/2026-11-09-empty-coverage-guard-test-gaps.md), [separate memory DB collector assumption](../../issues/2026-11-09-common-corpus-collector-single-db-assumption.md). Local mitigations and applied results are in the [common preparation report](reports/2026-11-09-common-source-preparation.md); product empty-file verifier is not fixed. Current cf66 product gate passed; earlier failed gates remain historical.

Embedding-only transport: [redirect escape](../../issues/2026-11-09-embedding-relay-redirect-escapes-fixed-upstream.md), [incomplete framing](../../issues/2026-11-09-embedding-relay-truncated-http-bodies-succeed.md), [partial-client bounds](../../issues/2026-11-09-embedding-relay-unbounded-partial-clients.md), [numeric disconnect](../../issues/2026-11-09-embedding-relay-large-integer-disconnect.md), [masked Transfer-Encoding test](../../issues/2026-11-09-embedding-relay-transfer-encoding-test-masked.md). Local corrections and exact applied-review outcomes recorded; integration/gate remain separate.
Common preparation: [missing image corpus](../../issues/2026-11-09-real-source-image-has-no-onboarding-corpus.md), [guard tests](../../issues/2026-11-09-common-preparation-guard-test-gaps.md), [linked rules](../../issues/2026-11-09-preparation-frozen-rules-parent-link-escape.md), [output alias](../../issues/2026-11-09-preparation-dangling-output-link-redirects-write.md), [runtime permission drift](../../issues/2026-11-09-preparation-runtime-mode-drift-unbound.md), [read-only clones](../../issues/2026-11-09-preparation-readonly-input-produces-readonly-actor.md), [onboarding source version](../../issues/2026-11-09-onboarding-source-binding-ignores-gitignore-change.md). Current fixes are local/uncommitted; native preparation is not yet admitted.
Execution-profile follow-up: [raw inspection preservation](../../issues/2026-11-09-rejected-execution-profile-loses-runtime-inspection.md), [image-declared scratch mount](../../issues/2026-11-09-cargo-profile-image-declared-volume.md), [anonymous-volume cleanup](../../issues/2026-11-09-executor-cleanup-leaves-anonymous-image-volume.md), [explicit test-file inventory](../../issues/2026-11-09-test-input-inventory-accepts-directory-only-digest.md), [five observed regression-guard gaps](../../issues/2026-11-09-execution-profile-guard-test-gaps.md), [archive hard-link verifier](../../issues/2026-11-09-evidence-archive-verifier-rejects-hardlinks.md). Corrections are local and uncommitted; [scope and runtime controls](reports/2026-11-09-real-project-execution-profiles.md).

Public handoff follow-up: [missing phase evidence](../../issues/2026-11-09-native-phases-miss-public-evidence-handoff.md) has a locally passing actual native consumer check; [export regression guard gaps](../../issues/2026-11-09-public-handoff-export-guards-untested.md) tracks three actually surviving mutations and their fix verification. [Session log](session-log.md) records the measured seam finding; these runtime results do not establish repair quality.

Latest verification incident: [CLI fixture probes escaped into the research checkout](../../issues/2026-11-09-in-workspace-temp-tests-leave-repository-probes.md) — mitigated by preserving/moving eight synthetic untracked documents; exact write path remains untraced. Full product gate stays failed. [Native R boundary](../../issues/2026-11-09-native-control-submission-instruction.md) now has a passing actual synthetic CLI control; real-task parity remains open.

Additional control-arm finding: [native R inherited a protocol submission instruction](../../issues/2026-11-09-native-control-submission-instruction.md), locally corrected with two RED/GREEN consuming regressions; actual native R qualification remains open.

New native phase checkpoint issues (2026-11-09), separate from the earlier dated catalog snapshot: [Skill delivery shape](../../issues/2026-11-09-native-phase-skill-delivery-shape.md), [preparation input drift](../../issues/2026-11-09-native-phase-preparation-input-drift.md), [executor binding](../../issues/2026-11-09-native-readiness-not-bound-to-executor.md), [cleanup on write failure](../../issues/2026-11-09-native-readiness-logging-skips-cleanup.md), [known charge](../../issues/2026-11-09-native-context-error-loses-known-charge.md), [host mount ownership](../../issues/2026-11-09-native-model-mount-host-ownership.md), [tmpfs quota](../../issues/2026-11-09-native-verification-tmpfs-quota.md), [TMPDIR constraints](../../issues/2026-11-09-verification-disk-tmpdir-constraints.md).

Bug records remain canonical in docs/issues, as the project requires. This is a dated index, not a second bug-status authority. Read each record's caveat and fix provenance before treating fixed or mitigated as qualified.

Coverage: all 104 catalog rows matching topic or tag effort-steering in the main checkout at this inventory, plus the separately referenced pre-fix attribution gate issue below. It is not a count of all machine/worktree bugs or every historical defect. Refresh when a new experiment issue is filed or its disposition changes.

| Recorded status | Issue |
|---|---|
| mitigated | [New effort phase adapter invokes native actors without checking Codescout preparation](../../issues/2026-11-09-effort-native-phase-misses-preparation-admission.md) |
| investigating | [Effort evidence test reports cleanup timeout without identifying the timed-out Docker operation](../../issues/2026-11-09-effort-evidence-cleanup-timeout-diagnostic.md) |
| mitigated | [Readiness test fixture aliases registered probe arguments to observed arguments](../../issues/2026-11-09-effort-readiness-fixture-aliases-expected-and-observed.md) |
| mitigated | [Generated executable inputs are recorded but not checked by effort receipt readback](../../issues/2026-11-09-effort-protocol-generated-input-drift-not-checked.md) |
| mitigated | [Prospective receipt reader accepts a missing test inventory as the whole source snapshot](../../issues/2026-11-09-effort-protocol-missing-test-inventory-falls-back-to-source.md) |
| mitigated | [Uncertainty admission suite misses call-site and reference-retention guards](../../issues/2026-11-08-uncertainty-admission-test-gaps.md) |
| mitigated | [Uncertainty author entry shadows qualified harness prepare module](../../issues/2026-11-08-uncertainty-entry-module-name-collision.md) |
| mitigated | [Sequential oracle authors nest a non-idempotent runtime compatibility wrapper](../../issues/2026-11-07-baseline-pair-nested-runtime-wrapper.md) |
| mitigated | [Baseline oracle tests miss orchestration and capture guards despite a green suite](../../issues/2026-11-07-baseline-controller-integration-test-gaps.md) |
| mitigated | [Oracle capture scanner overlooks executable tests without the required prefix](../../issues/2026-11-07-oracle-capture-undeclared-tests.md) |
| mitigated | [A non-object oracle contract clause escapes protocol-invalid classification](../../issues/2026-11-07-oracle-contract-nonobject-clause.md) |
| mitigated | [Baseline author mount inventory omits equals-form Docker mounts](../../issues/2026-11-07-baseline-native-equals-mount-bypass.md) |
| mitigated | [Supervisor can pin the CLI's transient MCP discovery worker and abort its valid startup fallback](../../issues/2026-11-07-differential-oracle-native-startup-exit.md) |
| open | [Generated contrast tests accept known wrong repairs and reject the qualified reference](../../issues/2026-11-07-generated-contrast-tests-invert-repair-oracle.md) |
| mitigated | [Native startup diagnostic reuses one stderr file for multiple MCP connections](../../issues/2026-11-07-native-startup-stderr-overwritten-on-reconnect.md) |
| mitigated | [Native memory calibration guesses a Cargo test target and runs no public tests](../../issues/2026-11-07-memory-calibration-wrong-cargo-target.md) |
| mitigated | [Rust test parser misses passing test concatenated with test stdout](../../issues/2026-11-06-rust-grader-parser-misses-concatenated-test-status.md) |
| mitigated | [Memory calibration exception reporter masks the primary failure and omits its invalid receipt](../../issues/2026-11-07-memory-calibration-error-receipt.md) |
| fixed | [External private grading conflates linker crash with model test failure](../../issues/2026-11-06-private-grader-linker-crash-counted-as-model-failure.md) |
| mitigated | [Admission mock clock times out before the third sample and misses a removed memory guard](../../issues/2026-11-07-admission-clock-control-misses-third-sample.md) |
| mitigated | [Observation probe uses fallible expressions inside the test harness print macro](../../issues/2026-11-07-observation-probe-print-closure.md) |
| mitigated | [Contrast function-name selector includes helper functions as executable tests](../../issues/2026-11-07-contrast-selector-counts-helper-functions.md) |
| mitigated | [Contrast probe Dockerfile interprets image ID as a registry tag](../../issues/2026-11-07-contrast-probe-image-id-build-reference.md) |
| mitigated | [Contrast evaluator assumes every Cargo assertion failure exits 101](../../issues/2026-11-07-contrast-evaluator-cargo-exit-assumption.md) |
| mitigated | [Contrast evaluator Docker copy to tmpfs reports success but patch is absent](../../issues/2026-11-07-contrast-evaluator-git-context.md) |
| mitigated | [Contrast verifier configured zero continuations cannot receive its background result](../../issues/2026-11-07-contrast-verifier-zero-continuation-limit.md) |
| open | [Pilot result explanation conflates private baseline-passing tests with actor-visible existing tests](../../issues/2026-11-07-p2p-test-visibility-misreported.md) |
| open | [Saved-grader tests do not observe task image preservation or independent cleanup guard](../../issues/2026-11-07-saved-grader-boundary-mutation-gaps.md) |
| open | [Saved-candidate grader permits a second run by changing output directory](../../issues/2026-11-07-saved-grader-output-path-allows-retry.md) |
| fixed | [Native bootstrap says semantic index is not built while status is queryable](../../issues/2026-11-04-semantic-bootstrap-disagrees-with-queryable-index.md) |
| fixed | [Cargo check shim hides generated snapshot artifacts from the author](../../issues/2026-10-31-cargo-shim-hides-generated-snapshots.md) |
| open | [Workflow session guard forwarding can disappear without lifecycle or adapter test failure](../../issues/2026-11-06-adapter-workflow-session-guard-coverage.md) |
| open | [Common adapter smoke bypasses concrete calibrated cost reconciliation](../../issues/2026-11-06-adapter-common-reconcile-bypass.md) |
| open | [Adapter source freeze hardcodes a gate started before the review fix pass](../../issues/2026-11-06-adapter-freeze-gate-source-binding.md) |
| open | [Adapter immutable arm drift admission lacks regression coverage](../../issues/2026-11-06-adapter-immutable-drift-coverage.md) |
| open | [Supervisor classifier source drift recheck lacks regression coverage](../../issues/2026-11-06-supervisor-policy-drift-coverage.md) |
| open | [Adapter daemon availability guard is masked by the cleanup fixture](../../issues/2026-11-06-adapter-daemon-proof-coverage.md) |
| open | [Adapter public round completeness and identity checks lack regression coverage](../../issues/2026-11-06-adapter-public-round-coverage.md) |
| open | [Adapter plugin and read-only source validation can be skipped without test failure](../../issues/2026-11-06-adapter-plugin-drift-coverage.md) |
| open | [Adapter writable input drift admission is not pinned by backend tests](../../issues/2026-11-06-adapter-writable-drift-coverage.md) |
| open | [Supervisor process group isolation validation lacks regression coverage](../../issues/2026-11-06-supervisor-process-group-coverage.md) |
| open | [Supervisor native label ownership validation lacks regression coverage](../../issues/2026-11-06-supervisor-label-coverage.md) |
| open | [Workflow final supervision failures can be ignored without test failure](../../issues/2026-11-06-workflow-final-failure-coverage.md) |
| fixed | [Prospective qualification tests miss native evidence wiring and runtime guards](../../issues/2026-11-06-prospective-runner-integration-coverage.md) |
| fixed | [Prospective registration builder expects object values from a digest-string manifest](../../issues/2026-11-06-prospective-registration-digest-schema.md) |
| fixed | [Prospective policy accepts stripped full-step evidence after fatal tails disappear](../../issues/2026-11-06-prospective-policy-missing-step-tails.md) |
| fixed | [Prospective policy mistakes Cargo test-summary errors for compiler failure](../../issues/2026-11-06-prospective-policy-cargo-summary-compiler-false-positive.md) |
| fixed | [Audit suite misses P2P recovery and three safety-guard mutations](../../issues/2026-11-06-audit-tests-miss-p2p-recovery.md) |
| fixed | [Audit reparse attributes a recovered ordinary test to another test's glued line](../../issues/2026-11-06-audit-reparse-cumulative-line-misattribution.md) |
| fixed | [Toolchain classifier mistakes compiler source excerpts for fatal signals](../../issues/2026-11-06-toolchain-classifier-source-excerpt-false-signal.md) |
| fixed | [Campaign engine permits registration and admission replacement after its start barrier](../../issues/2026-11-06-semantic-campaign-registration-not-pinned-through-run.md) |
| fixed | [Campaign engine accepts a workflow flag contradictory to completed public verification](../../issues/2026-11-06-semantic-campaign-receipt-coherence-unchecked.md) |
| mitigated | [Semantic wire registration omits prospective plugin input identity](../../issues/2026-11-06-semantic-wire-plugin-identity-unbound.md) |
| fixed | [Plugin identity tests accept removal of nested-file content hashes](../../issues/2026-11-06-plugin-identity-nested-hash-unpinned.md) |
| fixed | [Plugin identity tests do not isolate either stability guard](../../issues/2026-11-06-plugin-identity-stable-read-guard-unpinned.md) |
| fixed | [Semantic wire verifier does not bind native calls to registered inputs](../../issues/2026-11-06-semantic-wire-native-inputs-unchecked.md) |
| fixed | [Semantic wire verifier accepts truncated Recon body retaining one marker](../../issues/2026-11-06-semantic-wire-recon-marker-not-body.md) |
| fixed | [Synthetic wire verifier assumes deferred MCP tools are declared in the first request](../../issues/2026-11-06-wire-verifier-assumes-eager-mcp-tools.md) |
| fixed | [Wire prompt negative control masks removal of the full-prompt guard](../../issues/2026-11-06-wire-prompt-control-removes-guidance-too.md) |
| fixed | [Semantic runtime tests do not pin snapshot identity binding](../../issues/2026-11-06-semantic-runtime-snapshot-identity-coverage.md) |
| fixed | [Semantic runtime tests miss truncated actor AGENTS rules](../../issues/2026-11-06-semantic-runtime-agents-rules-coverage.md) |
| fixed | [Semantic runtime clone isolation guard is unpinned by negative controls](../../issues/2026-11-06-semantic-runtime-clone-guard-coverage.md) |
| fixed | [Semantic runtime preparation tests do not require per-arm actor validation](../../issues/2026-11-06-semantic-runtime-preparation-validation-coverage.md) |
| fixed | [Semantic-contract verifier accepts nonpositive caps and reserves](../../issues/2026-11-06-semantic-contract-verifier-budget-bounds.md) |
| fixed | [Semantic-contract verifier accepts noncanonical arm identity and registration stage](../../issues/2026-11-06-semantic-contract-verifier-unbound-arm-identity.md) |
| fixed | [Semantic-contract verifier controls miss four load-bearing guards](../../issues/2026-11-06-semantic-contract-verifier-controls-miss-guards.md) |
| mitigated | [Focused contract probe assumes every failed Cargo test exits101](../../issues/2026-11-06-focused-probe-assumes-cargo-exit-101.md) |
| open | [External repair public verification stays red on temporary execution and auxiliary test failures](../../issues/2026-11-06-external-public-verification-temporary-execution-failures.md) |
| fixed | [Disposable qualification source guard tests do not exercise command prefixes](../../issues/2026-11-06-qualification-guard-prefix-uncovered.md) |
| mitigated | [Unchanged-source qualifier rejects native startup gitignore addition](../../issues/2026-11-06-baseline-qualifier-rejects-native-gitignore-initialization.md) |
| open | [Lifecycle README still describes the retired two CPU profile](../../issues/2026-11-06-lifecycle-readme-stale-cpu-profile.md) |
| fixed | [Private grader zero-test substring rejects forty passing tests](../../issues/2026-11-05-private-grader-zero-substring-rejects-forty-tests.md) |
| fixed | [Closed recovery history arm identity guard lacks regression](../../issues/2026-11-05-closed-recovery-arm-identity-guard-untested.md) |
| fixed | [Legacy CLI turn ceiling censors xhigh before public verification](../../issues/2026-11-05-workflow-xhigh-censored-by-legacy-turn-cap.md) |
| mitigated | [Aborted workflow readback incorrectly requires reference-only tests](../../issues/2026-11-05-workflow-readback-requires-reference-only-tests.md) |
| fixed | [Cold complete workflow times out under a deadline qualified only with a warm reference cache](../../issues/2026-11-05-cold-workflow-deadline-unqualified.md) |
| fixed | [Aborted history raw terminal corroboration lacks call-site regression](../../issues/2026-11-05-aborted-history-raw-cost-guard-untested.md) |
| fixed | [Cold admission command mismatch guard has no regression coverage](../../issues/2026-11-05-cold-admission-command-guard-untested.md) |
| fixed | [Workflow cleanup ignores a pinned owner's immutable container ID](../../issues/2026-11-05-workflow-cleanup-ignores-pinned-container-id.md) |
| fixed | [Workflow timeout cleanup discards native background logs](../../issues/2026-11-05-workflow-timeout-loses-native-background-logs.md) |
| fixed | [Background job reader ignores an already pinned native container ID](../../issues/2026-11-05-background-reader-ignores-pinned-container-id.md) |
| fixed | [Workflow campaign default legacy CLI budget change survives mutation](../../issues/2026-11-05-workflow-default-cli-budget-coverage.md) |
| fixed | [Workflow campaign settled-lifecycle guard survives mutation](../../issues/2026-11-05-workflow-lifecycle-endpoint-coverage.md) |
| fixed | [Workflow campaign forwarded-total guard survives mutation](../../issues/2026-11-05-workflow-forwarded-total-coverage.md) |
| fixed | [Workflow campaign latest-terminal selection survives mutation](../../issues/2026-11-05-workflow-latest-terminal-coverage.md) |
| fixed | [Workflow campaign default composition omits lifecycle without a failing test](../../issues/2026-11-05-workflow-default-fingerprint-coverage.md) |
| fixed | [Workflow campaign ledger lacks a nonfinal marker after unknown attempt cost](../../issues/2026-11-05-workflow-ledger-unknown-cost-state.md) |
| mitigated | [Final runtime qualification wrapper omits its exclusive parent directory](../../issues/2026-11-05-runtime-qualification-parent-directory.md) |
| mitigated | [Native reference gate retains a process-group grandchild under container PID1](../../issues/2026-11-05-native-reference-orphan-reaping.md) |
| mitigated | [Native full-gate reference control fails spawning test worker threads](../../issues/2026-11-05-native-full-gate-test-worker-exhaustion.md) |
| mitigated | [Full Rust reference control hits temporary-build quota and leaves incomplete receipts](../../issues/2026-11-05-public-profile-reference-temporary-build-quota.md) |
| fixed | [Verified worker CPU quota guard survives mutation testing](../../issues/2026-11-05-verified-worker-resource-conflict-coverage.md) |
| fixed | [Verified worker admits Docker run argv without usable image](../../issues/2026-11-05-verified-worker-malformed-native-argv.md) |
| fixed | [Verified worker admits unknown filesystem output before credentials](../../issues/2026-11-05-verified-worker-unknown-filesystem.md) |
| mitigated | [Reference-control readback confuses newly generated runtime metadata with source changes](../../issues/2026-11-05-reference-readback-runtime-metadata.md) |
| mitigated | [Full public-profile reference control omits the worker's Git baseline initialization](../../issues/2026-11-05-public-reference-control-missing-git-baseline.md) |
| mitigated | [CPU admission probe attempts to execute from a non-executable temporary mount](../../issues/2026-11-05-cpu-probe-temp-execution-permission.md) |
| fixed | [Private build verifier rejects Docker absence because it assumes uppercase error text](../../issues/2026-11-04-native-build-verifier-rejects-lowercase-docker-absence.md) |
| mitigated | [Diagnostic source overlay recreates symlinks already materialized by Git clone](../../issues/2026-11-04-context-clone-overlay-recreates-existing-symlinks.md) |
| mitigated | [Native worker bootstrap omits Codescout guidance trimmed from initialize instructions](../../issues/2026-11-04-native-worker-omits-trimmed-codescout-guidance.md) |
| mitigated | [Context audit mistakes client HTML-comment stripping for missing global rules](../../issues/2026-11-04-context-audit-mistakes-html-comment-stripping-for-rule-loss.md) |
| mitigated | [Isolated HOME does not isolate Claude instruction discovery from host profile](../../issues/2026-11-04-isolated-home-still-loads-host-claude-instructions.md) |
| mitigated | [Isolated Claude worker configuration cannot invoke the requested Recon skill](../../issues/2026-11-04-isolated-worker-recon-plugin-not-loaded.md) |
| mitigated | [Claude MCP discovery retry fails on a retained fixed container name](../../issues/2026-11-04-claude-mcp-discovery-retry-reuses-container-name.md) |

## Related source-gate caveat

The [attribution directory-swap failure](../../issues/2026-10-28-attribution-directory-swap-gate-flake.md) also applies to the pinned old research source. Its current-main fix does not retroactively make an old-source gate green.

## Reorganization gate observation — 2026-11-09

The [shared main lean holder-cap failures](../../issues/2026-11-09-shared-gate-lean-holder-cap-observation.md) were observed after this 104-row snapshot. The source owner's in-flight state is unresolved; this is a separate gate incident, not an experiment outcome.
