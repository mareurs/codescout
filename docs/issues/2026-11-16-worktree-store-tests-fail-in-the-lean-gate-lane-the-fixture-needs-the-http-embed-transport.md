---
id: da6b6a00432b1736
kind: bug
status: fixed
title: 'BUG: the three worktree_store_tests of #34 fail in the lean gate lane, because their fixture needs the HTTP embed transport that --no-default-features removes'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-11-16
opened: 2026-11-16
severity: medium
---

# BUG: the three worktree_store_tests of #34 fail in the lean gate lane, because their fixture needs the HTTP embed transport that --no-default-features removes

## Summary

Commit `5d356cbe1` ("a linked worktree's status and code store resolve against main (#34)") added `src/tools/semantic/worktree_store_tests.rs`. Its fixture writes `[embeddings] url = "http://127.0.0.1:9/v1"` into every `project.toml` (`project_toml`, line 55-60), so each test builds an HTTP embedder. The feature `remote-embed` provides that transport, and `--no-default-features` removes it (`Cargo.toml:209`, `default = ["remote-embed", "http"]`). In the lean lane all three tests fail, and the gate's LEAN lane exits 100. The commit message says "Full suite green", which holds for the default lane only.

## Class fit

The class `repro-env-diverges-from-gate-env` fits: the author's test run used the default features, and the gate also runs a lean build, so the test passed where it was written and failed where it is gated. It differs from the class's other members in that the divergence is a cargo feature set, not an environment variable or a temp directory; the earlier member `2026-08-30-et2-gating-left-the-lean-test-lane-red` is the same feature-set kind.

## Symptom (Effect)

`./scripts/gate.sh` ends with `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=100 DEFAULT=0`. nextest reports `11038 tests run: 11035 passed, 3 failed`. The failing tests:

- `tools::semantic::worktree_store_tests::status_in_a_linked_worktree_reports_the_delta_the_build_wrote`
- `tools::semantic::worktree_store_tests::a_worktree_client_sees_the_main_checkouts_chunks`
- `tools::semantic::worktree_store_tests::status_in_a_plain_checkout_counts_its_own_chunks`

The message of the first two is `an embedder url is configured, but this build has no HTTP embed transport. Rebuild with --features remote-embed, or unset [embeddings].url (CODESCOUT_EMBEDDER_URL) so the model name selects an in-process backend.` The third asserts `indexed: true` and gets `Retrieval stack offline` with the same text.

## Reproduction

Measured 2026-11-16 at `experiments` tip `a7857b2ed`, a clean detached worktree, with no integration code:

1. `TMPDIR=/dev/shm scripts/with-slot.sh cargo test --lib --no-default-features -- worktree_store_tests`
2. Result: `3 failed; 0 passed`.
3. The same command run through `env -u` for all seven `CODESCOUT_EMBED*` variables fails the same way, so the environment is not the cause.

## Environment

`experiments` at `a7857b2ed` (includes `5d356cbe1`, committed 2026-11-16 16:34). The gate run on a merge candidate of `experiments` and `integration/identity-lifecycle-20261114` showed the same three failures, and nothing else.

## Root cause

The fixture's `project.toml` always sets an embedder URL. `RetrievalClient::from_env` then needs the HTTP embedder, which exists only with `remote-embed` (`src/retrieval/client.rs`, `src/retrieval/embedder.rs` gate on it). The test module has no matching `cfg`. Other modules that need the transport gate on `#[cfg(feature = "remote-embed")]` (for example `src/agent/mod.rs:4207`, `src/librarian/mod.rs:905`).

## Evidence

The gate output of the candidate run (job `@bg_00000001`, 2026-11-16), the reproduction above, and the fixture text at `worktree_store_tests.rs:55-60`.

## Hypotheses tried

- The ambient embedder environment: refuted, the run with all seven variables unset still fails.
- The merge of identity work: refuted, the plain `experiments` tip fails the same three tests.

## Fix

The module declaration in `src/tools/semantic/mod.rs` is now `#[cfg(all(test, feature = "remote-embed"))]`, as the other transport-dependent modules do. The lean lane skips these tests, and the default lane keeps them. A lean-build proof of the worktree behaviour would need a fixture that needs no embedder. This fix does not add one.

## Fix provenance

- **SHA:** `0e53e96ab` (`experiments`)
- **patch-id:** `b480eb0bdbf7f2a20883039e29a4969e5a60e4ca`

## Tests added

None. The existing three `worktree_store_tests` are the regression check. Measured 2026-11-16 after the fix: `cargo test --lib -- worktree_store_tests` passes 3 of 3 in the default lane, and `cargo nextest run --workspace --no-default-features` passes 11035 of 11035.

## Workarounds

None are needed after `0e53e96ab`. Before it, the LEAN lane stayed red, and a local run of the default lane passed these tests.

## Resume

Fixed by `0e53e96ab`. The full `./scripts/gate.sh` has not run on the fix. Archive after a green gate.

## References

- Commit `5d356cbe1` (#34), upstream issue `mareurs/codescout#34`.
- Archived bug `docs/issues/archive/2026-08-30-et2-gating-left-the-lean-test-lane-red.md`.
