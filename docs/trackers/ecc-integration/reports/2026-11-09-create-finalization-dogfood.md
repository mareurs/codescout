---
id: '2ba861100ce80249'
kind: report
status: active
title: ECC create finalization dogfood — atomic catalog failures
tags:
- ecc
- codex
- dogfood
- librarian
topic: ecc-integration
---

# Create finalization dogfood — 2026-11-09

## Current state

Completed and integrated locally. One confirmed current-source defect was corrected: catalog-stage create failures left earlier catalog effects. [Bug](../../../issues/archive/2026-11-09-doc-create-leaves-partial-artifacts-on-finalization-failure.md), [plan](../plans/2026-11-09-create-finalization-dogfood.md), [bounded observed draft](2026-11-09-create-finalization-observed.md).

## Scope and method

Explicit use of adapted ecc-spec-miner and ecc-silent-failure-review bodies to continue the user-authorized dogfood workflow. This is one controller-assisted improvement case, not a fresh independent skill arm, no-skill repair replay, or causal ECC comparison. Scope: create::call's row/slug/augmentation/registration/final disk write, direct enforcement helpers and their tests. The prior disk-diagnostic fix is preserved.

## Frozen baseline and future replay

Commit038fffb3447b55900e617a251b6472486259e07c/tree4a0f76c8e8a0068974c2ba95afc872c309a43435, branch feat/ecc-create-finalization-20261109 in the owned .worktrees/ecc-navigation-gate checkout. [Baseline capture](../evidence/2026-11-09-create-finalization-dogfood/baseline.json) records ten exact source/instruction/lockfile blob IDs and SHA256 hashes, skill hashes/bodies, neutral candidate brief, source cleanliness, host-runtime caveat and base tree. Controller-only skill bodies and new solutions/evaluators must be excluded from future candidate input. Existing Git commit remains the full repository baseline, not a claim that those ten files are its whole contents. No replay ran.


[Session input provenance](../evidence/2026-11-09-create-finalization-dogfood/session-input-provenance.json) retains an exact pre-action local host-transcript prefix privately (17,214,502 bytes, SHA256479ce8dcf44397380f43da17de6287fa7fcdb59e5edd820e2623362e681ed05d). This is host history, not a byte-exact serialized provider request or a promise that all history was resident in model context. Configured gpt-6.1-sol/high is recorded, not backend-attested. Personal policy/earlier conversation remain in the ignored600-mode snapshot only. A future skill-free condition must explicitly control prior ECC conditioning and exclude solution/evaluator material; none ran now.

## Reproduction and correction

Existing module baseline48/0. Three new tests reach actual injected SQLite errors during slug mint, augmentation insert and worktree registration, then fail the residual artifact assertion:2 rows vs the single unrelated keeper expected. Existing48 remain green. [RED result](../evidence/2026-11-09-create-finalization-dogfood/red-result.json), [test-only delta](../evidence/2026-11-09-create-finalization-dogfood/red-tests.patch).

The correction wraps catalog row+slug+augmentation+worktree registration in one transaction, commits before directories/files, and removes registration after disk writing. Any failure in that catalog phase rolls back its earlier writes. Existing disk-error cleanup/message and original IO source remain unchanged. This is catalog-phase atomicity, not cross-filesystem/database crash atomicity. The sibling worktree::resolve_write_target already demonstrates the same transaction pattern.

[GREEN result](../evidence/2026-11-09-create-finalization-dogfood/green-result.json):51/0, including all three new failure+retry controls and existing disk-cleanup tests. Green source before formatting7b27f5043607efa05ab32134e9a02103a6fcebf966f37ec3c7814ec7297ff24d; formatted frozen candidate68f4fdaa9030bd0bd78939ee7debb37c6dd5b30917db07e43278505fee27d3bd. Gate/build and private runtime bind the formatted source separately.

## Verification and review

Final source SHA256 `a6febbcb5bada021e0384cfeda9ccd4e527e840c35c9639ed9d043389af27e49`; production prefix is byte-identical to the independently reviewed candidate. [Final source binding](../evidence/2026-11-09-create-finalization-dogfood/final-source.json).

[Independent stronger-model review](../evidence/2026-11-09-create-finalization-dogfood/review/review.md) applied exactly7 initial mutations:6 KILLED,1 SURVIVED,0 inconclusive. M7 moved registration before the transaction and passed51/0. A diagnostic enabled worktree context in the two downstream failure fixtures and observed49/2, both residual registration1 vs expected0. Only after observing that failure did the controller expand both tests across regular/worktree contexts. [Exact M7 recheck](../evidence/2026-11-09-create-finalization-dogfood/final-M7-result.json):49/2, KILLED; [restored final module](../evidence/2026-11-09-create-finalization-dogfood/final-module-result.json):51/0. This closes the Important test gap; it does not claim all7 were rerun on the final tests. No production defect was identified by review. Tracker bug-fix-session-log:F-223 records the gap.

