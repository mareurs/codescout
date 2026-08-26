---
id: a1deddf51fb81a65
kind: bug
status: open
title: codescout-embed's RemoteEmbedder::from_url still reads EMBED_API_KEY from ambient env — three of its tests race the sibling tests that mutate it
owners:
- marius
tags:
- test-isolation
- codescout-embed
- env-race
- concurrency
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

*Not yet implemented — filed as a discovered-but-not-fixed sibling issue, not part of the
work that found it.*

Mirror `EmbedderHttp`'s already-fixed shape: resolve `EMBED_API_KEY` once at the real
call site (`create_embedder_with_config` in `src/lib.rs`, or wherever `RemoteEmbedder` is
constructed outside tests) and pass it explicitly; stop `from_url`/`custom`/`ollama` from
reading `std::env::var` internally. Tests then construct with an explicit `Option<String>`
and need no `#[serial]` at all — same remedy `test-env-isolation.md` already prescribes,
applied to the one struct in this workspace that didn't get it.

Do not "fix" this by tagging the three untagged tests `#[serial]` instead — that keeps
the ambient-read shape `test-env-isolation.md` already ruled out project-wide, and only
narrows the window rather than closing it (same reasoning the doc gives for why Option B
was retired, not patched).

## Tests added

None yet — no fix implemented.

## Workarounds

None needed today — no observed failure. If `cargo test` in this crate ever becomes
flaky around `api_key`/endpoint assertions, this file is the first thing to check.

## Resume

Read `create_embedder_with_config` (`crates/codescout-embed/src/lib.rs:197-326`) to find
where `RemoteEmbedder` is actually constructed in production, thread an explicit
`api_key: Option<String>` through from there (resolved once, at that edge), and remove
the internal `std::env::var("EMBED_API_KEY")` reads from `custom`/`from_url`/`ollama`.
Then drop `#[serial_test::serial]` from the four tests that no longer need it and confirm
`cargo test` (in `crates/codescout-embed`) stays green with no serial coordination at all.

## References

- `docs/conventions/test-env-isolation.md` — the rule this violates, and the exemplar
  (`EmbedderHttp`) this crate's `RemoteEmbedder` should mirror
- `docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` —
  the same env var, same shape, already fixed in the main crate
- `docs/issues/archive/2026-08-11-ci-never-ran-codescout-embed-tests.md` — why this
  instance was invisible to the project-wide sweep that would otherwise have caught it
- `docs/issues/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md` — the
  sibling investigation this one was found while doing

