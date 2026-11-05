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
unverified: Controlled root rebuild from e6f07e9a6 is now observed with unchanged source manifests and identical tested ELF hash 8f567f170b07de559b0dfc89e0250d41bd80f9e52c0e650b1b77a795f1454dea. Existing dependency/compiler caches were reused, not an independent hermetic rebuild. Live reload was accepted but this active turn still answers from old PID1745637; verify the next active turn before declaring deployment complete.
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

## Controlled build and live reload — 2026-11-04

Evidence archive: [native build provenance](../research/data/2026-11-04-native-build-provenance.tar.gz), SHA7b9cc96c195aa38f1f6ae3598a09df83a59cb67a738fbe9adebac271f0ce12ad,5889757bytes/72verifiedentries. Contains source snapshot, manifests, gate/build/forced-root logs, actual native responses and reload receipts; excludes ELF/model/database/profile payloads.

Ran `./scripts/gate.sh --check`: Python-light, formatter, clippy, lean/default nextest and doctest lanes all exit 0. Nextest summaries: 6012/6012 lean and 8876/8876 default, with 48/62 skips respectively. The gate does not exercise every server-stack configuration.

Pinned HEAD `e6f07e9a6337079d25ab6db7b62f46286047f2e5` includes fix `a8ee4db68`. Inventoried all 527 tracked files in the recorded build scopes before/after; scopes clean, HEAD stable, and archived source entries independently match their hashes. The first `rb.sh --locked --offline -v` returned `Fresh`; it did not establish a compilation. Then invalidated only `build.rs` mtime, observed both root library and binary rustc invocations, and checked source bytes/HEAD unchanged again. Rebuilt raw ELF SHA `8f567f170b07de559b0dfc89e0250d41bd80f9e52c0e650b1b77a795f1454dea`, 65680152 bytes, byte-identical to the first artifact. Rust/cargo 1.97.1; feature set includes server-stack and default local-embed. This is controlled root rebuild evidence with existing compiler/dependency caches, not an independent hermetic dependency rebuild or a claim of general bit reproducibility.

The hash-identical tested private copy, adapted only for guest interpreter/RPATH, SHA `cdc53728d28a659b5a697048e6e5b719149693df2cf57cb8554ee0751ebf51bb`, passed all four actual sqlite-vec cells: empty +/- zero-byte marker gives Not built and zero results; populated +/- marker gives Built and three capped results. All sixteen source/config instances match; all four exact owned names independently absent. The initial cleanup verifier capitalization false negative is separately recorded and corrected as issue `3e9647481c8c7d1b`.

Actual Codex app-server API `config/mcpServer/reload` accepted `{}` for the running harness. A fresh workspace call during this active turn still reports PID1745637, old dirty `0eb6190ec`, deleted executable, build_id439eb06e. Therefore live deployment is PENDING the next active-turn refresh; no process was killed and no separate probe was mislabeled as the live server. The endpoint has no per-thread selector and queues refresh for loaded threads; no profile/config files were changed. See [official app-server documentation](https://learn.chatgpt.com/docs/app-server).

Receipts and source snapshot: `/home/marius/.cache/codescout-resident/codescout/native-build-provenance-20261104`. No new model/onboarding calls or paid worker admission.

## Claude-side `/mcp` reconnect check — 2026-11-05

Read from tool output in the Claude session `48d211eb` after `/mcp`:

- `index(action="status")` reports `reading_binary_sha: 538068636`, `reading_binary_dirty: false`. `git rev-parse HEAD` gave `538068636222134f2d5de8bff1c498400ca4104e`.
- `target/release/codescout` has mtime 2026-11-05 09:20:21 +0200. HEAD was committed at 08:58:05 +0200.
- `git merge-base --is-ancestor` says `a8ee4db68` (this fix) is an ancestor of `538068636`.
- The semantic store is queryable: 108513 chunks, 3254 files. The bootstrap block says `Built`.

What this does not show: this project has both the legacy `.codescout/embeddings/project.db` and a populated store. The old code would also say `Built` here. So the live server is not tested on the two cells that differ (populated store without the legacy file, empty store with the legacy file). Only the copied binary was tested on those cells (see the sections above). The `unverified` caveat stays.

The semantic index was 15 commits behind HEAD (`last_indexed_commit: 89b7e151`). No `index(action="build")` was run.

## Workarounds
Read actual status, verify and query results; do not schedule a paid comparison from the banner or blindly rebuild from it.

## Resume

Controlled root rebuild and four real-store cells are verified for raw hash8f567f170b07de559b0dfc89e0250d41bd80f9e52c0e650b1b77a795f1454dea. Reload is accepted but not yet applied in this active turn. At the next active turn, query the actual workspace server and independently hash `/proc/<reported pid>/exe`; require the verified hash, expected source SHA and exe_deleted=false before declaring live deployment. Before paid work, still freeze/qualify the exact common worker package, coverage, authority and task/oracle boundaries. Do not repeat onboarding/index building.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
