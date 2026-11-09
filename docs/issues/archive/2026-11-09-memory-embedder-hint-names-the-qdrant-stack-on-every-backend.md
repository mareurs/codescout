---
id: af689c86c4e6f03c
kind: bug
status: fixed
title: 'BUG: memory embedder failure tells a lite-stack user to run retrieval-stack.sh up'
tags:
- cluster/hint-composed-without-the-request
closed: 2026-11-09
opened: 2026-11-09
unverified: The two call sites in src/tools/memory/mod.rs delegate to embedder_unavailable in one line each, and no test drives a failing embedder through memory(remember/recall). The helper is tested per backend; the call-site wiring is read from the diff, not exercised.
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

Fixed on `experiments` in `9b43a3d01` (patch-id `a56e01c91e6c716ee1cb859c6119b72f013016d1`).

- `VectorBackend::offline_hint()` (`src/retrieval/code_store.rs`) holds each backend's text, moved unchanged from `map_from_env_error`.
- `map_from_env_error` (`src/tools/semantic/semantic_search.rs`) calls it.
- `embedder_unavailable` (`src/tools/memory/mod.rs`) builds the memory refusal from it. Both memory sites call it with `VectorBackend::resolve()`.

Gate green before the commit: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Fix provenance

- **SHA:** `9b43a3d0145acb7b7e1cfc4075b2152ac07d89c0` (`experiments`)
- **patch-id:** `a56e01c91e6c716ee1cb859c6119b72f013016d1`

## Tests added

In `src/tools/memory/tests.rs`:

- `a_lite_stack_memory_call_is_not_sent_to_the_qdrant_script`
- `a_qdrant_memory_call_is_told_to_start_the_stack`
- `the_embedder_cause_stays_in_the_message`

They pin `embedder_unavailable` per backend. Limit: no test drives a failing embedder through the two call sites, so reverting one site to the literal would not fail them (see `unverified` in the frontmatter).

## Workarounds
Read the `embedder unavailable: <error>` text, which names the real cause.

## Resume

The call-site limit in `unverified` is the only open item. To close it, build a memory call whose embedder fails at construction and assert its hint.

## References
- `docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md` (same class)
- `src/prompts/builders.rs` retrieval-stack paragraph, rewritten on 2026-11-09 to name no backend
