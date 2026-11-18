---
id: '1aa221c71a06065f'
kind: report
status: active
title: 'Common source-only preparation: isolated arm inputs and native indexing'
tags:
- effort-steering
- onboarding
- preparation
topic: effort-steering
---

# Common source-only preparation — 2026-11-09

## Current scope

The generic preparation module freezes source, knowledge, global rules and declared CLI/Codescout/plugin/embedding-model/image inputs, then creates independent writable L/R/P/X copies. One real development corpus and four physical copies are now prepared and verified. Actor-model registration, actual four-arm native CLI/context parity, outcome-blind cohort/accounting and paid admission remain open. This is uncommitted development qualification, not an efficacy result.

The pinned source image contains CLAUDE.md but no .codescout; source-only programmatic onboarding ran once and four public documents were copied verbatim into memories. Jina and MiniLM full CPU index attempts timed out; the existing CodeRank GPU service completed one index in441.289s with61603 added chunks. Six memories were re-embedded. No author/repair LLM call or image build/pull; embedding compute cost remains unpriced.

## Local verification

Common preparation TDD and review: initial10 missing-module failures, then10pass. Independent review found four functional path/mode defects;21-method regressions reproduced five expected failing subcases. After corrections21pass. Initial16 applied mutations9killed7survived; survivor-first probes were integrated. Final21 mutations all detected:20behavioral,1diagnostic-only redundant root guard; one overlap mutation also produced a copy-recursion error alongside two assertions. Copies restored.

Latest Docker-enabled protocol suite207discovered:205passed,2explicit native CLI skips,55.971s (protocol-suite-coverage-final.log). This supersedes earlier177/195-test results. Product gate is reported separately below; it does not collect these experiment tests.

## Current whole-product gate

The new cf66 run of ./scripts/gate.sh --check completed with exit0. Its final lane receipts are PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0, FMT_STATE=passed. This supersedes the previous failed gate for this checkout; it does not rewrite the older run. The gate does not collect tools/effort_protocol tests, which were run separately. Printed limits: three heavy Python suites, server-stack, librarian disabled in lean, and two skipped local ONNX weight controls. Full log retained as product-gate.log. No claim covers excluded lanes.

## Embedding-only transport — verification in progress

**Closed as a local native transport control; the historical heading is retained.**


Relay fixes one embedding endpoint/model, strips inbound auth/cookies and validates vectors. Independent actual HTTP review reproduced four defects: [redirect escape](../../../issues/2026-11-09-embedding-relay-redirect-escapes-fixed-upstream.md), [incomplete framing](../../../issues/2026-11-09-embedding-relay-truncated-http-bodies-succeed.md), [partial-client bounds](../../../issues/2026-11-09-embedding-relay-unbounded-partial-clients.md), [oversized-integer disconnect](../../../issues/2026-11-09-embedding-relay-large-integer-disconnect.md). Parent reproduced18-test RED(4failures,1disconnect error), then18/18 GREEN after correction.

Initial13 mutations8killed5survived; five survivor-first probes passed original/restored and killed matching mutants. Final20 mutations:16assertion kills,3runtime-error kills,1 [masked Transfer-Encoding test](../../../issues/2026-11-09-embedding-relay-transfer-encoding-test-masked.md). The corrected test killed that exact survivor with200 !=400; restored18/18. Other19 results remain tied to the preceding test version. Relay SHA16ceb890a5c341f8698251c35d486ab09dc70946ece621f316feddc2850e1839.

Two actual native controls used experiment-owned relay/internal networks; native MCP attached only to its own network. Exact native/proxy/anonymous-volume/network cleanup receipts all complete without errors. The existing GPU service/container/network configuration was not changed.

## Source version and resource observations

The original export stays unchanged at7615f687a70e5fd0b2846c962ba4cbdc177b81e94564046c38dcbd2e24375c94. Onboarding changed only .gitignore among2578 original source paths; the prepared digest is44ad1034c6c570c2f998cfcf48e15f9556cc3f115608dfd9b261bb38bf29d40d. The first failed controller used the old digest in readiness metadata; newer profile controls bind the actual prepared version. No onboarding is repeated to correct host metadata.

Jina native verification after timeout reports incomplete:1614 expected files,79 stored,1535 missing,zero vectorless chunks; six disk memories missing from its store. Four operations returned without MCP errors, which does not establish source-bound canary success. The partial store and failed attempt remain preserved.

Both CPU profiles had32GiB container caps; Jina kept4CPU. A direct2s aggregate CPU tick sample later measured37.878% idle+iowait and the owned MiniLM preparation quota changed4→16CPU; subsequent281.21% usage does not prove a speedup. MiniLM still timed out at1800.0182s. Both exact owned cleanup receipts report complete with no close errors. RAM/OOM is not inferred; DB bytes/row counts are not completeness verdicts. These are preparation failures, not repair-model failures.

## Native capability and coverage interpretation

