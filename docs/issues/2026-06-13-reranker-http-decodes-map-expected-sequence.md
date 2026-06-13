---
status: open
opened: 2026-06-13
closed:
severity: medium
owner: marius
related: []
tags: [retrieval, reranker, tests, integration, env-leak]
kind: bug
---

# BUG: reranker_returns_scores_in_input_order fails deterministically (map vs sequence)

## Summary
The integration test `reranker_returns_scores_in_input_order` fails deterministically:
`RerankerHttp::rerank` decodes the HTTP response expecting a JSON **sequence** but
receives a **map**. Pre-existing (the test file is unchanged since before the
2026-06-13 session) and disjoint from the honest-usage.db-logging work — surfaced
only because that work ran the full `cargo test` suite (integration tests are skipped
by the usual `cargo test --lib` gate).

## Symptom (Effect)
```
thread 'reranker_returns_scores_in_input_order' panicked at tests/retrieval_integration.rs:88:10:
rerank: rerank json

Caused by:
    0: error decoding response body
    1: invalid type: map, expected a sequence at line 1 column 1
```
Exit code 101. Fails identically on repeat runs (not flaky).

## Reproduction
```
git rev-parse HEAD   # experiments, ~f13f6a46
cargo test --test retrieval_integration reranker_returns_scores_in_input_order
```
The test's `mockito` mock serves `[{"index":1,"score":0.9},{"index":0,"score":0.1}]`
(a sequence) for `POST /rerank`, yet the decode reports it received a map.

## Environment
- Linux, codescout v0.15.0, branch `experiments`. Test uses `mockito` (hermetic mock
  server) — no real external service.

## Root cause
Unknown — under investigation. `RerankerHttp::rerank` deserializes into a sequence type,
but at runtime the body it decodes is an object. Candidate leads: (a) `RerankerHttp`
reads a protocol/format knob from env or `.env` (cf. `edeaa96c feat(retrieval):
multi-protocol embedder, env knobs`) that makes it expect/post a different shape, and the
`ca57869e fix(embedder): hermetic test ctor — env reads no longer leak from .env` fix did
not cover the reranker ctor; (b) the request doesn't match the mock (wrong path/headers),
so mockito returns its unmatched-request object body. Mechanism not yet pinned.

## Evidence
- Two identical failing runs (full suite + isolated `--test` run), same map-vs-sequence error.
- `git log -- tests/retrieval_integration.rs` → latest touch `ca57869e`, which is present
  at session-start HEAD `26ae1c4b` — so the test is unchanged across this session ⇒
  pre-existing failure, not introduced here.
- Full suite at `f13f6a46`: 2755 passed, 1 failed (this test), 46 ignored.

## Hypotheses tried
1. **Hypothesis:** flaky mockito mock-registration race. **Test:** re-ran the single test.
   **Verdict:** rejected — deterministic failure, identical error both runs.
2. **Hypothesis:** introduced by the 2026-06-13 logging changes. **Test:** the changes
   touch only `src/usage/*` + the `Tool` overflow envelope in `src/tools/core/types.rs`;
   `RerankerHttp` is in the retrieval subsystem and never goes through that envelope; the
   test file predates the session. **Verdict:** rejected — disjoint subsystem, pre-existing.

## Fix
Unknown — not addressed (out of scope of the logging plan; pre-existing). Status `open`.

## Tests added
N/A — the failing test already exists; the fix (when made) should make it pass, plus a
hermetic-ctor guard if the cause is env leakage.

## Workarounds
None needed for the logging work (disjoint). The `--lib` gate does not run this test, so
local/CI `cargo test --lib` stays green; a full `cargo test` shows the red.

## Resume
Read `RerankerHttp::rerank` and `RerankerHttp::new` (find via
`symbols(name="RerankerHttp")`): confirm the expected response type and whether the ctor
reads env/`.env`. If env-sensitive, apply the same hermetic-ctor pattern as `ca57869e`.
Otherwise add a request matcher to the mock and dump the actual decoded body to see what
the map is. Anchor with `cargo test --test retrieval_integration reranker_returns_scores_in_input_order`.

## References
- `tests/retrieval_integration.rs:74-92` (the test).
- `69f741d9 feat(retrieval): RerankerHttp client`; `edeaa96c` (multi-protocol env knobs);
  `ca57869e` (hermetic test ctor — env leak fix).
