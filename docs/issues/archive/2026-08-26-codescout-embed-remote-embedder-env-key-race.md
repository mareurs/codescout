---
id: 95c74a543d0c9804
kind: bug
status: fixed
title: codescout-embed's RemoteEmbedder::from_url still reads EMBED_API_KEY from ambient env — three of its tests race the sibling tests that mutate it
owners:
- marius
tags:
- test-isolation
- codescout-embed
- env-race
- concurrency
closed: 2026-08-26
---

## Summary

`crates/codescout-embed/src/remote.rs`'s `RemoteEmbedder::from_url` reads `EMBED_API_KEY`
from ambient process env when no explicit key is passed (`:182`). Four of this file's tests
correctly mutate that same var under `#[serial_test::serial]`, but three others
(`from_url_normalizes_v1_suffix`, `from_url_normalizes_v1_embeddings_suffix`,
`from_url_normalizes_trailing_slash`) call `from_url(url, model, None)` — which reads the
same var — without the tag. `#[serial]` only coordinates *annotated* tests; an untagged
test elsewhere in the same binary still races a tagged mutator, which is precisely the
failure mode `docs/conventions/test-env-isolation.md` documents as "Option B — NOT VIABLE."

This is the neighbour-crate instance of a class the main `codescout` crate already fixed:
`EmbedderHttp` in `src/retrieval/embedder.rs` reads the SAME env var name
(`EMBED_API_KEY`) and was moved to explicit injection (`api_key: Option<String>` threaded
through the constructor, no ambient read at test time) — the doc's own "Established
exemplars" table cites it. `codescout-embed`'s `RemoteEmbedder` never received that fix.

## Symptom (Effect)

Not yet observed as a test failure — the three untagged tests only assert on `e.endpoint`,
never on `e.api_key`, so a raced value doesn't currently flip any assertion. The race is
real (confirmed by reading both sides: the tagged tests genuinely `set_var`/`remove_var`
the same key the untagged tests read), but it is latent rather than actively flaky today.

## Reproduction

```
grep -n 'EMBED_API_KEY\|serial_test::serial\|fn from_url_normalizes' crates/codescout-embed/src/remote.rs
```

Four sites `set_var("EMBED_API_KEY", ...)` / `remove_var("EMBED_API_KEY")` under
`#[serial_test::serial]`: `custom_rejects_http_with_api_key`,
`custom_allows_http_without_api_key`, `custom_allows_https_with_api_key`,
`from_url_falls_back_to_env_api_key`. Three sites call `from_url(..., None)` — which reads
the same var via `RemoteEmbedder::from_url`'s `api_key.or_else(|| std::env::var("EMBED_API_KEY").ok())`
(`:182`) — with no `#[serial]` tag at all: `from_url_normalizes_v1_suffix`,
`from_url_normalizes_v1_embeddings_suffix`, `from_url_normalizes_trailing_slash`.

## Environment

