---
id: '52f65564a09fd8f6'
kind: bug
status: fixed
title: 'BUG: every CI Test lane is red because one test builds an embedder from ambient config, and the local gate is green only because a dev env var is set'
tags:
- ci
- test-env-isolation
- embeddings
- false-green
- gate
- cluster/repro-env-diverges-from-gate-env
closed: 2026-08-26
opened: 2026-08-26
owner: marius
severity: high
unverified: CI has NOT been re-run — origin/experiments is 62 commits behind local, so no lane has seen this fix. What is established is that the reproduction goes red→green and the whole lib suite passes with the variable unset (4330/0), which is the environment CI runs in for this test. Separately, the 2026-08-24 local-embed lane failed on two OTHER tests (artifact_advertises_the_append_entry_section_writer, every_guide_topic_is_triggered_or_declared_pull_only) that were never assessed against HEAD; both pass locally today, but that is not the same as a green lane.
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

Taken: **option 2, inject via project config rather than env.** The test now builds its
`Agent` over a `tempfile::tempdir()` carrying `.codescout/project.toml`:

```toml
[project]
name = "embedder-wiring"

[embeddings]
model = "openai:text-embedding-3-small"
url = "http://127.0.0.1:1"
```

`url` is what does the work — it selects `build_embedder`'s HTTP branch, which only
*constructs* an `EmbedderHttp`; no network call is made, so a deliberately dead port is
fine. The env-driven construction path stays fully under test, which is why
`set_memory_embedder_for_test` could not be used. No process env is mutated, so the
`EnvGuard` prohibition is respected and no `serial_test` ordering is needed.

The seam was already proven: `tests/retrieval_unit.rs` has two tests
(`config_from_env_and_project_prefers_project_toml_when_env_silent` and its `env_wins`
twin) that exercise exactly this root → `ProjectConfig::load_or_default` → merge chain.
The unit test simply never got the same treatment.

**Two doc-comment corrections fell out of verifying it**, both in
`src/retrieval/client.rs`:

- `build_embedder`'s doc said a url combined with a **`local:` or `local-dir:`** model is
  rejected by `guard_local_model_with_url`. Only `local-dir:` is — deliberately, and the
  guard's own doc comment explains at length why covering `local:` would break every
  ordinary remote deployment. The wrong line is what led this fix to predict a rejection
  that cannot happen.
- `guard_local_model_with_url`'s doc cited this very test as the reason the wider guard
  was abandoned, describing it as building "from a root-less config". That is no longer
  true, and the parenthetical would have quietly become false. Updated to say so, and to
  note that the reasoning stands without the example.

**What was NOT done:** option 1 (threading a `RetrievalConfig` through `Agent`) remains
the correct close for the ambient-config class as a whole — `from_env_and_project` still
reads a dozen `CODESCOUT_*` variables directly, and this fix only removes the one that
had a victim.
## Tests added

None added — the repair is to the setup of an existing test, and that test's `Arc::ptr_eq`
assertion is unchanged.

**Verified by mutation and by control, because a rewritten setup can silently neuter the
assertion it feeds:**

| probe | result |
|---|---|
| the fix, `env -u CODESCOUT_EMBEDDER_URL` | **green** (was the original panic) |
| delete the `[embeddings]` block | **red**, original panic — the config is load-bearing |
| set `model = "local:AllMiniLML6V2Q"` alongside the url | **green** — mutant survives |

The surviving mutant is the interesting one. It was predicted to fail, and its survival is
what exposed the `build_embedder` doc error above: `guard_local_model_with_url` covers
`local-dir:` only. Recorded rather than "fixed" — the narrow guard is correct, and the
prediction was wrong.

Full gate, with the variable unset — the environment CI runs in for this test:
`cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test --lib`
**4330 passed / 0 failed / 8 ignored**. Identical counts with the variable set, so the
gate is now environment-independent where it was not before.
## References

- `src/agent/mod.rs` — `Agent::memory_embedder`, the test at 2305-2327, and `EnvGuard`'s
  doc comment at 1909
- `src/retrieval/client.rs` — `RetrievalClient::from_env`
- `docs/conventions/test-env-isolation.md`
- `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`
  — why env mutation in default-feature tests was removed in the first place

## Fix provenance

- **SHA:** `d81064f7` (`experiments`)
- **patch-id:** `eebb9eb7286e37470c619200687a9db8ddb34b9e`

`fix(tests): stop memory_embedder's test reading the developer's environment` — the
project-config injection in `src/agent/mod.rs`, plus the two doc-comment corrections in
`src/retrieval/client.rs` that the surviving mutant exposed.

One commit, `experiments` only. `master` is a strict ancestor, so promotion is a
fast-forward and this is already the master-side SHA; there is no second one to record.
