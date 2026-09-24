---
id: '98dd2eb72228cf9d'
kind: bug
status: open
title: 'BUG: the bare-model status test flipped to remote-http once under the full parallel lane and passes alone'
tags:
- cluster/transient-shared-state-lies-to-readers
closed: null
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

## Reproduction

Not reproduced deterministically. Observed once in a full `cargo test --workspace` (5801 tests,
parallel); `cargo test --lib -- status_reports_local_onnx_for_an_urlless_bare_model_name` alone:
`1 passed`.

## Root cause

**Hypothesis, not established.** The test is `#[serial_test::serial]` and checks the ambient
`CODESCOUT_EMBEDDER_URL` / `CODESCOUT_EMBED_URL` (skipping if set) BEFORE `Agent::new` resolves
the embedder. `serial` only serialises against other `serial` tests; a non-serial test that sets
one of those variables between the check and `Agent::new` would make the live backend
`remote-http` exactly as observed. The competing test has not been identified.

## Fix

Not attempted. Candidates: move the ambient check after an env guard that clears both variables
for the test's duration, or find the non-serial writer and serialise it.

## Tests added

None.

## References

- `docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md`
  — the bug this test guards
