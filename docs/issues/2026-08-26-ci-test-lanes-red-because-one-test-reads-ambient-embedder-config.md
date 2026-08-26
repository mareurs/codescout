---
id: '1440b07375a1e30f'
kind: bug
status: open
title: 'BUG: every CI Test lane is red because one test builds an embedder from ambient config, and the local gate is green only because a dev env var is set'
tags:
- ci
- test-env-isolation
- embeddings
- false-green
- gate
closed: ''
opened: 2026-08-26
owner: marius
severity: high
---

## Summary

`agent::tests::memory_embedder_is_built_from_the_shared_code_embedder` calls
`agent.memory_embedder().await.unwrap()`, which runs `RetrievalClient::from_env()` →
`RetrievalConfig::from_env_and_project(root)`. That resolves the embedder from **ambient
configuration**. On this machine the shell exports
`CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081`, so an `EmbedderHttp` is built and the
`unwrap()` succeeds. On a CI runner nothing is set, the config default `local:AllMiniLML6V2Q`
applies, and building it needs the `local-embed` feature — so the `unwrap()` panics.

Consequence in two directions, and the second is the serious one:

- **CI has been red on every `Test` lane** — ubuntu / macos / windows × default /
  no-features / local-embed, plus `server-stack` — for at least five runs, back to
  2026-08-19.
- **The local gate is green only by accident of one environment variable.** `cargo fmt`,
  `cargo clippy -- -D warnings`, `cargo test` is what CLAUDE.md requires before completing
  any task, and it is the evidence behind every *"gate green"* line in every bug file
  archived in that window. It reads green here in a world where CI is red.

## Symptom (Effect)

```
thread 'agent::tests::memory_embedder_is_built_from_the_shared_code_embedder' panicked at
src/agent/mod.rs:2309:84:
called `Result::unwrap()` on an `Err` value: could not build the 'local:AllMiniLML6V2Q'
embedder: Local embedding requires the 'local-embed' feature.
```

The message is accurate and even helpful. Nothing about it says *"this test should not have
needed a backend at all"*, which is the actual defect.

## Reproduction

One variable, on the machine where the gate is green:

```
env -u CODESCOUT_EMBEDDER_URL cargo test --lib memory_embedder_is_built_from_the_shared_code_embedder
```

Reproduced at HEAD `433a8de7` on 2026-08-26 — identical panic and identical message to the
CI log. This is not a stale 2026-08-24 artifact; the failure is live.

## Environment

- Local: `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081` exported in the shell. No
  `[embeddings]` section in `.codescout/config.toml` or `~/.config/codescout/config.toml`
  — the env var is the whole reason it resolves.
- CI: neither set. `local-embed` is not a default feature.

## Root cause

The test asserts **instance identity** — that `memory_embedder()`'s returned
`CodeDenseAdapter` wraps the *same* `Arc` as the `RetrievalClient` it built, via
`Arc::ptr_eq`. That assertion needs no working embedder backend. But the only way to reach
it is through the env-driven construction path, and that path insists on building a real
embedder first.

`Agent::set_memory_embedder_for_test` exists as the injection seam for exactly this kind of
test — and **cannot be used here**, because pre-populating the cache bypasses the
construction path that is the thing under test. This is the same argument `EnvGuard`'s doc
comment makes about its own single consumer, in the same file.

## Evidence

Blast radius measured, not estimated — full lib suite with the variable removed:

```
env -u CODESCOUT_EMBEDDER_URL cargo test --lib
→ 4326 passed; 1 failed; 8 ignored
```

**Exactly one test of 4335** depends on ambient embedder config. The dependency is a
pinpoint, not a systemic rot — which is what makes the false green so durable: 4326 green
lines scroll past the one that is only green by luck.

CI run `32740102144` (`047dd433`, 2026-08-24) — `Clippy`, `Format`, `MSRV`, `Feature check`
and `Tool Docs Sync` all **pass**; every `Test` lane and `Audit Doc Refs` fail. The
ubuntu/default lane reports `4301 passed; 1 failed`, the same single test. The
`local-embed` lane fails on two *different* tests
(`server::tests::artifact_advertises_the_append_entry_section_writer`,
`server::tests::every_guide_topic_is_triggered_or_declared_pull_only`) — not yet checked
against HEAD, 57 commits later, and out of scope for this file.

## Fix

Not chosen. Three options, each with precedent in this crate, and the trade-off is real:

1. **Thread a `RetrievalConfig` through `Agent`.** Named as the correct close by
   `EnvGuard`'s own doc comment — *"worth doing, not done here"*. Largest change, removes
   the ambient dependency at the source, and would let the test construct the path
   explicitly.
2. **Inject via project config rather than env.** `RetrievalConfig::from_env_and_project`
   takes a `root`; build the `Agent` over a temp project carrying
   `.codescout/config.toml` with an `[embeddings].url`. Uses the blessed injection points
   (`GlobalConfig::load_from_dir` / `ProjectConfig::load_with_global_base`) instead of
   mutating env, and keeps the construction path under test. Needs confirming that
   `from_env_and_project` reads the file for this key, and note that a set env var still
   wins over it (`docs/issues/archive/2026-08-13-url-silently-overrides-local-dir-model.md`)
   — harmless here, since the assertion is about wiring and not about which backend.
3. **Move the test behind `server-stack` and use `EnvGuard`.** Matches the one env-mutating
   helper the crate deliberately kept, whose stated reason — *"exercises the ENV-DRIVEN
   construction path; injecting past it would delete the thing under test"* — is verbatim
   this test's situation. Cheapest, but it drops the guard from eight lanes to one, and
   that lane is currently red for unrelated reasons.

**What is NOT acceptable:** skipping when no embedder is configured
(`if from_env().is_err() { return }`). That is green on CI in a world where the wiring is
broken, and the `Arc::ptr_eq` assertion — sabotage-verified by a previous author — is
precisely what would stop proving anything.

## Tests added

None yet. The reproduction above is the failing case, and any fix must be checked with the
variable unset, not on a developer shell.

## References

- `src/agent/mod.rs` — `Agent::memory_embedder`, the test at 2305-2327, and `EnvGuard`'s
  doc comment at 1909
- `src/retrieval/client.rs` — `RetrievalClient::from_env`
- `docs/conventions/test-env-isolation.md`
- `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`
  — why env mutation in default-feature tests was removed in the first place

