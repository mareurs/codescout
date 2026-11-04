---
id: '5faa9c6e3fd2a6f9'
kind: bug
status: fixed
title: Native bootstrap says semantic index is not built while status is queryable
tags:
- cluster/unclassified
- effort-steering
closed: 2026-11-04
opened: 2026-11-04
owner: marius
severity: medium
unverified: 'Real sqlite-vec caveat discharged by four actual native cells on 2026-11-04. Remaining: the tested binary self-reports clean c98859b5e (Git history contains a8ee4db68), but that metadata and ancestry do not prove the build''s source tree; no shared live server restart/deployment was verified.'
---

# BUG: Native bootstrap disagrees with a queryable semantic index

## Summary
One actual MCP response reports indexed=true, queryable=true and Git up_to_date, while its appended project-status instruction says Semantic index: Not built and directs the agent to build again.

## Class fit
This is a contradiction between two model-facing readiness surfaces in the same response. It is recorded as unclassified because the source/backend selection responsible has not yet been traced; a stale heading alone does not establish the cause.

## Symptom (Effect)
A worker receives an instruction to repeat preparation despite a usable populated store. The actual search in the same session returns substantive bindgen implementation content. No measured worker followed the incorrect instruction, so repeated cost or low-effort failure is not claimed.

## Reproduction
S=/home/marius/.cache/codescout-resident/codescout/native-env-admission-20261104/semantic-minimal-20261104. query-bindgen-git-resumed-project/mcp/3-response.json contains both statements. 4-response.json independently reports Git up_to_date and 745 stored files; 5-response.json returns source results. The raw coverage verdict remains incomplete because four whitespace-only eligible files have no chunks; that separate issue is cc61bf08099dac2d.

## Environment
Private pinned Codescout binary58c33c3064c3e08b19ff26a2b84d96f447746a09ca1547ad4867445a77dd8028, local Jina, sparse disabled, sqlite-vec. Guest project/testbed, clean baseline898002898161832a7c718527d8a75bf8b73428a7. This is not a verified statement about the current product HEAD or all backends.

## Root cause

Agent::project_status checks only .codescout/embeddings/project.db existence (legacy path), and prompt rendering maps that boolean to Built/Not built. Actual IndexStatus uses RetrievalClient::from_env and project_index_stats for the selected backend. Current V and the binary's recorded base commit have the same legacy-path check; actual four-state controls establish this behavior independently on the dirty pinned package. Neither Git synchronization nor another full index repairs the bootstrap predicate. This describes the original defect; the product correction is recorded below.

## Evidence

Four actual native controls in worker-context-verification-20261104/readiness-results.json cross empty/populated sqlite-vec store with absent/present ZERO-BYTE legacy project.db. Results: empty/no marker -> indexedfalse, bootstrapNotbuilt,0results; empty/marker -> indexedfalse, bootstrapBuilt,0results; populated/no marker -> indexedtrue/queryabletrue, bootstrapNotbuilt,3returnedresults; populated/marker -> indexedtrue/queryabletrue, bootstrapBuilt,3returnedresults. This establishes the actual predicate mismatch on the pinned package, including false positive and false negative. Full wire/launch/cleanup retained. Source inspection in current V and the binary's base commit locates the legacy existence predicate; dirty binary source identity is not assumed.

## Hypotheses tried
The observed Git-baseline/no-op-index control did not remove the contradiction. No blanket stale-state cause is inferred.

## Fix

Applied 2026-11-04 in commit `a8ee4db68`. `Agent::project_status` no longer tests whether the legacy `.codescout/embeddings/project.db` exists. It calls `crate::tools::config::check_has_index_cached` (now `pub(crate)`), the bounded and cached `project_has_chunks` probe that `activate` already uses. The bootstrap block and `index(action="status")` therefore read the same store.

Not changed: the coverage verdict stays separate (the four whitespace-only files, `cc61bf08099dac2d`). The first probe per session waits at most `FIRST_PROBE_TIMEOUT` (2 s). On a timeout it reports "Not built" and does not cache that answer, as `activate` already does.

## Fix provenance

