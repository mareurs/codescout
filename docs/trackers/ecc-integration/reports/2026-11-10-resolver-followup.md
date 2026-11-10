---
id: '58ae601a4aec9b40'
kind: report
status: active
title: ECC resolver follow-up — documentation and multi-hop identity
tags:
- ecc
- codex
- dogfood
- call-graph
topic: ecc-integration
---

# Resolver follow-up — 2026-11-10

## Current state

Completed and integrated locally on experiments: source `9aee4be48e446c15a441397f877c662f265f56ac`, integration `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`, stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. The resolver strategy comment now describes actual outgoing AST fallback; the subprocess fixture retries only intentional unavailable results. Production store and executable call-graph logic are unchanged. Source and exact integrated-tree full gates passed; independent fixture review approved with3 actual mutations killed/0 survived/0 inconclusive,19-test baseline/restoration (1 ignored). Both corrected bugs are archived.

Fresh source-bound native data separately confirms a multi-hop identity defect: four qualifying controls match; all three duplicate-name depth2 cases omit a branch with a clean render. That behavioral correction remains open and is the next candidate. No repair replay ran.

## Method and replay

Explicitly applied installed `ecc-spec-miner` and `ecc-silent-failure-review` bodies. [Frozen baseline](../evidence/2026-11-10-resolver-followup/baseline.json): `5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3`, eight exact input snapshots, two installed skill hashes and local replay ref. [Private input metadata](../evidence/2026-11-10-resolver-followup/session-input-provenance.json) retains the exact pre-action host prefix privately, with hash/size/mode; no serialized-provider-request or all-history-resident claim. Prior ECC conditioning, evaluator artifacts and solutions must be controlled/excluded for any future comparison. No repair replay ran.

## Inspected boundaries

`resolve_one_hop` routes missing LSP hierarchy to `resolve_via_ts`; its outgoing arm calls `resolve_callees_via_ts`. That helper walks call expressions with the definition file's grammar and can return recoverable errors for unsupported grammar, unreadable/unparseable source or no enclosing function. Existing direct Rust fallback and unsupported-language tests were read and executed within the84-test passing baseline. The resolver source change is limited to /// doc-comment lines. The separately justified fixture extension is described under review and verification.

`bfs` stores visited/current/next symbols as strings. Outgoing AST `callee_identifier` keeps only the final name segment. `CachedResolver::lookup_pos` uses a short-name positions map, first workspace-symbol match, then seed-file/workspace AST fallbacks. These source boundaries explain the fresh native topology failure recorded below; the observations remain bounded to that fixture.

## Verification design

A private native Rust topology calls both `a::helper()` and `b::helper()` from distinct lines in `entry`. Both helpers have different leaf callees. Unique-name topology, seed depth1 and both path-pinned helper depth1 queries are qualifying controls. Cold/restarted/warmed depth2 queries must retain both branches to match the complete expected multiset. Scorer checks actual response header, declared edge/file totals, caller/callee/file/line/depth multisets and absence of warnings; a later guide block cannot supply missing graph edges. Five actual-positive/perturbed controls qualify those specific scorer axes.

## Native observations

[Cases](../evidence/2026-11-10-resolver-followup/bfs-cases.json), [summary](../evidence/2026-11-10-resolver-followup/bfs-summary.json), [fixtures](../evidence/2026-11-10-resolver-followup/bfs-fixtures.json). Unique-name depth2 yields both branches; duplicate seed depth1 and both pinned helper depth1 queries also match exact expected multisets (four qualifying controls). Cold duplicate depth2, restarted depth2 and depth2 after querying both pinned helpers each return only one helper's outgoing branch while rendering no warning (three observed mismatches). Counts remain separate; this is not7/7 correct behavior. [Scorer controls](../evidence/2026-11-10-resolver-followup/bfs-scorer-controls.json) actual-positive, wrong-file, wrong-depth, wrong-callee and guide-only-edge cases each yield their expected verdict (5/5); no general scorer completeness claim.

The source-inspected mechanism is loss of declaration identity between hops: AST targets keep the final name segment, BFS visited/frontiers key strings, and lookup_pos chooses one position by name. Completed-cache definition scoping from the prior cycle remains useful but cannot reconstruct the missing traversal identity. This is separate from the corrected one-hop cache boundary.

[Separate confirmed BFS bug](../../../issues/2026-11-10-call-graph-multihop-collapses-same-name-definitions.md), artifactfed27cae5971c526. Its correction is pending and is outside this candidate's resolver documentation and test-fixture changes.

## Review and verification limits

The [initial independent review](../evidence/2026-11-10-resolver-followup/review/review-result.json) approves the comment against the actual implementation and independently confirms the seven native responses. Three actual scorer mutations were killed,0 survived,0 inconclusive; restored calibration5/5. The file+line omission mutant was caught by wrong-file, so it does not prove independent line-only discrimination. This review predates the fixture extension; no executable product logic was mutated.

