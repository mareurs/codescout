---
id: '98dd2eb72228cf9d'
kind: bug
status: fixed
title: 'BUG: the bare-model status test flipped to remote-http once under the full parallel lane and passes alone'
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related: []
severity: low
---

## Summary

`tools::config::tests::status_reports_local_onnx_for_an_urlless_bare_model_name` failed once in
a full default-lane gate run (2026-09-24, `./scripts/gate.sh`, per-session target) with
`left: "remote-http", right: "local-onnx"`, and passed when re-run alone minutes later on the same
tree. The diff under test touched no config code (`src/tools/config` unchanged since the previous
green gate, `git log c8d4e0d6..HEAD -- src/tools/config` empty).

## Symptom (Effect)

```
assertion `left == right` failed: a bare model name with no url resolves through arm 6 to a
local ONNX embedder, so the status must name it, got: String("remote-http")
  left: String("remote-http")
 right: String("local-onnx")
```

A spurious red in the default lane — the lane every session reads as "did my change break
something" — on a diff that cannot reach embedder resolution.


**Recurrence 2026-09-25, recorded by session `09093108` (not claimed).** This is the second observed flip, so the title's "once" is now a lower bound.

- The run was `./scripts/gate.sh`'s DEFAULT lane in a leased slot, at HEAD `87001799` plus one uncommitted edit to an unrelated test in `src/retrieval/index_state.rs`.
- The assertion at `src/tools/config/tests.rs:788` got `String("remote-http")`. The lane's other 5,821 tests passed.
- An immediate re-run of `tools::config::tests` (85 tests, default features) passed 85/85.
- The test was not skipped, so no ambient embedder URL was set when its guards ran. `#[serial_test::serial]` serializes it only against other `serial` tests, so a NON-serial test setting `CODESCOUT_EMBED(DER)_URL` between the guard and the `ProjectStatus` call would produce exactly this. That is a hypothesis, not measured.

## Reproduction

Not reproduced deterministically. Observed once in a full `cargo test --workspace` (5801 tests,
parallel); `cargo test --lib -- status_reports_local_onnx_for_an_urlless_bare_model_name` alone:
`1 passed`.

## Root cause

**Writer identified; the file's original hypothesis about it was wrong.** The racing writer is the bare `#[test]` `resolved_chunk_budget_reflects_the_projects_configured_model` (`src/tools/memory/tests.rs`), which wraps its body in `temp_env::with_vars_unset(embedding_env::all_names(), ..)`. A bare `#[test]` is outside every `serial` group, so it ran concurrently with the `tools::config::tests` status tests. `status_reports_local_onnx_for_an_urlless_bare_model_name` checks for an ambient embedder URL and skips if one is set; that guard ran inside the memory test's unset window and passed, the ambient value was then restored, and `Agent::new` read the restored URL and resolved to `remote-http`. The `EnvGuard` helper this file's earlier text suspected is gated on the `server-stack` feature and `#[serial]`, so it was not the writer.

Source: the fixing commit `84d5e230` records a reproduction (measured 2026-10-05): with only those two tests selected and `CODESCOUT_EMBEDDER_URL` exported, 20 of 30 runs failed with the exact message above; either test alone, or `--test-threads=1`, 0 of 10; after the fix the same 30-run loop failed 0 times. That reproduction was not re-run while closing this file. Whether the shells of the two original flips (2026-09-24 and 2026-09-25) had the URL exported was not checked, so the match between them and the reproduced mechanism is by symptom, not by their environment.

## Fix

The memory test now carries `#[serial_test::serial]` (the default group the status tests are already in), with the measurement in its doc comment. Both status-test guards, in the bare-model test and in `status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`, now read through `embedding_env::read(&MODEL)` and `embedding_env::read(&URL)` in `src/tools/config/tests.rs`. The previous hand-typed `std::env::var` pairs omitted the canonical `CODESCOUT_EMBEDDING_*` names, so a shell exporting only `CODESCOUT_EMBEDDING_URL` passed the guard and then resolved to `remote-http`. A source lint in `tests/env_mutation_isolation.rs` now refuses any `src/` function that uses `temp_env` without a `serial` attribute. `serial` only locks against other annotated tests, so this closes the pairing of a `temp_env` window against the serial status readers and nothing wider. The convention's option A (an injected `EmbedEnv` seam in `RetrievalConfig::from_env_and_project`, removing the env mutation altogether) was not done, because `EmbedEnv` is private.

## Tests added

No behavioural regression test was added for the race itself; the evidence is the 30-run loop quoted under Root cause. The commit adds three tests to the existing `tests/env_mutation_isolation.rs`, which cover the class rather than this one pair:

- `every_src_function_that_uses_temp_env_is_serial` scans `src/` and fails on any `temp_env` user without a `serial` attribute, and on a bare `use` of the crate (which would hide later calls). Per the commit message, removing the attribute from the memory test turns it red at `src/tools/memory/tests.rs`.
- `the_temp_env_scan_is_not_vacuous` pins a floor of 3 `temp_env` lines seen under `src/`, so a blind walk cannot pass the absence assertion above.
- `the_temp_env_checker_flags_exactly_the_unserialised_users` drives the checker on synthetic source: a bare `#[test]` user is flagged on the line of the call, and serial users (including with a doc comment between the attributes) are not.

The lint scans `src/` only, and matches `serial` as a substring of an attribute line.

## Fix provenance

- **SHA:** `84d5e230` (`experiments`)
- **patch-id:** `51ae6fec43c9b58732f2a92b8319286b4d2c1e35`

## Resume

Closed on 2026-10-06. Residual follow-ups, listed and not filed:

- `serial` locks only against annotated tests (`docs/conventions/test-env-isolation.md`); a non-`serial` test that mutates the embedder variables through some route other than `temp_env` is still not covered by the lint.
- Convention option A (an injected `EmbedEnv` seam in `RetrievalConfig::from_env_and_project`) is not done; `EmbedEnv` is private, so it needs a visibility decision first.

## References

- `docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md`
  — the bug this test guards
