---
kind: bug
status: fixed
title: retrieval_integration embedder tests fail with HTTP 501 against mock server
owners: []
tags:
  - embedder
  - tests
  - mock-server
opened: 2026-05-18
closed: 2026-05-18
---

## Symptom

`tests/retrieval_integration.rs::embedder_returns_dense_and_sparse` and
`embedder_dim_mismatch_errors` both fail with the same root cause:

```
HTTP status server error (501 Not Implemented) for url
(http://127.0.0.1:<random>/v1/embeddings)
```

The tests panic on the embed call — the mock server returns 501 instead of
the expected dense+sparse payload.

## Reproduction

```
cargo test --test retrieval_integration -- embedder_returns_dense_and_sparse
```

Fails deterministically (not a flake — every run, same error).

## Scope

- Reproduces on both `master` and `experiments` (verified 2026-05-18 during
  pre-merge audit).
- `git log master..experiments -- tests/retrieval_integration.rs` is empty —
  no commit on experiments touched the test file.
- Therefore: pre-existing on master, **not** introduced by experiments.

## Hypotheses

- The mock server crate (`wiremock` or similar) may have changed its default
  for unmatched routes from 404 → 501, or vice-versa.
- The test sets up route matchers that no longer match the actual request
  shape — e.g. embedder client now sends `/v1/embeddings` with extra params
  the mock's expectation does not allow.
- The `dense openai status` and `embed: dense openai status` panics suggest
  the test is panicking on a status assertion before reaching content
  parsing.

## Workaround

None applied. Marked as known pre-existing breakage in the 2026-05-18 merge
notes. Cherry-pick / FF merge of experiments → master does not change this
state — same failure before and after.

## Root cause

The user's `.env` sets `CODESCOUT_EMBEDDER_PROTOCOL=openai` for the live MCP
server. `EmbedderHttp::new()` reads that env var at construction time and
selects `DenseProtocol::OpenAi` — which posts to `/v1/embeddings`, not the
TEI `/embed` path that the test mock was registered for. mockito's
unmatched-route default is HTTP 501 ("not implemented"), hence the symptom.

The defect was test isolation: `EmbedderHttp::new` reads process env on
every construction, so any caller's environment leaks into every test.
The mock-server URL was correct; the protocol selection was not.

## Fix

Landed on experiments in 58133ae7.

Added `EmbedderHttp::with_protocol(...)` — explicit constructor taking
protocol, model name, and query prefix as arguments, with no env reads.
`EmbedderHttp::new` is preserved as the env-reading convenience for
production callers; it now delegates to `with_protocol` for field
construction. The two integration tests switched to `with_protocol(...,
DenseProtocol::Tei, "", "")`, making them hermetic.

## Tests added

The existing `tests/retrieval_integration.rs::embedder_returns_dense_and_sparse`
and `embedder_dim_mismatch_errors` now serve as the regression tests — they
were the symptom and are the verification.

## Resume

If env-var-leak surfaces again in other tests, prefer adding similar
explicit-constructor escape hatches over locking. Mutex-based env locking
(`lock_env_for_tests`) is `pub(crate)`-only and not reachable from
integration tests under `tests/`, so it cannot fix this class of defect
without leaking internals.
## Resume

When picking this up:
1. Read `tests/retrieval_integration.rs` lines 24 and 54 (the panic sites)
   to confirm the assertion shape.
2. Check the embedder client crate for recent changes to how requests are
   shaped — `crates/codescout-embed/src/`.
3. Run `cargo update -p wiremock` (or whichever mock crate is used) to see
   if a major upgrade flipped a default; review changelog.
4. If the 501 is from an unmatched route in the mock, log the actual
   incoming request shape and reconcile with the registered expectations.