Initial full gate was red: lean6383passed/1failed/49skipped; default9287passed/63skipped. Unchanged-source retry was red in both lanes: lean6383passed/1failed/49skipped; default9286passed/1failed/63skipped. The [incident](../../../issues/archive/2026-11-10-observation-store-subprocess-writer-lean-gate-failure.md), artifact255f71914a9174e4, retains child exit101 vs9. Focused cargo test and focused nextest each passed1/0. A diagnostic full lean run with child stdout visible passed6384/6384. None observes the individual original child panic's cause.

The original verification wrapper asserted on its red gate before native.wait(), so native child exit status was not observed. [Native observation status](../evidence/2026-11-10-resolver-followup/native-observation-result.json) records qualified terminal response data without inventing an exit code (F-228).

The store's inspected secure::lock intentionally returns unavailable after100ms; a sibling threaded fixture already accommodates it. Before changing subprocess_producer, the new held-lock child control actually failed with unavailable => unwrap panic => exit101 ([RED](../evidence/2026-11-10-resolver-followup/fixture-red.log):1passed/1failed). The fixture now retries only unavailable within5s per append, leaving other errors fatal. Positive control requires an observed retry before lock release and then8 contiguous records; negative disabled-store control requires failure with no retry marker. The original two-process16-record/sequence/abrupt-exit assertions are unchanged. [First fixture GREEN](../evidence/2026-11-10-resolver-followup/fixture-green.log):19passed/1ignored.

The [final two-file source](../evidence/2026-11-10-resolver-followup/final-source.json) and [patch](../evidence/2026-11-10-resolver-followup/final-candidate.patch) preserve the expanded candidate separately from the initial documentation-only patch. Final source full gate passed: lean6386/49skipped, default9289/63skipped, Python1827/7skipped/554subtests; fmt/clippy/doc-test lanes passed, subject to the gate's printed exclusions. The separate fixture review below records actual mutation execution.

## Fixture review and integration

[Fixture review](../evidence/2026-11-10-resolver-followup/review/fixture-review-result.json) independently verified frozen hashes, unchanged production store and retained original16-record assertions. All three applied fixture mutations were killed: disabled retry fails on unavailable; retry-all-errors leaves a forbidden disabled retry marker; break-after-marker loses records (7vs8 in held-lock control,14vs16 in original). Baseline and restored full fixture target each pass19 tests,1 ignored. This population is separate from the earlier three scorer mutations; production-store mutation population0.

[Exact identities](../evidence/2026-11-10-resolver-followup/integration.json): source `9aee4be48e446c15a441397f877c662f265f56ac`, integration `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`, stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. Owned source hashes match; whole trees differ through peer commits. Six foreign staged blobs were preserved exactly at integration. The [separate integrated-tree gate](../evidence/2026-11-10-resolver-followup/integration-gate-result.json) passed on cb7d13ff3/treed563673b185d884dbbfe215d39cad286d32dada0: lean6387/49skipped, default9290/63skipped, Python1827/7skipped/554subtests; fmt/clippy/doc-tests passed, subject to printed exclusions. Later experiments commits are not silently included in that exact-tree claim. All worktrees and evidence remain retained.

## Evidence provenance correction

A parent raw-byte hash check found the first fixture log digest differed from the review record. Review had hashed read_text(), which normalized24 carriage returns. The [correction](../evidence/2026-11-10-resolver-followup/review/fixture-log-hash-correction.json) preserves the normalized digest and records actual retained raw-byte hashes for all three logs. No log, source, outcome or verdict changed; no mutation reran. Captured as bug-fix-session-log:F-230. Final manifest hashes raw bytes.

## Instrument count boundary

The reviewer reported final top-level cargo summaries and actual failed assertions. In the first mutation log, mutation-probe instead prints21 test(s) ran by summing parent and captured child running N tests lines, including an ignored selection. [Separate instrument bug](../../../issues/2026-11-10-mutation-probe-counts-nested-and-ignored-tests-as-executed.md), artifacte4b663a4f2145663. This is a count-population defect; no false mutation verdict is established here. Its source correction is deferred.

## Evidence finalization

Both fixed bugs were archived through the catalog with reminted identities; live path/id citations were updated, while raw historical captures remain unchanged. [Managed-document validation](../evidence/2026-11-10-resolver-followup/validation-result.json) checks explicit owned IDs/local targets, preserved foreign staged blobs, ignored private-prefix hash/size/mode and five calibrated privacy patterns. It is a selected-pattern check, not universal secret detection. [Post-staging issue frontmatter](../evidence/2026-11-10-resolver-followup/issue-frontmatter.log) passes18/0. The raw-byte manifest binds retained files; no self-referential evidence-commit SHA is written into it.

## Boundaries

No traversal redesign. Production observation-store implementation is byte-identical to the frozen base. Shared MCP remains older build9e4db9b9 as observed this cycle and is not the current-source execution oracle. All fixture/catalog/XDG/binary state is private. No repair replay, profile changes, push, shared runtime replacement or causal ECC effectiveness claim.