The preserved first GPU control exited1 at strict readiness despite index exit0: native verify reports1614eligible paths,1612stored and exactly two missing zero-byte fixtures. Raw native verdict remains incomplete, all six memories present,zero orphan/vectorless chunks,Git up_to_date ([issue](../../../issues/2026-11-09-native-index-verify-empty-files-missing.md)). Its reference-canary expectation also named caller wrappers that the actual references display does not print.

A NEW no-index continuation reuses a separate copy of the completed corpus, corrects the source-derived reference callsite witness, and runs all four actual canaries. It reports21advertised tools/four verified operations. Semantic search finds crates/stdx/src/lib.rs; call graph primary result includes both caller wrappers and literal(depth=1,lsp) basis. Onboarding/index/author/repair calls in continuation:zero.

The generic opt-in coverage interpretation verifies actual public source digest before and after, requires the complete unique missing population, rejects noncanonical/outside/linked/nonregular/nonempty paths and all other missing/vector/orphan/memory failures. It returns native_verdict=incomplete separately from coverage=all_nonempty_eligible_files and names/hashes both zero-byte omissions. Raw evidence is not rewritten. Default strict behavior still rejects incomplete.

[Supplemental-guide defect](../../../issues/2026-11-09-readiness-witness-includes-supplemental-guides.md): four positive witnesses could be supplied by a guide; parent RED4assertion failures, fix primary-result selection, independent revert mutation4assertion kills/restored18pass. Empty interpretation review initially11applied mutations7assertion kills4survivors; four survivor-first probes integrated, final exact replay kills all4/restored29pass ([test gaps](../../../issues/2026-11-09-empty-coverage-guard-test-gaps.md)). Earlier7kills remain tied to preceding25-test version. This qualifies selected guards, not universal correctness or repair quality.

## Evidence and open prerequisites

Canonical source/native artifacts are under /home/marius/.cache/codescout-resident/codescout/common-preparation-20261109. The earlier [compact archive](../evidence/common-preparation-20261109/source-and-review.tar.gz) and [manifest](../evidence/common-preparation-20261109/manifest.json) remain unchanged; they predate the relay and final native control. Final preservation is verified in the next section, without overwriting those artifacts.

Prepared development snapshot: common-frozen-v2; manifest SHA927de9c888bf452fe588ca5b46574cf3dbb1e741e928d470f009f181f5efd76d. Curated knowledge contains source configuration/guidance, six memories, index state, code DB and separate memory DB. Initial collector refused an incorrect single-DB assumption; failed directories/log remain preserved ([issue](../../../issues/2026-11-09-common-corpus-collector-single-db-assumption.md)). Versioned retry verifies both stores.

Physical independence scans2346 regular-file paths in EACH frozen/L/R/P/X set, including metadata/rules, with zero shared device/inode pairs. Write controls modify L source/rules/guidance, verify unchanged R/P/X and frozen snapshot, then restore all bytes. This count is a file-path inventory, not source coverage. Actual four-arm native CLI/context delivery is not yet tested; Git runtime and actor model are not registered for those copies.

Remaining: freeze outcome-blind repository-distinct cohort and complete cost ledger; bind transitive native/embedding runtime; qualify actual common context and evaluator controls; run new immutable no-cost preflight. Paid admission remains closed. No new repair-efficacy result.


Documentation verification: CommonMark parser checked315 link destinations in17explicit current documents and260distinct local targets, zero missing target paths; fragments were not assessed. Native audit_doc_refs scanned6main documents,306references all resolved,zero broken/unknown,exit0,not degraded. Its displayed findings are capped at50; these are scanner totals, not a claim that every finding was printed. A final readback after this paragraph is recorded in the QA files.

## Final preserved capsules

[Final manifest](../evidence/common-native-final-20261109/manifest-with-static.json) binds [source/native evidence](../evidence/common-native-final-20261109/source-and-native-evidence.tar.gz):2748152bytes, SHA825e23e3a36cb7514d70b0c029b45e2d0dd64ed281fea38aa04e7fd7d1cbae79,1801regular archive entries, including23Python source files. Every archived regular file was read back and checked by size/mode/SHA; captured originals were rechecked before completion. The preservation command's own log is a zero-byte point-in-time entry captured before its final print, not a completed-run receipt.

Full frozen source/knowledge/rules plus source-only Git seed: /home/marius/.cache/codescout-resident/codescout/common-preparation-20261109/frozen-preparation.tar.gz,186353060bytes,SHA3cef532d9ede62ccf634cdfe97cad38ea68bb8ccf197f8798ac1f2547fad32d6,4887regular entries. Verified by full readback. This capsule remains outside the repo; physical arm copies and original attempts remain available. It is not a self-contained transitive runtime/model-service closure. No source/evidence deletion, merge or push occurred.

The [static-input capsule](../evidence/common-native-final-20261109/static-runtime-inputs.tar.gz) adds prompts.json and README.md:2300bytes,SHA0acfc388a43e18b408e63bed2ab02f2376a8411bfe4944521e3480e94e1bcf80. Combined manifest verifies the union against all25 current top-level regular protocol files:23Python and2static. The earlier report's25Python claim was corrected; the original capsule/manifest remain unchanged ([issue](../../../issues/2026-11-09-source-capsule-omits-static-prompt-inputs.md)).
