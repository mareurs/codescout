---
id: '8d175194a77d679c'
kind: report
status: done
title: ECC BFS identity dogfood — declaration-aware traversal
tags:
- ecc
- codex
- call-graph
topic: ecc-integration
---

# BFS identity dogfood — 2026-11-10

## Current state

Completed and integrated locally on `experiments`: source `537fa820d95b3298393cc515ab104776a57455c7`, integration `dc769e430b7df552733e55693391f9bdf31b4b1b`, stable patch-id `52c9b1d039bbdd7de6183eefca168f32318d8cd8`. Integration tree `115cd5bb196cd91c7763deede84d3c4e93070059` exactly matches the separately gated tree, including four peer Python harness changes. Both required full gates passed: lean6398/default9301 tests passed; Python1827passed,7skipped,554subtests; format, both clippy lanes and doc tests passed with the gate's stated exclusions. Related95/0. [Integration binding](../evidence/2026-11-10-bfs-identity-dogfood/integration.json).

Final15 actual mutations killed15/0survived/0inconclusive; every run restored all ten source hashes in both trees. This follows the independent initial13-run review (8killed/5survived), not a reclassification of its results. Original7 and extended4 native graph cases match. Four lexical checks remove the arbitrary wrong leaf and disclose incomplete binding, with a separate known collector extra edge. Four corrected bugs are archived. Evidence and replay provenance are indexed here and in the committed raw manifest; no repair replay or shared runtime/profile rollout.

## Contract and choices

[Bounded contract](2026-11-10-bfs-identity-contract.md), [plan](../plans/2026-11-10-bfs-identity-dogfood.md). Explicitly used installed ecc-spec-miner and ecc-silent-failure-review bodies. Chosen correction preserves declaration file/line/column through traversal and LSP completed cache, and uses call-site context for AST resolution. Unsupported or ambiguous bindings disclose incomplete analysis; no short-name first-match repair. A warning-only correction and a full cross-language semantic resolver were considered; this choice targets the demonstrated module-qualified case without inventing type/alias inference.

## Frozen evidence and replay

[Baseline](../evidence/2026-11-10-bfs-identity-dogfood/baseline.json) captures12 source/instruction inputs and exact installed skill bodies/hashes. [Pre-action host prefix metadata](../evidence/2026-11-10-bfs-identity-dogfood/session-input-provenance.json) retains the exact local prefix privately before the first assistant message or function call after the current user request. Host prefix is not a serialized provider request and does not prove all history was resident. Prior ECC conditioning, solutions and evaluator material remain excluded/controller-only for a future deliberately symmetric replay. No replay ran.

## Baseline execution

[Module](../evidence/2026-11-10-bfs-identity-dogfood/baseline-module.log):84passed/0failed. [Immutable binary](../evidence/2026-11-10-bfs-identity-dogfood/baseline-binary.json)957538e6, built at unchanged frozen inputs. [Native baseline](../evidence/2026-11-10-bfs-identity-dogfood/baseline-native/process-result.json) completed; raw cases, expected multisets, fixtures and calibration are in the same baseline-native directory. These are fresh baseline executions, not a repair replay.

## Implementation observations

First typed/context implementation passed both new tests but broke3 existing outer-tool checks (83passed/3failed): canonical declaration starts had replaced operational LSP seed query positions. Separating those concepts restored the full86-test related suite. The node now retains provider query coordinates and a separate visited identity; this is an observed implementation correction, not a pre-existing product defect.

A new LSP regression places two helper declarations at different columns of the same file/line and then removes scripted wire replies for a cached repeat. Existing MockLspClient hierarchy scripts can use opaque string item.data to keep display names identical. This tests actual consumer behavior with real parser/cache and a wire-only substitute; it does not claim actual language-server qualification.

## Independent review and bounded fix pass

The [independent review](../evidence/2026-11-10-bfs-identity-dogfood/review/review-result.json) returned **With fixes**: 13 actual production mutations, 8 killed, 5 survived, 0 inconclusive; every frozen source hash restored in both review and mutation trees. Five survivors preceded the new parent regressions. Separate D01 test-fixture diagnostic (not counted among the 13) reproduced display aggregation dropping a second endpoint. Four rustc-valid native lexical-binding fixtures returned wrong clean branches. Filed bugs: lexical `dcecb5f9c3475b38`, display `278ed8254992c41a`, regression gaps `09a013c4d59dfc24`.

Parent added eight regressions after observing the review artifacts. First run: 91 passed/4 failed, but three failures were fixture setup (the replacement LSP provider lacked document symbols), not product evidence. Corrected fixtures produced actual intended RED: 93 passed/2 failed, specifically lexical wrong expansion and shared-callsite missing leaf. The remaining safety tests pass unmutated and will be discriminated by reapplying the observed survivors. The first implementation compile failed for three end-expression iterator borrows; local result bindings address those lifetime errors. All unsuccessful logs are retained. No initial candidate artifact was overwritten.

Raw endpoint-bearing edges now feed next-hop placement after cap admission while display aggregation retains path counts. Rust fallback checks enclosing lexical bindings before module/workspace name lookup, after semantic definition results. Unsupported lexical imports/values/local items/generic types disclose incompleteness; this does not implement full binding inference. Subsequent final related95/0 and all15 actual mutation kills are recorded in the final verification section; the preceding observations describe the unsuccessful and intermediate attempts.

## Initial candidate native observations

