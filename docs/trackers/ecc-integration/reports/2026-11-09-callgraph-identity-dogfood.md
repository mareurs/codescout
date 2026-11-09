---
id: b7951b533f8ee457
kind: report
status: active
title: ECC call graph identity dogfood — current-source investigation
tags:
- ecc
- codex
- dogfood
- call-graph
topic: ecc-integration
---

# Call graph identity dogfood — 2026-11-09

## Current state

Completed and integrated locally as `4776e44bd49347da8b7c962f72bbed44b6f2fe99` on experiments, source `555f2817cbb790bcbd5de118e6194f45bbbdc15b`, stable patch-id `82470b4a164f780eab0ce409ca01dfafc29f0d7a`. Warm-cache cross-definition contamination is corrected with a completed-resolution store keyed by definition identity and direction. [Fixed bug](../../../issues/archive/2026-11-09-call-graph-generic-call-returns-unrelated-callees.md). Both the final isolated candidate and exact committed integration tree passed full gates.

## Scope and method

Explicit use of the adapted ecc-spec-miner and ecc-silent-failure-review bodies alongside systematic debugging. CachedResolver::one_hop previously read raw EdgeCache rows by project+symbol before definition lookup. Source inspection plus independent cold/warm controls and wrong-adjacency Rust REDs establish that accumulated call-site edges were treated as completed adjacency for another definition. The correction resolves identity first and uses an additive completed-resolution store keyed by project, symbol, definition file+line+column and direction. Raw edges remain separate; legacy rows are never promoted into trusted completed snapshots.

## Frozen baseline and replay

[Baseline](../evidence/2026-11-09-callgraph-identity-dogfood/baseline.json): commit69ffdd12e6d51f4884c0190a1a6c4edeca4289e4/tree f1c0f44c76cc02ec677466b6b73320cb989d1e34; ref refs/ecc-replay/callgraph-identity-20261109; eleven exact Git/blob/SHA snapshots and a pre-fix neutral brief. The Git tree is the full baseline; the eleven snapshots are the inspected input subset. [Private input provenance](../evidence/2026-11-09-callgraph-identity-dogfood/session-input-provenance.json) retains exact local pre-action host history privately; it is not a byte-exact provider request or proof all history was resident. Earlier ECC conditioning and solutions need explicit exclusion/control before any future skill-free replay. No replay ran.

## Verification
Instrument limits retained: the first new-API attempt timed out without a terminal verdict. A later final-runner edit attempted to append a nonexistent file and was refused; the premature background launch exited because the script was absent (job98), before any check ran. Creating the file through create_file corrected the launch, and job99 completed the final source-bound checks. Refused/missing-script calls are not test failures or mutation kills.

Existing related module77/0. Two consumer regressions observed RED77pass/2fail at wrong returned adjacency: leaf_a vs leaf_b and partial [leaf_a] vs full [leaf_a,other_a]. Separate new-API test compile-red records missing key/method symbols; its first30-second attempt timed out without a terminal verdict and is retained as an instrument limit. [Observed contract](2026-11-09-callgraph-identity-observed.md), [plan/design](../plans/2026-11-09-callgraph-identity-dogfood.md).

Baseline actual MCP has6 records: cold A and B match, warm B and restarted B return A's edges; the incoming control returns zero and is not used to establish a second runtime defect. Its subsequent outgoing result matches the complete fixture but does not test a populated incoming cache. Candidate core/legacy probes are separate; no combined pass-count hides the nonqualifying control.

## Candidate verification — frozen source

[Candidate source](../evidence/2026-11-09-callgraph-identity-dogfood/candidate-source.json) and immutable executable are bound separately. [Qualified native controls](../evidence/2026-11-09-callgraph-identity-dogfood/candidate-mcp-summary.json):6/6 match complete caller/callee/file/depth tuples and declared totals. Cases: cold A, warm B after A, restart B, cold B, quiet empty leaf, and B on the retained baseline's populated legacy cache. [Legacy rows](../evidence/2026-11-09-callgraph-identity-dogfood/candidate-legacy-cache.json) retain A's prior projection and show the new exact B resolution without importing the projection as trusted adjacency. No empty leaf snapshot was stored.

[Scorer controls](../evidence/2026-11-09-callgraph-identity-dogfood/candidate-scorer-controls.json): actual cold-A positive plus wrong-file, wrong-callee, wrong-depth and guide-only-edge perturbations all yield their expected verdicts (5/5). This is validation of those concrete controls, not a general scorer correctness rate. Original6-record baseline probe remains separate, including its nonqualifying incoming control. The initial frozen-source full gate passed. The final test-strengthened source also passed a separate full gate; results and integration are recorded below.

## Independent review and observed survivor fix