- `experiments` @ `aa511e54`, Linux
- `crates/codescout-embed`, a separate workspace member with its own `cargo test`
  invocation (per this repo's `development-commands` memory)
- Found while auditing for the same defect class as
  `docs/issues/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`
  (codescout's own memory-store tests resolving ambient config unisolated) — checking
  whether the sibling crate in this workspace had a similar gap.

## Root cause

`RemoteEmbedder::from_url` (`crates/codescout-embed/src/remote.rs:182`) reads
`std::env::var("EMBED_API_KEY")` directly inside the constructor rather than having the
caller resolve it and pass it in. `docs/conventions/test-env-isolation.md`'s Option A
("Accept env values as explicit arguments... push the env read up to the caller") is the
rule this violates — and the doc names the exact sibling struct
(`EmbedderHttp`/`src/retrieval/embedder.rs`, same env var, same `api_key` field name) as
the correct exemplar this crate should mirror but doesn't.

`codescout-embed`'s tests were never compiled or run by any CI lane until
`docs/issues/archive/2026-08-11-ci-never-ran-codescout-embed-tests.md` was fixed
(2026-08-11–14) — roughly two weeks *after* `a656f8cec220d347`'s project-wide sweep
(2026-07-27) that took `set_var`/`remove_var` occurrences in the default `cargo test`
build from 119 to 0. That sweep could not have found this instance: the crate it lives in
was invisible to every tool at sweep time.

*Measured 2026-08-26: read both the four tagged and three untagged test bodies plus the
constructor at `:182` — not run under a stress harness, so the race is confirmed by code
inspection, not by an observed flake.*

## Hypotheses tried

1. **Hypothesis:** the untagged tests don't actually reach the `EMBED_API_KEY` read.
   **Test:** read `from_url`'s body — `api_key.or_else(|| std::env::var("EMBED_API_KEY").ok())`
   fires whenever the caller passes `None`, which all three untagged tests do.
   **Verdict:** rejected — they reach it every time.

## Fix

**Shipped 2026-08-26 — narrower than originally planned, and the narrowing is deliberate.**

**SHA:** `b3d2a0c1` (`experiments`)
**patch-id:** `a571303308f46ef17aa208d9c09de8f1b11fb557`

`fix(codescout-embed): stop RemoteEmbedder::from_url reading EMBED_API_KEY from env`.

The plan above said mirror `EmbedderHttp` across `from_url`/`custom`/`ollama` together.
Only `from_url` was actually changed. Re-reading the confirmed race before writing code
found that `custom()`'s three `EMBED_API_KEY`-touching tests are ALL already tagged
`#[serial_test::serial]` — no untagged test calls `custom()`, so nothing races it today.
`ollama()` doesn't read `EMBED_API_KEY` at all (it reads `OLLAMA_HOST`, an unrelated var,
under no confirmed race). Changing either would have been speculative scope creep against
an unproven problem, not a fix for a measured one — so `custom`/`ollama` are untouched,
and their existing `#[serial]` tags stay exactly as needed.

`from_url` no longer accepts an implicit ambient fallback at all: `api_key` is used
exactly as given by the caller. The one production caller
(`create_embedder_with_config`) already receives an explicitly-resolved value from
`RetrievalConfig`/`LibrarianEnv` in every real call site (verified by reading
`src/retrieval/client.rs:283`, `src/librarian/mod.rs:99,368` — all three pass an
already-resolved `config.api_key`/`env.embed_api_key`, never rely on the internal
fallback), so removing it changes no production behavior.

Do not "fix" this by tagging the three untagged tests `#[serial]` instead — that keeps
the ambient-read shape `test-env-isolation.md` already ruled out project-wide, and only
narrows the window rather than closing it (same reasoning the doc gives for why Option B
was retired, not patched).

## Tests added

`from_url_falls_back_to_env_api_key` (`crates/codescout-embed/src/remote.rs`, kept its
name despite now testing the opposite) rewritten as the regression test: sets
`EMBED_API_KEY=sk-should-be-ignored`, calls `from_url(loopback_url, model, None)`, asserts
`api_key.is_none()`. Confirmed RED first for the right reason — the initial version used a
non-loopback host and failed on the HTTPS guard instead of the assertion; corrected to a
loopback host (matching the sibling `from_url_normalizes_*` tests' own style) and
re-confirmed RED (`assertion failed` on the intended line) before GREEN.

Full-crate gate, both feature configurations: `cargo test` (default features, 19 passed)
and `cargo test --features remote-embed` (33 passed, 5 correctly `ignored` — real-Ollama
tests) both green, plus the main `codescout` crate's full suite (4543 passed) and
`cargo fmt`/`cargo clippy --all-targets -- -D warnings` clean in both crates.

## Workarounds

None needed today — no observed failure. If `cargo test` in this crate ever becomes
flaky around `api_key`/endpoint assertions, this file is the first thing to check.

## Resume

Fixed and verified. Nothing outstanding on `from_url`. `custom()`'s ambient
`EMBED_API_KEY` read is UNCHANGED and still not a confirmed problem — leave it unless a
future untagged test starts calling `custom()` directly, at which point re-open or file
fresh rather than reusing this record (the race that would justify touching it doesn't
exist yet).

## References

- `docs/conventions/test-env-isolation.md` — the rule this violates, and the exemplar
  (`EmbedderHttp`) this crate's `RemoteEmbedder` should mirror
- `docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` —
  the same env var, same shape, already fixed in the main crate
- `docs/issues/archive/2026-08-11-ci-never-ran-codescout-embed-tests.md` — why this
  instance was invisible to the project-wide sweep that would otherwise have caught it
- `docs/issues/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md` — the
  sibling investigation this one was found while doing
