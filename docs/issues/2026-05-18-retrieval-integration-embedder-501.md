---
kind: bug
status: open
title: retrieval_integration embedder tests fail with HTTP 501 against mock server
owners: []
tags:
  - embedder
  - tests
  - mock-server
opened: 2026-05-18
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
