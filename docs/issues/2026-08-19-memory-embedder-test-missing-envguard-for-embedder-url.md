---
status: open
opened: 2026-08-19
closed:
severity: low
owner: marius
related: []
tags: [test-isolation, embedder]
kind: bug
---

# BUG: `memory_embedder_is_built_from_the_shared_code_embedder` lacks the `EnvGuard` its sibling test uses, so it depends on ambient embedder config instead of testing what it claims to

## Summary
`agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`
(`src/agent/mod.rs`) fails before it ever reaches its actual assertion
(`Arc::ptr_eq` on `CodeDenseAdapter`) because it doesn't pin
`CODESCOUT_EMBEDDER_URL` the way its neighboring test in the same file does.
It hits ambient default config, tries to resolve the default
`local:AllMiniLML6V2Q` model, and panics on `.unwrap()` when no local-embed
backend is compiled in. This is a test-isolation gap, not a production bug
and not a feature-gate issue.

## Symptom (Effect)
```
thread panicked at src/agent/mod.rs:2309:
called `Result::unwrap()` on an `Err` value: could not build the
'local:AllMiniLML6V2Q' embedder: Local embedding requires the
'local-embed' feature.
```

## Reproduction
1. `git checkout experiments` at `5b54848fd2a4e7fe5da6bf277dc85de39958ff27`
2. `cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib agent::tests::memory_embedder_is_built_from_the_shared_code_embedder -- --nocapture`
   (built WITHOUT `local-embed`/`local-embed-dynamic` — see
   `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`)
3. Observe the panic above.

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain, default features (`remote-embed, http, librarian`) + `server-stack`,
no `local-embed`. `codescout` repo, `experiments` branch. Likely also
reproducible on any host without cached ONNX weights / network access even
WITH `local-embed` compiled in, per the sibling test's own comment (see Root
cause).

## Root cause
`agent.memory_embedder()` (`src/agent/mod.rs:1845-1860`) is itself
feature-agnostic — it wraps whatever `RetrievalClient::from_env().embedder`
resolves to. Resolution happens in `build_embedder`
(`src/retrieval/client.rs:260-296`), which falls back to
`config.model` (`default_embed_model()`, `src/config/project.rs:347-349` →
`"local:AllMiniLML6V2Q"`) whenever `[embeddings].url` /
`CODESCOUT_EMBEDDER_URL` isn't set. `local-embed` is not a default Cargo
feature (`Cargo.toml:175`), so this path fails whenever it isn't explicitly
compiled in — and per the sibling test's comment, would fail even with
`local-embed` compiled in on a host without cached weights or network.

The sibling test in the same file,
`semantic_memory_store_bootstrap_times_out_on_hung_qdrant`
(`src/agent/mod.rs:2241-2288`), explicitly guards against exactly this
class of failure:
```rust
// a host with a local model configured would make this test perform a
// real ONNX load (or fail if weights are absent)... the timeout guard
// this test exists to pin would silently stop being exercised.
EnvGuard::set("CODESCOUT_EMBEDDER_URL", "http://unused.invalid");
```
`memory_embedder_is_built_from_the_shared_code_embedder` was introduced in
the same feature work (`git log -S` → `bc79f98c`/`b3aaf820`) but omits that
guard.

## Evidence
### Subagent investigation (2026-08-19)
Confirmed by reading both tests side-by-side in `src/agent/mod.rs` — the
sibling test's `EnvGuard::set("CODESCOUT_EMBEDDER_URL", ...)` call has no
counterpart in the failing test.

## Hypotheses tried
1. **Hypothesis:** This is the same class of "expected failure" as
   `workspace(status)` reporting `embedding_backend: "unavailable"` when
   `local-embed` isn't compiled in — i.e. a `#[cfg(feature = "local-embed")]`
   gate would be the right fix.
   **Test:** Read what the test actually asserts (`Arc::ptr_eq` on
   `CodeDenseAdapter` — embedder *identity* wiring, unrelated to which
   backend is active) and compared against the sibling test's guard
   pattern.
   **Verdict:** rejected — a feature gate would mask the real gap (the
   test's dependence on ambient config) rather than close it. The
   `Arc::ptr_eq` assertion the test is actually about would still be
   worth running on a `local-embed` build with a missing-weights host,
   which the sibling test's comment explicitly anticipates as a failure
   mode too.

## Fix
Not yet implemented. Add the same `EnvGuard::set("CODESCOUT_EMBEDDER_URL",
"http://unused.invalid")` pattern (or equivalent) that
`semantic_memory_store_bootstrap_times_out_on_hung_qdrant` already uses, so
`memory_embedder_is_built_from_the_shared_code_embedder` forces the cheap
HTTP branch and actually reaches its `Arc::ptr_eq` assertion regardless of
build features or host state.

## Tests added
N/A — not yet fixed; this bug file *is* about the test itself.

## Workarounds
Set `CODESCOUT_EMBEDDER_URL` to any reachable (or even unreachable, per the
sibling test's pattern) HTTP endpoint before running this specific test.

## Resume
Add the missing `EnvGuard` to
`agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`
(`src/agent/mod.rs`, near line 2309), mirroring
`semantic_memory_store_bootstrap_times_out_on_hung_qdrant`
(`src/agent/mod.rs:2241-2288`).

## References
- `src/agent/mod.rs:1845-1860` (`memory_embedder`)
- `src/agent/mod.rs:2241-2288` (sibling test with the correct guard pattern)
- `src/agent/mod.rs:2309` (the failing assertion)
- `src/retrieval/client.rs:260-296` (`build_embedder`)
- `src/config/project.rs:347-349` (`default_embed_model`)
- `Cargo.toml:175` (default features)
