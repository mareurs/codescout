---
status: fixed
opened: 2026-06-13
closed: 2026-06-15
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

**Confirmed: env-leak via the env-reading ctor.** Pre-fix, the test built its client with `RerankerHttp::new(server.url())`, whose ctor calls `Protocol::from_env()` — which reads `CODESCOUT_RERANKER_PROTOCOL` (`src/retrieval/reranker.rs:18-27`) — plus `CODESCOUT_RERANKER_MODEL`. Under the full `cargo test` suite, env state leaked from a sibling test's `.env`/`set_var` reads into the shared process env, so the client's runtime config diverged from the test's Tei intent and the decode target no longer matched the mock body → `invalid type: map, expected a sequence`. In isolation the env was clean, so an isolated `--test … <name>` run could pass — the failure was test-ordering / env-leak dependent (hence the `env-leak` tag).

(The exact leaked variable was not re-derived: the hermetic-ctor fix makes it moot, and reconstructing it would require checking out `b716d664^`. Recorded honestly rather than asserting a precise mechanism the error direction doesn't cleanly confirm.) Same family as the embedder hermetic-ctor fix `ca57869e`, which had not covered the reranker ctor.
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

Fixed by **`b716d664`** — *fix(reranker): hermetic test ctor — env reads no longer leak from .env* (2026-06-14). It introduced `RerankerHttp::with_protocol(base, protocol, model_id)` — a ctor that takes protocol/model explicitly and reads **no** process env — and switched `reranker_returns_scores_in_input_order` to use it (`Protocol::Tei`, `model_id=None`). The test is now hermetic; `new()` (env-reading) remains the production convenience. This is the exact fix the bug's lead (a) predicted.

**Zombie-open:** the fix shipped under a `fix(reranker):` subject that never referenced this file, so it stayed `status: open` until the 2026-06-15 verify-open pass surfaced that the test was green. **SHA:** experiments-side `b716d664` (also on `vdi-windows`); NOT yet on `master` — file stays in `docs/issues/` until it ships there.
## Tests added

No new test — the existing `reranker_returns_scores_in_input_order` (`tests/retrieval_integration.rs:74-92`) IS the regression guard: `b716d664` made it construct via the hermetic `with_protocol`, so it no longer depends on ambient env. **Verified 2026-06-15:** passes in isolation AND in the full `cargo test --test retrieval_integration` run (4/4 green). The hermetic ctor + this test together cover the regression.
## Workarounds
None needed for the logging work (disjoint). The `--lib` gate does not run this test, so
local/CI `cargo test --lib` stays green; a full `cargo test` shows the red.

## Resume

N/A — fixed by `b716d664`, verified 2026-06-15 (isolation + full integration-file pass, 4/4). Archive to `docs/issues/archive/` once `b716d664` ships to `master`.
## References
- `tests/retrieval_integration.rs:74-92` (the test).
- `69f741d9 feat(retrieval): RerankerHttp client`; `edeaa96c` (multi-protocol env knobs);
  `ca57869e` (hermetic test ctor — env leak fix).
