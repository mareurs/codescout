---
title: ADR — Remote-Only Embedding Architecture
date: 2026-05-11
status: accepted
related:
  - docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md
  - docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md
  - docs/trackers/archive/embedding-chunk-size-2026-04.md
  - docs/trackers/retrieval-benchmark.md
---

# ADR — Remote-Only Embedding Architecture

## Status

Accepted — 2026-05-11.

## Context

`codescout-embed` shipped two implementors of the `Embedder` trait:

- `LocalEmbedder` — fastembed + ONNX Runtime, in-process, models downloaded on
  first use.
- `RemoteEmbedder` — HTTP client targeting any OpenAI-compatible embedding
  service (Ollama, llama-server, OpenAI itself).

Local was the original default. Remote was added once external services
became reliable in our deployment topology.

## Decision

Remove `LocalEmbedder` entirely. `RemoteEmbedder` becomes the sole production
implementor. `[embeddings] url` in `.codescout/project.toml` becomes required.
No fallback, no auto-discovery, no bundled service. The `Embedder` trait
survives — it earns its keep at the test boundary (`MockEmbedder`) and as
future-proofing for provider swaps.

Major version bump on the next release. Existing `local:`-prefix indexes
auto-wipe on first run via a narrow `check_model_mismatch` special case.

## Drivers

1. **Compile cost and binary size.** The ONNX runtime, `tokenizers`, and
   model-download machinery dominate cold compile time and add measurable
   bytes to every release artifact. Confirmed at removal time: release
   binary shrank by ~22 MiB (-39.8%).
2. **Maintenance burden.** Two backends meant two failure modes, two
   model-naming conventions (`local:BGESmallENV15Q` vs `ollama:bge-m3`), and
   two integration-test surfaces.
3. **Deployment topology.** A docker-hosted embedding service is now always
   available in every codescout deployment shape we ship. Local was redundant.
4. **Platform reach.** ONNX Runtime requires per-platform native libraries
   (`.so`/`.dylib`/`.dll`). Windows support has been blocked by the
   complexity of bundling and distributing them. Pure-Rust + `reqwest` is
   cross-platform free.

## Alternatives considered

- **Auto-spawn docker container on first index.** Rejected: codescout becomes
  a process supervisor. Lifecycle bugs, container-cleanup edge cases, and
  unclear ownership of the embedding image's update cadence.
- **Bundle docker-compose + degrade gracefully.** Rejected: silent
  partial-feature mode hides misconfiguration. A hard error is easier to
  debug than `semantic_search` returning empty results.
- **Default localhost probe (Ollama on `:11434`).** Rejected: implicit
  dependencies break in production. One line of explicit config is cheaper
  than a probe and friendlier on first failure.
- **Deprecation window with warning logs.** Rejected: pre-1.0 project, users
  expected to follow breaking changes. A window costs an extra release of
  double-backend maintenance for marginal kindness.

## Consequences

**Positive:**

- Smaller release binary (-22 MiB, -39.8%), faster CI compile, no
  per-platform native lib shipping concerns.
- Windows support unblocked.
- Single error surface for embedding failures — easier to debug.
- No model-id schema sprawl in `[embeddings] model`.

**Negative:**

- Docker becomes a hard install dependency for any user wanting semantic
  search. First-run requires both the codescout binary AND a running
  embedding service.
- Mitigated by an actionable `RecoverableError` message that names the
  config key, suggests a docker image, and links to setup docs.

**Neutral:**

- Retrieval quality unchanged (`MockEmbedder` protects plumbing; the
  20-query benchmark protects quality, recorded in
  `docs/trackers/retrieval-benchmark.md`).
- Legacy `local:`-prefix indexes auto-wiped on upgrade via a narrow special
  case in `check_model_mismatch`. No user action required.
- AST chunker change shipped alongside removal: `enforce_max_chunk_size` →
  `prefer_chunk_size` (identity). The post-hoc char-boundary truncator is
  gone. The signature-prefix-preserving `sub_split_node` still line-splits
  oversized leaf nodes, which keeps embedding vectors focused on
  individual concepts rather than averaging across kitchen-sink chunks.

## When to revisit

- A pure-Rust embedding library (e.g. `candle`, `burn`) reaches
  production-grade with stable, Windows-clean model coverage and no native
  binding requirements.
- GitHub issues tagged `embedding-setup` accumulate beyond ~5 per quarter,
  indicating the docker-setup friction is a meaningful abandonment point.
- The embedding-service space consolidates such that auto-spawn becomes a
  one-line concern (e.g. a stable, opinionated default container image).

## References

- Spec: `docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md`
- Implementation plan: `docs/superpowers/plans/2026-05-11-remote-only-embedding.md`
- Related spec: `docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md`
- Archived tracker: `docs/trackers/archive/embedding-chunk-size-2026-04.md`
- Living tracker: `docs/trackers/retrieval-benchmark.md`
