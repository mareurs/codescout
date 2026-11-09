---
id: '9afb19c765edc936'
kind: bug
status: open
title: 'BUG: memory embedder failure tells a lite-stack user to run retrieval-stack.sh up'
tags:
- cluster/hint-composed-without-the-request
opened: 2026-11-09
---

# BUG: memory embedder failure tells a lite-stack user to run retrieval-stack.sh up

## Summary
When the memory embedder cannot be built, `memory(action="remember")` and `memory(action="recall")` answer with the hint "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack." The hint is the same for every backend. On the default (lite, `sqlite-vec`) build there is no stack and no script to start, so the hint sends the agent to a command that does not exist outside this repo.

## Class fit
The hint is a fixed string written for the Qdrant case. It does not read the request's backend, which `semantic_search` already does in `map_from_env_error` (`src/tools/semantic/semantic_search.rs`). It differs from the class's other members in that the correct text is already implemented one module away.

## Symptom (Effect)
```
embedder unavailable: <error>
hint: Run `./scripts/retrieval-stack.sh up` to start the retrieval stack.
```
Shown on a lite-stack build, where the real fix is `CODESCOUT_EMBEDDER_URL` / `EMBED_API_KEY` (see `docs/manual/src/concepts/lite-stack.md`).

## Reproduction
Not run. Best lead: on a default build, with `CODESCOUT_VECTOR_BACKEND` unset and no reachable embedding endpoint, call `memory(action="remember", ...)`.

## Environment
Branch `experiments`. Default build (no `server-stack` feature).

## Root cause
Two call sites map the embedder error to a fixed hint: `src/tools/memory/mod.rs` (the `remember` path, `memory_embedder_for(...).map_err`) and the `recall` path a few hundred lines later. Both pass the literal `"Run `./scripts/retrieval-stack.sh up` to start the retrieval stack."` regardless of `VectorBackend::resolve()`. `Agent::memory_embedder_for_with` (`src/agent/mod.rs`) reaches the same error on the `SqliteVec` arm through `memory_embedder_for_config`.

inferred from the source on 2026-11-09 — not measured at runtime.

## Evidence
`grep retrieval-stack src/tools/memory/mod.rs` on 2026-11-09 finds the literal at two sites. `map_from_env_error` in `src/tools/semantic/semantic_search.rs` already branches on `VectorBackend`.

## Hypotheses tried
None.

## Fix
Route both sites through one backend-aware hint, for example the text `map_from_env_error` builds, so the lite stack names `CODESCOUT_EMBEDDER_URL` and `EMBED_API_KEY`. Pin it with a test per backend.

## Tests added
N/A — not fixed yet.

## Workarounds
Read the `embedder unavailable: <error>` text, which names the real cause.

## Resume
Extract the backend-keyed hint from `map_from_env_error` into a shared function and call it from both memory sites.

## References
- `docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md` (same class)
- `src/prompts/builders.rs` retrieval-stack paragraph, rewritten on 2026-11-09 to name no backend
