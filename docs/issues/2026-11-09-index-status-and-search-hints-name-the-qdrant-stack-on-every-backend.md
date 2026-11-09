---
id: '8906e05d97b175e5'
kind: bug
status: open
title: 'BUG: index status and semantic_search failure hints send a lite-stack user to retrieval-stack.sh'
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-09
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
Not yet.

## Workarounds
Read the error text before the hint: it names the real cause.

## Not covered
- `classify_search_error` arms "Qdrant collection is missing" and "Vector dimension" (curl against Qdrant) are Qdrant-specific too. What a lite store reports for those cases is not verified.
- `src/dashboard/api/index.rs` carries the same "Qdrant scroll failed" label.

## References
- `docs/issues/archive/2026-11-09-memory-embedder-hint-names-the-qdrant-stack-on-every-backend.md`
