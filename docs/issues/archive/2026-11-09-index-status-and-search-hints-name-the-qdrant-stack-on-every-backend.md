---
id: '28d4f0268e7b1a2e'
kind: bug
status: fixed
title: 'BUG: index status and semantic_search failure hints send a lite-stack user to retrieval-stack.sh'
tags:
- cluster/hint-composed-without-the-request
closed: 2026-11-09
opened: 2026-11-09
unverified: 'The five production call sites of classify_search_error in semantic_search and the index status offline arm pass VectorBackend::resolve() to code the tests cover, and no test drives a failing embedder through semantic_search or index(action=status). The mapping is tested per backend; the call-site wiring is read from the diff and compile-checked, not exercised. A real from_env failure that is not a RecoverableError needs either process environment or a build-dependent config; no deterministic seam was found (the memory call sites have one: d01f0c0e8).'
---

# BUG: index status and semantic_search failure hints send a lite-stack user to retrieval-stack.sh

## Summary
Two places build a recovery hint without reading the active backend. `index(action="status")` answers "Retrieval stack offline: {e}. Run scripts/retrieval-stack.sh up." and `classify_search_error` sends five of its arms to `./scripts/retrieval-stack.sh` (and the generic fallback to `docker logs codescout-qdrant`). On the default lite (`sqlite-vec`) build there is no stack to start, so the hint names a command that does not apply.

## Class fit
Same class as `docs/issues/archive/2026-11-09-memory-embedder-hint-names-the-qdrant-stack-on-every-backend.md`: the hint is a fixed string written for the Qdrant case and does not read the request's backend. `VectorBackend::offline_hint()` (`src/retrieval/code_store.rs`) already holds the per-backend text.

## Symptom (Effect)
```
Retrieval stack offline: <error>. Run scripts/retrieval-stack.sh up.
```
from `index(action="status")`, and from `semantic_search` the hints "Restart with `./scripts/retrieval-stack.sh up` and retry." and "Check `./scripts/retrieval-stack.sh ps` and qdrant logs (`docker logs codescout-qdrant`)."

## Reproduction
Not run. Best lead: on a default build, with `CODESCOUT_VECTOR_BACKEND` unset and no reachable embedding endpoint, call `index(action="status")`, then `semantic_search` with a query that fails inside the embedder.

## Environment
Branch `experiments`. Default build (no `server-stack` feature).

## Root cause
- `src/tools/semantic/index.rs` (status, `RetrievalClient::from_env` `Err` arm): a literal message with the script. `RetrievalClient::from_env` (`src/retrieval/client.rs`) resolves the backend itself, so the same arm fires on lite.
- `classify_search_error` (`src/tools/semantic/semantic_search.rs`) takes `(err_str, project_id)` and no backend. Six call sites pass it errors from both backends.
- The same status arm also says "Qdrant scroll failed" for any `project_index_stats` error.

inferred from the source on 2026-11-09 — not measured at runtime.

## Evidence
`grep retrieval-stack src/tools/semantic/semantic_search.rs src/tools/semantic/index.rs` on 2026-11-09. The manual (`docs/manual/src/tools/semantic-search.md`) repeats the status message and calls it "an unreachable Qdrant".

## Hypotheses tried
None.

## Fix

Fixed on `experiments` in `2f37a8167`.

- `classify_search_error` (`src/tools/semantic/semantic_search.rs`) takes the `VectorBackend`. On the lite stack the five arms that named `retrieval-stack.sh` or `docker` (embedder status, TEI, embedder connect, transport, generic fallback) name `CODESCOUT_EMBEDDER_URL` and `EMBED_API_KEY`. The Qdrant text is unchanged. The five production call sites pass the backend resolved once per call.
- `status_offline_message` (`src/tools/semantic/index.rs`) builds the `index(action="status")` offline message from `VectorBackend::offline_hint()`. The `project_index_stats` error arm says "Index status query failed" on every backend.
- `docs/manual/src/tools/semantic-search.md` shows the per-backend message.

Gate green before the commit: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Fix provenance

- **SHA:** `2f37a81679b07cd70ce7279763028bb65964f19f` (`experiments`)
- **patch-id:** `b62a5d95fd08ed260f3b11a848f11b49ccbf7695`

## Tests added

- `classify_search_error_tests::a_lite_stack_search_failure_is_not_sent_to_the_qdrant_script` and `a_qdrant_stack_search_failure_still_names_the_script` (`semantic_search.rs`): the same five error strings, one per script arm, on each backend.
- `status_offline_message_tests` (`index.rs`): three tests on the message per backend and on the cause staying in it.
- `classify_search_error_tests::a_lite_query_at_another_width_is_told_to_rebuild` (`204508f53`): produces the real error from `SqliteVecCodeStore` (3-wide index, 5-wide query) and routes it through the classifier. `a_lite_error_that_says_not_found_is_not_a_missing_qdrant_collection`: the same function, a lite error containing "not found". Mutation probe: with the dimension constant misspelled and the `!lite` guard removed, exactly these two tests failed and the other 22 passed; the file was restored.

Limit: see `unverified` in the frontmatter.

## Workarounds
Read the error text before the hint: it names the real cause.

## Not covered
- None open for the two Qdrant-specific arms: fixed in `204508f53`. The lite store has no collection to miss (`query` returns no hits for an unindexed project), so that arm is Qdrant-only. For a query at another width, sqlite-vec says `Dimension mismatch for query vector for the "embedding" column. Expected 3 dimensions but received 5.`; the Qdrant arm matched neither "Vector dimension" nor "expected dim", so the lite fallback answered. A lite arm now names `index(action="build", force=true)`. Measured by running the lite store, not read from source.
- The lite message was not run against a real lite stack end to end: the test calls the real `SqliteVecCodeStore` and the classifier, not `semantic_search`.
- `src/dashboard/api/index.rs` carried the same "Qdrant scroll failed" label; it says "Index status query failed" since `d01f0c0e8`.

## References
- `docs/issues/archive/2026-11-09-memory-embedder-hint-names-the-qdrant-stack-on-every-backend.md`