- **SHA:** `a8ee4db68d55bd26f0007f98ef1584b3934b1112` (`experiments`)
- **patch-id:** `872fd87c7b8d9f328863f0a8b0b8af1467abeb72`

The commit was local when written, not pushed. `./scripts/gate.sh` was green on the tree before the commit: FMT, CLIPPY, LEAN and DEFAULT exited 0, and 8876 tests passed.

## Tests added

Two product tests in `src/tools/config/tests.rs`, written before the fix and seen to fail for the stated reason (`has_index` wrong at the assertion):

- `bootstrap_reports_a_populated_store_as_built_without_the_legacy_file` (false negative cell).
- `bootstrap_reports_an_empty_store_as_not_built_despite_a_legacy_file` (false positive cell).

They seed the store's answer through the shared `index_status_put` cache, which is the value `check_has_index_cached` returns. They do not build a real sqlite-vec store. The two cells with matching store and marker were not added: the old code passed them already. The native four-state controls above remain the check on a real store.

## Native binary recheck — 2026-11-04

Independent follow-up ran the actual four native cells on a stable private copy of target/release/codescout, without seeding the answer cache, rebuilding the tiny source index, invoking a provider model or changing the shared release file.

| Selected sqlite-vec store | Zero-byte legacy project.db | Actual indexed / queryable | Bootstrap | Returned query results |
|---|---|---|---|---|
| Empty | absent | false / unspecified | Not built | 0 |
| Empty | present | false / unspecified | Not built | 0 |
| Populated | absent | true / true | Built | 3, capped |
| Populated | present | true / true | Built | 3, capped |

All four cells agree. The two discriminating cells changed from the preserved old-binary result: populated/no marker now says Built; empty/marker now says Not built. Each cell uses a fresh native process and fresh source/config clone with the same local Jina/sqlite-vec setup. Source/config byte populations were independently checked, and all four exact owned container names inspect absent after cleanup. This is actual backend behavior, not a seeded-cache unit test or a quality trial.

Binary identity: original stable copied bytes SHA2560b9bcb54e633224d4b04073f96e045aefa38469c6043edbae00818fa60205ef6,65680152bytes, original mtime2026-11-04T15:13:29.326570Z. The version command reports0.15.0, clean commitc98859b5e27229b8d230a18600bf9191a6f8a3b0. Git ancestry independently confirms that commit contains a8ee4db68d55bd26f0007f98ef1584b3934b1112. This metadata is not a reproducible source-to-binary proof.

For the old calibration image, ONLY the private copied ELF interpreter/RPATH were adapted using the previously qualified package procedure; packaged SHA256c4bf914175758c41c7f13a77712b2cf43feddb9916fcb5db1a8510b967e2e332. Runtime index status independently identifies this reading build id and c98859b5e/clean while retaining the original index writer's provenance. The shared release binary/runtime/server was not modified or restarted.

[Complete native recheck evidence](../research/data/2026-11-04-native-readiness-recheck.tar.gz): SHA2563b2e57109e7957cd4ef921828dc5df3dedca7c165307bc4748cc29ed168390ac,38042bytes,72archive entries. Every file hash/link target was verified by rereading the archive. Includes full native responses/wire/launch/stderr/cleanup, exact tiny source/config inputs, original comparator result, private diagnostic/helper, independent checks and binary identity. Heavy ELF/model/DB payloads remain privately retained. Root: /home/marius/.cache/codescout-resident/codescout/native-readiness-recheck-20261104.

The prior real-store caveat is discharged for this hash-pinned package. No claim of source-build reproducibility, deployment to an existing shared server, all backends/cold-start timing, full task admission or low/xhigh quality follows. No product source change or repeated full gate was performed by this verifier; the implementer's gate/test report remains separately attributed.

## Workarounds
Read actual status, verify and query results; do not schedule a paid comparison from the banner or blindly rebuild from it.

## Resume

Four native real-store cells now pass on the identified copied binary. Retain source/build and live-server deployment limits separately. Before using a new binary in a paid campaign, freeze and qualify that exact package/common worker configuration; keep coverage checks and the remaining authority/task/oracle gates separate. Do not repeat onboarding or index building to repair the old legacy marker predicate.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
