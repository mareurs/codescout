---
id: '5faa9c6e3fd2a6f9'
kind: bug
status: taken
title: Native bootstrap says semantic index is not built while status is queryable
tags:
- cluster/unclassified
- effort-steering
claimed_at: 2026-11-04
claimed_by: 48d211eb-3bd8-4247-975b-631a73b1a87a
opened: 2026-11-04
owner: marius
severity: medium
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

Agent::project_status checks only .codescout/embeddings/project.db existence (legacy path), and prompt rendering maps that boolean to Built/Not built. Actual IndexStatus uses RetrievalClient::from_env and project_index_stats for the selected backend. Current V and the binary's recorded base commit have the same legacy-path check; actual four-state controls establish this behavior independently on the dirty pinned package. Neither Git synchronization nor another full index repairs the bootstrap predicate. No product change yet.

## Evidence

Four actual native controls in worker-context-verification-20261104/readiness-results.json cross empty/populated sqlite-vec store with absent/present ZERO-BYTE legacy project.db. Results: empty/no marker -> indexedfalse, bootstrapNotbuilt,0results; empty/marker -> indexedfalse, bootstrapBuilt,0results; populated/no marker -> indexedtrue/queryabletrue, bootstrapNotbuilt,3returnedresults; populated/marker -> indexedtrue/queryabletrue, bootstrapBuilt,3returnedresults. This establishes the actual predicate mismatch on the pinned package, including false positive and false negative. Full wire/launch/cleanup retained. Source inspection in current V and the binary's base commit locates the legacy existence predicate; dirty binary source identity is not assumed.

## Hypotheses tried
The observed Git-baseline/no-op-index control did not remove the contradiction. No blanket stale-state cause is inferred.

## Fix

Applied 2026-11-04, not yet committed when this was written. `Agent::project_status` no longer tests whether the legacy `.codescout/embeddings/project.db` exists. It calls `crate::tools::config::check_has_index_cached` (now `pub(crate)`), the bounded and cached `project_has_chunks` probe that `activate` already uses. The bootstrap block and `index(action="status")` therefore read the same store.

Not changed: the coverage verdict stays separate (the four whitespace-only files, `cc61bf08099dac2d`). The first probe per session waits at most `FIRST_PROBE_TIMEOUT` (2 s). On a timeout it reports "Not built" and does not cache that answer, as `activate` already does.

## Tests added

Two product tests in `src/tools/config/tests.rs`, written before the fix and seen to fail for the stated reason (`has_index` wrong at the assertion):

- `bootstrap_reports_a_populated_store_as_built_without_the_legacy_file` (false negative cell).
- `bootstrap_reports_an_empty_store_as_not_built_despite_a_legacy_file` (false positive cell).

They seed the store's answer through the shared `index_status_put` cache, which is the value `check_has_index_cached` returns. They do not build a real sqlite-vec store. The two cells with matching store and marker were not added: the old code passed them already. The native four-state controls above remain the check on a real store.

## Workarounds
Read actual status, verify and query results; do not schedule a paid comparison from the banner or blindly rebuild from it.

## Resume
Trace both readiness producers and reproduce populated/empty backend controls before applying a product correction. Preserve existing reusable cache and source-only onboarding.

## References

[Actual client context verification and preserved evidence](../research/2026-11-04-worker-context-verification.md). The full continuation report and bundle are in the main checkout.
