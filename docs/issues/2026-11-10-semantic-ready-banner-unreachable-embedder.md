---
id: db67deedbae5a09e
kind: bug
status: fixed
title: 'BUG: semantic index banner promises ready search when the embedder is unreachable'
tags:
- cluster/unclassified
- prompts
- readiness
topic: effort-steering
closed: 2026-11-11
opened: 2026-11-10
owner: marius
severity: low
unverified: 'The original counterexample (pinned native binary, network none, unreachable embedder) was NOT re-run against the fixed binary. Verified only: the renderer strings, unit tests on the rendered text, the full gate for the banner commit, and for the onboarding commit a gate whose one failure came from another session''s uncommitted guide rewrite (that commit''s bytes passed on a clean tree of HEAD plus its five files).'
---

# BUG: complete stored index is described as operational semantic search

## Summary

The project status banner says semantic_search is ready whenever has_index is true. A complete vector store does not establish that query embeddings can be produced in the current runtime.

## Class fit

Unclassified readiness overclaim in prompt rendering. Unlike the older legacy project.db marker bug, the store is genuinely complete here; the missing prerequisite is access to the embedding provider at query time.

## Symptom (Effect)

One actual native Obscura response contains both an error payload (ok=false, embedding server unreachable at source-embeddings) and a supplemental banner saying Semantic index Built, semantic_search ready to use. MCP result.isError is false, so checking only that envelope flag would also miss the failed query.

## Reproduction

The prepared CodeRank index verifies complete:111expected/stored files,5disk/store memories,0missing/orphans/vectorless,Git up to date. Query it through the existing actor-style Docker network none with a fresh native process. semantic_search cannot reach the private embedding relay. No model or reindex call is used in this counterexample.

## Root cause

Current main build_project_status_segments in src/prompts/mod.rs selects the operational wording using only status.has_index. Provider reachability is not represented by that predicate. The protocol's existing source-result canaries remain required; storage status alone must not admit an actor.

## Evidence

common-obscura-421-v1/index-coderank-v1/index-verification.json and semantic-network-none-v1/semantic-response.json under resident evidence-cohort-20261110. Current main function body was read. This observation does not invalidate earlier studies that used other runtime profiles.

## Fix

Reworded the stored-index banner in `build_project_status_segments` (`src/prompts/mod.rs`) from "Built — `semantic_search` is ready to use" to "Built — `semantic_search` also needs a working embedder". It states the storage fact and names the second prerequisite. No probe was added, as the earlier note advised. The `Built` prefix stays, so the existing assertion `Semantic index:** Built` still holds. The segment is `Substitutable` and rides the tool response, so the 1900-character static cap does not bind it, and no `ONBOARDING_VERSION` bump applies. NOT changed in `ad5fbcfc9`: the onboarding prompt line `**Semantic index:** ready (N files, M chunks)` and the Phase 1 announcement in `src/prompts/source.md`, which used the same word for the same stored-index fact. Changed in `ab599d204` (2026-11-11): the data line now reads `built (N files, M chunks)`; Phase 1 reads "If the index is BUILT" and the announcement says the agent will try `semantic_search` and fall back to `grep` if a query fails because the embedder is unavailable; `ONBOARDING_VERSION` 33 to 34 (the `onboarding_prompt` slice feeds the stored per-project prompt); the `onboarding_prompt.md` snapshot is blessed. A separate defect found on the way, the opposite overclaim (a probe failure reported as "not built"), is bug `191a5b00c7ab02b6`.

## Tests added

`prompts::tests::built_index_banner_does_not_promise_semantic_search_works` renders the response block for a status with `has_index: true` and checks the `Semantic index:` line: it must keep `Built`, must not contain `ready`, and must name the embedder. Watched RED on the old line (failed on `ready`), GREEN after the reword. Measured 2026-11-11: `cargo test --lib prompt` 205 passed; full `./scripts/gate.sh` green (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; lean 6409 and default 9312 tests passed, Python 1827 passed). The test checks the rendered text, not the runtime: it does not show that an unreachable embedder produces the failure the banner now warns about. Onboarding surface, `ab599d204`: `prompts::tests::build_onboarding_shows_index_built` (renamed from `..._ready`) asserts the data line `built (42 files, 350 chunks)` and that the prompt says "Semantic index is built" and never "Semantic index is ready"; the pin in `system_prompt_points_to_tool_guide_resource` moved to 34. Both watched RED, then GREEN. The gate for that commit had one DEFAULT failure, `server::guide_hint_tests::a_declared_section_still_arrives_once_the_content_topic_is_spent`, caused by another session's uncommitted rewrite of `src/prompts/guides/tracker-conventions.md`; it passed on a clean tree of HEAD plus the five committed files.

## Workarounds

Require a successful source-bound semantic query in the exact runtime. The campaign also needs a qualified embedding-only transport profile; the existing external validator deliberately accepts network none only.

## Resume

Keep as a product prompt issue separate from the research runtime integration. No Rust source was edited by this report. Do not weaken network isolation or count the failed query as a repair-model result.


## Fix provenance

- **SHA:** `ad5fbcfc921e55178771a23ad56a1081e6d812ea` (`experiments`)
- **patch-id:** `4b2cf21ee8390edde4c072960c89a668cc927ac9`

The first commit rewords the status banner. A second commit, `ab599d204b267e74096bb5015930f982342b71bd` (`experiments`, patch-id `17750d485cea34f05bba1fb5cf35ca42b0de09ed`), rewords the onboarding data line and the Phase 1 announcement, and moves `ONBOARDING_VERSION` to 34.
