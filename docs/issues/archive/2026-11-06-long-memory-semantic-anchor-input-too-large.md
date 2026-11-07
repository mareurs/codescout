---
id: 139881c33d1798b7
kind: bug
status: fixed
title: Memory semantic anchor creation fails when local embedder rejects a long topic
tags:
- cluster/unclassified
- memory
- embeddings
closed: 2026-11-07
opened: 2026-11-06
owner: marius
severity: medium
unverified: STANDING For a long memory the anchors reflect only its head, not its whole body. The reranker (off by default, CODESCOUT_RERANK) receives the same query bounded by the embedder budget only; a smaller-context reranker is not accounted for (rerank_or_passthrough degrades to vector order on error). A whitespace-only memory over budget still gives an empty query, as before.
---

# BUG: Long memory write persists text but fails semantic anchor creation

## Summary

A real memory(action=write) on research/effort-steering/current returned status ok and persisted the topic, but reported semantic anchor creation failed because the local embedding server rejected oversized input. The exact scope of affected semantic data is not yet verified.

## Class fit

Unclassified memory/indexing boundary failure. The observed operation is memory anchor creation, separately from the historical artifact-grain embedding issue799145380a54b665. Do not infer missing durable text from the embedding warning.

## Symptom (Effect)

Actual tool response on2026-11-06: status ok; wrote_to /home/marius/work/claude/codescout; warnings contains `semantic anchor creation failed: embed status failed: http://127.0.0.1:48081/v1/embeddings — HTTP 500 from embedding server` with error message `input is too large to process. increase the physical batch size`.

## Reproduction

The observed write appended the disk-audit closure to the complete current research topic through the real memory tool. The prior full text was read in two bounded pages and preserved; the write did not replace it with a section. Reproduce with an isolated long synthetic topic and the same embedder configuration, not repeated writes to the research record. No synthetic reproduction attempted yet.

## Environment

MAIN /home/marius/work/claude/codescout; Codescout server455879; local embedding endpoint127.0.0.1:48081. Running binary build provenance and physical batch/context configuration have not been checked for this issue.

## Root cause

Corrected 2026-11-07 by the fixing session: this section said "Unconfirmed". The code now shows the mechanism, so that statement is withdrawn. The earlier text is kept below as history.

Mechanism (confirmed in code): in `src/tools/memory/mod.rs`, `cross_embed_memory` stores the memory's vector through `embed_document_pooled`, which segments the content and mean-pools it to the model budget. `find_semantic_anchors` did not do that. It passed the WHOLE memory content to `client.search_code`, which embeds the query with no bound (`src/retrieval/search.rs`, `search_in` -> `embedder.embed_one`). A long memory therefore gave llama.cpp HTTP 500 `input is too large to process`. That error was swallowed as a warning, so the memory was saved without semantic anchors. The cause is not backend batch configuration, a stale binary or request chunking in the write path; it is the unbounded anchor query.

What measured it: the new test `a_long_memory_anchors_through_a_query_bounded_by_the_chunk_budget` was red before the fix with a query of 20902 chars against a chunk budget of 5222. The mechanism itself is read from the code paths named above (not re-measured against the live llama.cpp server).

Earlier text, superseded: "Unconfirmed. The endpoint directly rejected an input in this call. Whether chunking, request sizing, backend physical batch configuration, or stale running binary caused the rejection is not yet established. No claim about total semantic anchor absence or search behavior has been tested."

## Evidence

Observed response from memory(action=write,topic=research/effort-steering/current) during the2026-11-06 private-grade disk audit. Dedicated smaller topic research/effort-steering/private-grade-audit-20261106 and reportfce51ffc4baf7906 preserve the audit separately. The research evidence archive89dfc97b... predates this incidental warning and does not claim to capture it.

## Hypotheses tried

None beyond observing the real failing call. The successful text write and failed anchor step must remain separate outcomes.

## Fix

Implemented on branch `fix/open-issues-batch-20261107-r2`. All three commits below are now on `experiments` at 143c1941f, which is the same SHA because the branch reached `experiments` by fast-forward. On that identical tree `./scripts/gate.sh --check` gave FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0.

Where the change lives: `src/tools/memory/mod.rs`.

- A private helper `anchor_query_prefix(content, budget_chars)` returns the content unchanged when it fits the chunk budget. Otherwise it returns the longest leading slice (after leading whitespace) of at most `budget_chars` chars. The slice is cut after the last whitespace in the second half of the window; if there is none, it is a hard cut on a char boundary.
- `cross_embed_memory` passes that bounded query to `find_semantic_anchors`, which now takes a `query: &str` instead of the whole content.
- Two budget-dependent tests got `#[serial_test::serial]`, because a `#[serial]` test unsets the embedding env vars in a window and could skew the budget the test read versus the budget the write used.

Plan deviation: an earlier first-segment version (`segment_for_budget(...).next()`) was rejected in review. A heading followed by one over-budget line gave a heading-only query. It is superseded by the prefix helper.

Commits (the labels name the branch they were made on; all are on experiments, see above). The primary commit, the one that fixes the behaviour, is 83e1f68c1f4eb03749cfee99350cc15b4a62c3e2; 6a89e83667e933fc23cbeb07b281e493d3c535dc (first-segment version, superseded) and af3e2a9ec85e778d0459785d460bb8fc677c211a (signature, clippy) are also on experiments:

- 6a89e83667e933fc23cbeb07b281e493d3c535dc (branch fix/open-issues-batch-20261107-r2) / patch-id f7a4eed603452d39c71ab81edcda8f22268d4ce1. First-segment version, same patch as f481e75e7. Superseded by the next commit.
- 83e1f68c1f4eb03749cfee99350cc15b4a62c3e2 (branch fix/open-issues-batch-20261107-r2) / patch-id bf6a54610931f68a5ea7c0e3f415b2ac4bde0a42. The prefix repair.
- af3e2a9ec85e778d0459785d460bb8fc677c211a (branch fix/open-issues-batch-20261107-r2) / patch-id 710856803a7bfea2b620325ad690eda4320b7033. The signature takes the query, clippy `too_many_arguments`, doc wording.

Verification: `./scripts/gate.sh --check` was run on `fix/open-issues-batch-20261107-r2` at its tip af3e2a9ec on 2026-11-07: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests. An Opus reviewer reviewed the fix adversarially, ran the new test red against the reverted production change (or used mutants), and approved.

## Tests added

All in `src/tools/memory/tests.rs`:

- `a_long_memory_anchors_through_a_query_bounded_by_the_chunk_budget` (red before the fix: 20902 chars against a 5222 budget)
- `a_heading_before_one_over_budget_line_still_anchors_from_the_body` (red with the first-segment query)
- `a_short_memory_anchors_through_its_whole_content` (exact equality)
- `the_anchor_query_prefix_is_bounded_and_word_aligned`

Line numbers: Unknown (not recorded).

## Workarounds

Read the memory by its exact topic and use the smaller dedicated audit topic/report. Durable text is retained; semantic refresh completeness is unverified.

## Fix provenance

- **SHA:** `83e1f68c1f4eb03749cfee99350cc15b4a62c3e2` (`experiments`)
- **patch-id:** `bf6a54610931f68a5ea7c0e3f415b2ac4bde0a42`

## Resume

N/A for the code: the fix landed on `experiments` (fast-forward to 143c1941f, same SHAs), `./scripts/gate.sh --check` was green there, and this file was archived to `docs/issues/archive/`. The `unverified:` caveat keeps only limits that no action can settle.

## References

Historical related artifact-size issue799145380a54b665. Current research reportfce51ffc4baf7906.