[Final full gate](../evidence/2026-11-09-create-finalization-dogfood/final-gate-result.json) passed16:15:27–16:20:39UTC with source unchanged: FMT/CLIPPY/LEAN/DEFAULT all0; lean6316 passed/48 skipped, default9208 passed/62 skipped, Python1764 passed/1 skipped/554 subtests. The lean lane omits librarian; default plus the dedicated51-test module cover this correction. Heavy Python and other gate exclusions remain as printed in the retained log.

[Final immutable executable](../evidence/2026-11-09-create-finalization-dogfood/final-binary.json) SHA256 `430e26890775b3c030cdd24c3dab739420fc8af9e587f6e182b89a34fe16b06d`. [Actual private stdio MCP](../evidence/2026-11-09-create-finalization-dogfood/final-mcp-results.json) passed5 fault/retry/read cases: slug and augmentation in regular and linked Git worktrees, registration in a linked worktree. Every injected failure carried its SQLite marker, left counts unchanged and created no target parent directory; removing only the trigger allowed persistence and full doc(get) read. Private catalogs/XDG state only, no model calls.

Review retained an initial restored-control50/1 false red: an outer held build lease spanned source roots and Cargo reused M6's mutant artifact despite original source bytes. Clean rebuilt control51/0 is retained beside that failure. All initial mutants compiled their armed source. Parent final recheck acquires with-slot INSIDE mutation-probe's isolated source root and reacquires for the restored candidate. This mitigation is bug-fix-session-log:F-224; no shared cache implementation change is claimed. Initial GREEN, initial3-case MCP and first frozen gate remain under their original filenames.

## Integration

Source commit `6377002c1111fc05c7fb708f68341c0f1293e271`; experiments integration `6b2befb1eb9344e7a37f1fe660ec7de501276028`; stable patch-id `ba636938caee459b5457133127e8d919038a6758`. [Integration record](../evidence/2026-11-09-create-finalization-dogfood/integration.json) verifies identical full committed trees `74f4462b41ee95e06cff10a168fc12ea5eae6f1e`, the gated source hash, source-only commit path sets, all5 runtime cases and unchanged foreign staged blobs for six peer-owned paths. No integration-source delta required another gate. Bug is fixed and archived as44160e1378b2e3f5; worktrees/evidence retained for future replay. Minor tool-grammar refusals and corrected readback are recorded as bug-fix-session-log:F-225.

## Evidence finalization

The post-staging document gate passed18/0 on the main checkout ([log](../evidence/2026-11-09-create-finalization-dogfood/document-gate.log)). The scoped validator parses YAML identities for the12 managed documents, checks their local Markdown targets and the four new ledger entries, preserves six foreign index blobs, and verifies private history is ignored/untracked. [Validation result](../evidence/2026-11-09-create-finalization-dogfood/validation-result.json), [raw evidence manifest](../evidence/2026-11-09-create-finalization-dogfood/manifest.json).

The initial validator assumed an unquoted id line and rejected a valid YAML-quoted identifier. [Failed-check record](../evidence/2026-11-09-create-finalization-dogfood/validation-initial-failure.json) is retained; validation now compares parsed YAML values. Privacy checks use calibrated selected credential/private-policy patterns and exclude the scanner's deliberate positive literals; they are a bounded check, not a proof against every possible secret. Only public provenance is committed; the exact private host transcript and executable snapshots remain ignored.

## Incidental navigation limit

A shared running older MCP returned unrelated outgoing chains for call_graph(call,src/librarian/tools/create.rs). Its build9e4db9b9 differs from current source. That graph was excluded; source symbols and exact references established the inspected chain. [Follow-up bug](../../../issues/2026-11-09-call-graph-generic-call-returns-unrelated-callees.md), [exact response](../evidence/2026-11-09-create-finalization-dogfood/incidental-navigation.txt). The earlier unique-Python-symbol routing result does not establish duplicated-Rust-name correctness. Fresh current-source reproduction and cause remain open.

## Boundaries

No shared catalog fault injection, profile/config changes, release build, shared-server rollout or push. The default pilot remains two optional skills. No efficacy or performance improvement is inferred from one dogfood case.