[Independent review](../evidence/2026-11-09-callgraph-identity-dogfood/review/review-result.json) ran83-test baseline/restored controls and ten actual applied mutations:8 killed,2 survived,0 inconclusive. Both survivors force a coordinate field to zero at the consumer; direct cache API tests did not guard consumer forwarding. After observing those survivors, added one same-file consumer regression covering different lines and different columns. [Exact line recheck](../evidence/2026-11-09-callgraph-identity-dogfood/final-M02-line-result.json) and [column recheck](../evidence/2026-11-09-callgraph-identity-dogfood/final-M03-column-result.json) each fail at wrong-definition adjacency (83pass/1fail). Restored final module84/0. These two later kills are separate from the initial ten-mutation population; see bug-fix-session-log:F-226.

[Final source](../evidence/2026-11-09-callgraph-identity-dogfood/final-source.json) records unchanged production prefixes and the test-only strengthening. [Final executable](../evidence/2026-11-09-callgraph-identity-dogfood/final-binary.json) is byte-identical to the qualified candidate executable (SHA256 d1c064c219c2ad468adf8e5b4644f7dfa45900ae7a5ed711c8601784f41fd944). The native6/6 and scorer5/5 results therefore bind to that same executable; they were not rerun after the test-only change.

Review also found [pre-existing contradictory resolver documentation](../../../issues/2026-11-09-call-graph-resolver-doc-denies-outgoing-ts-fallback.md), open and nonblocking. It is outside this source patch.

## Final candidate gate

[Source-bound final gate](../evidence/2026-11-09-callgraph-identity-dogfood/final-gate-result.json) passed, source hashes unchanged: Python1764passed/1skipped/554subtests; lean6323passed/48skipped; default9215passed/62skipped; doc tests0passed/2ignored, embed doc tests0. Fmt and both clippy feature configurations passed. Gate prints its coverage limits: heavyweight Python/server-stack and full ONNX inference are not covered. Focused restored module84/0 is separate from these overlapping whole-suite counts; counts are not added together.

The source worktree remains at the frozen base plus only the two changed files. Integration includes peer Rust/prompt/census changes after that base, so equality is asserted for the two owned source files and stable patch-id, not for the complete candidate/integration trees. A separate committed-tree integration gate records that distinction.

## Verified integration

[Integration identities](../evidence/2026-11-09-callgraph-identity-dogfood/integration.json) bind the exact same two owned source hashes and stable patch-id across source/integration. Whole trees differ because peer source changes landed. The [separate integrated-tree gate](../evidence/2026-11-09-callgraph-identity-dogfood/integration-gate-result.json) passed on4776e44bd/tree7ddf5acae4c8193bbe13313fc68c711ea6803439: lean6325passed/48skipped; default9222passed/62skipped; Python1765passed/1skipped/554subtests. Fmt/clippy/doc-test lanes passed with printed coverage exclusions. Later peer commits on experiments are not silently included in this exact-tree claim.

The six pre-existing foreign staged blobs were preserved exactly at source integration/provenance capture; peer dirty files remain outside the owned commits. Fixed bug archived through the catalog as e5016b19fb0f75da. Open pre-existing resolver-doc issue remains separate. Baseline ref, eleven input snapshots, exact private host prefix and all review worktrees/evidence remain available for a future explicitly designed replay; none ran.

## Evidence finalization

[Owned document list](../evidence/2026-11-09-callgraph-identity-dogfood/validation-docs.json) scopes validation to15 managed documents and the shared session ledger. The validator checks YAML identity, local Markdown link targets, the fixed bug's provenance, the exact foreign staged blobs, ignored/untracked private input and five calibrated credential/private-policy patterns. This selected-pattern scan is not universal secret detection. [Validation result](../evidence/2026-11-09-callgraph-identity-dogfood/validation-result.json), [owned paths](../evidence/2026-11-09-callgraph-identity-dogfood/owned-paths.json), [raw evidence hash manifest](../evidence/2026-11-09-callgraph-identity-dogfood/manifest.json). Raw review/baseline captures retain their original pre-archive identities as historical evidence; live document links use the archived bug identity.

Post-staging issue_frontmatter suite passed18/0, including tracked active/archive issue scans; [log](../evidence/2026-11-09-callgraph-identity-dogfood/issue-frontmatter.log). The owned managed-document diff passes git diff --check. A broader check including raw logs/.patch files reports retained whitespace (including patch context lines); those immutable captures are preserved rather than normalized.

The first D+A staging succeeded. A later repeat of the complete path list refused the already-staged deleted old path; restaging now filters existing files while preserving that deletion in the commit scope (bug-fix-session-log:F-227). No reset or unstage was used.

## Boundaries

Shared MCP remains older build9e4db9b9 and is not the current-source runtime oracle. All runtime fixture/catalog/cache/XDG state is private. This is one controller-assisted dogfood case, not an independent efficacy A/B or cost comparison. No shared-server/config/profile changes or push.