The immutable initial candidate `1fa4600c` has all seven expected graph multisets in the retained primary topology: 4 qualifying controls matched, 3 duplicate-depth2 mismatches reduced to 0, all 3 clean renders, scorer calibration 5/5. [Candidate summary](../evidence/2026-11-10-bfs-identity-dogfood/candidate-native/bfs-summary.json) and [native driver exit 0](../evidence/2026-11-10-bfs-identity-dogfood/candidate-native/process-result.json). Four additional concrete Rust cases all match (same-file/same-line type methods, inline modules, same-name seed and child module, unresolved receiver), with 2/2 warning-oracle controls: [extended summary](../evidence/2026-11-10-bfs-identity-dogfood/extended-native-summary.json). These are distinct fixture populations, not exhaustive binding coverage. Independent review has separately found lexical shadowing counterexamples; these successful probes do not invalidate those findings.

## Final candidate and integrated-tree verification

Final executable `45206b13fc56143003cfe1092cbe1aad3af67db4bd56225a65b0899dab54708e`, copied/hashed under the same source-bound lease as compilation. [Final ten-file source](../evidence/2026-11-10-bfs-identity-dogfood/final-source.json), [source gate](../evidence/2026-11-10-bfs-identity-dogfood/final-gate-result.json), [exact integrated-tree gate](../evidence/2026-11-10-bfs-identity-dogfood/integrated-gate-result.json) all retain byte/tree binding. [Final mutations](../evidence/2026-11-10-bfs-identity-dogfood/final-mutations/summary.json):15killed/0survived/0inconclusive. Both native drivers and both extra native jobs exited0. Primary7-case graph/scorer5controls; extended4graph/2warningcontrols; lexical4wrong-target-removal-and-warning/3controls. These are distinct measured populations.

The [lexical native predicate](../evidence/2026-11-10-bfs-identity-dogfood/final-lexical-native-summary.json) does not certify the whole graph: nested-function fixture retains an extra depth1 collector edge, separately open as `0f9e7f9b87799265`. Navigation mismatch `4e7ec7bfc047ac7b` and count instrument `e4b663a4f2145663` remain open. Initial review/candidate data remain historical and unchanged.

## Integration and closure

The six foreign staged blobs matched their initial index records before and after both source integration and the explicit pathspec commit. Only the ten frozen source paths entered integration. The full committed tree matches the independent pre/post gated tree; later documentation has a separate post-staging frontmatter check. Archived corrected bugs: declaration traversal `382466d3e15a3d1d`, lexical placement `dcecb5f9c3475b38`, display frontier `278ed8254992c41a`, regression gaps `09a013c4d59dfc24`. All worktrees, controller inputs and immutable binaries are retained. Local fixture databases, nested Git directories, compiler outputs and server state are excluded from the evidence commit; fixture source and runtime responses remain in JSON with exact hashes.

## Acceptance adjudication

R1/R2: declaration file/line/column survives queue, operational queries, incoming/outgoing hierarchy endpoints and completed cache; actual file/column/query/cache/canonical-cycle mutants are killed. R3/R4: declared ordinary Rust module/type targets preserve both branches; ambiguous, incomplete, attributed-module and unsupported lexical placement are disclosed without inventing a target. Actual ambiguity/language-family/unreadable/attribute/lexical mutants are killed; semantic inference remains bounded. R5: existing direction/cap/parallelism/empty/path-count regressions remain executed, and shared-callsite display paths2 retains both semantic expansions. Full source/integration gates and the scope-qualified native predicates support these bounded requirements. They do not certify every language binding or the separately filed nested-body collector behavior.

## Controller frictions

The first baseline snapshot helper built successfully but assumed worktree/target/debug rather than the leased target, so capture failed. The replacement built/copied/hashed under one source-bound lease; original failed helper/log retained (bug-fix-session-log:F-231). First two new-regression launches were refused by busy named slot0 and executed no tests; a separately retained named-slot2 attempt is the actual TDD run. No kill/failing-test verdict follows from either busy refusal.

## Evidence validation notes

The validator initially assumed every managed document carried a literal frontmatter id; the legacy bug-fix ledger instead has a catalog-derived id verified by doc.get. That exact exception is recorded; all other owned document ids are checked literally. A foreground validation attempt timed out without a verdict; the retained background attempt completed the checks. Whitespace diff checks apply to edited Markdown; raw logs, source snapshots and patches retain original bytes, including carriage returns/context-line whitespace. Local Markdown targets, live old-path negatives paired with archive-form positives, baseline snapshot hashes, binary hashes, raw mutation hashes, ignored0600 private-prefix provenance and all six foreign staged blobs are checked. The selected-evidence privacy predicate has two explicit positive controls and is not an exhaustive scan.

## Staging attribution recovery

The first evidence commit was refused because239 of253 positively owned reviewed paths had stage-log owner `-` / route `not-staging`; no named foreign writer was identified. A one-file control used explicit named restore --staged followed by add and changed attribution to this Codex root while preserving worktree bytes and all six foreign entries. Applying the same operation to the remaining238 unrecorded owned paths left the complete scoped staged patch byte-identical (SHA256d4105024...), all253 then classified mine, foreign entries unchanged. [Control](../evidence/2026-11-10-bfs-identity-dogfood/staging-attribution-control.json), [recovery](../evidence/2026-11-10-bfs-identity-dogfood/staging-attribution-repair.json), [original refusal](../evidence/2026-11-10-bfs-identity-dogfood/evidence-commit-initial-refusal.log). Hooks were retained; no ownership-log edit or wildcard acknowledgment. The original recorder's unreadable staging observation is not causally established. Later evidence metadata additions use real named staging too.

## Boundaries

No push, shared MCP replacement, config/profile changes, dependency installation or repair replay. Preserve all worktrees and private inputs. The separately filed mutation-count instrument bug remains outside these source changes. No general ECC efficacy, exhaustive language support or latency claim.
