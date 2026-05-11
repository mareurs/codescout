---
title: Remote-Only Embedding Architecture
date: 2026-05-11
status: proposed
owners: [Marius]
tags: [embedding, architecture, removal, codescout-embed]
related:
  - docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md
  - docs/trackers/archive/embedding-chunk-size-2026-04.md
  - docs/trackers/retrieval-benchmark.md
  - docs/research/2026-04-03-embedding-model-benchmark.md
---

# Remote-Only Embedding Architecture

Remove `LocalEmbedder` (fastembed + ONNX runtime) from `codescout-embed`. Make
`RemoteEmbedder` (HTTP backend, Ollama / llama-server / OpenAI-shape) the sole
production implementor of the `Embedder` trait. Embedding becomes an
infrastructure concern delegated to an external docker service.

## 1. Scope & Motivation

### Goals

- Delete the `LocalEmbedder` backend and all fastembed / ONNX runtime
  dependencies from `codescout-embed`.
- Keep the `Embedder` trait. One production implementor (`RemoteEmbedder`) plus
  one test implementor (`MockEmbedder`) justify the abstraction.
- Make `[embedding] url` in `project.toml` a required configuration key. Absent
  URL produces a single recoverable error pointing the user at the GitHub
  documentation for setting up a docker embedding service.
- Replace the per-model `chunk_size_for_model` derivation with a fixed default
  (1600 chars) plus a user-overridable `[embedding] chunk_size` key. Preserve
  AST symbol boundaries even when a method exceeds the target.

### Drivers

1. **Compile cost and binary size.** `fastembed` pulls the ONNX runtime, the
   `tokenizers` crate, and model-download machinery. These dominate cold
   compile time and add measurable bytes to the release binary.
2. **Maintenance burden.** Two backends mean two failure modes, two
   model-naming conventions, and two sets of integration tests. The
   `Embedder` trait pretended the backends were interchangeable, but their
   operational profiles never matched.
3. **Deployment topology.** A docker-hosted embedding service is now always
   available in every codescout deployment shape we ship. The local backend is
   redundant in practice.
4. **Platform reach.** ONNX runtime requires per-platform native libraries
   (`.so`, `.dylib`, `.dll`). Windows support has been blocked by the
   complexity of bundling and distributing these. A pure-Rust + `reqwest` path
   is cross-platform free.

### Non-goals

- Folding `codescout-embed` back into the main crate. The crate boundary still
  protects the rest of the codebase from `reqwest` and HTTP-shape concerns.
  Re-evaluate after the dust settles.
- Deleting the `Embedder` trait. Single production implementor is acceptable
  because the trait earns its keep at the test boundary (mock impl) and as
  future-proofing for provider swaps.
- Auto-spawning a docker container on first index. Codescout does not become a
  process supervisor.
- Changing retrieval ranking, reranker, vector storage layer, or BM25 pipeline.
- Changing the wire format used by `RemoteEmbedder` to talk to the embedding
  service.

## 2. Architecture

### Container view — before

```
codescout (Rust binary)
  └── codescout-embed (crate)
        ├── LocalEmbedder ── fastembed ── ONNX runtime (native .so/.dylib/.dll)
        │                                 └── tokenizers, model files on disk
        └── RemoteEmbedder ── reqwest ── HTTP ──► [external docker service]
```

### Container view — after

```
codescout (Rust binary)
  └── codescout-embed (crate)
        └── RemoteEmbedder ── reqwest ── HTTP ──► [external docker service]

[external docker service]   ◄── hard dependency for semantic_search + index
  └── Ollama / llama-server / OpenAI-shape provider
```

### Component view — `codescout-embed` after

| Component | Role | Change |
|---|---|---|
| `Embedder` trait (`embedder.rs`) | Single abstraction boundary | Unchanged |
| `RemoteEmbedder` (`remote.rs`) | HTTP client: batching, retry, HTTPS/loopback guard | Unchanged |
| `chunker.rs` | Chunking + AST-aware splitting | Modified Phase 4: soft 1600-char target, preserve symbol boundaries |
| `lib.rs::create_embedder_with_config` | Factory | Simplified: no `local:` prefix branch, no fastembed model-id parsing |
| `lib.rs::chunk_size_for_model` | Per-model chunk derivation | Deleted Phase 4 |
| `local.rs` | fastembed wrapper | Deleted |
| `parse_model` (in `local.rs`) | fastembed model-id parser | Deleted |
| `mock.rs` (new) | Deterministic test double | Added Phase 1 |

### Dependency direction

Domain code (`src/tools/semantic.rs`, `src/index/`) depends only on the
`Embedder` trait. The trait depends on nothing external. `RemoteEmbedder`
depends on `reqwest`. Inward-pointing arrows preserved.

### Coupling surface reduction

`Cargo.toml` of `codescout-embed` drops:

- `fastembed`
- transitively: ONNX runtime native binding, `tokenizers`, model-download deps
- `cargo update` shrinks the lockfile measurably

### External contract change

- `project.toml` `[embedding] url` becomes **required**.
- `[embedding] chunk_size` newly honored. The latent bug from archived tracker
  `fa795feac3878eac` (where this key was silently ignored) is fixed as part
  of Phase 4.
- `[embedding] model` semantics narrow: no more `local:bgesmallenv15q`-style
  identifiers. Only remote model names (e.g. `nomic-embed-text`,
  `text-embedding-3-small`).
- Existing indexes with a `local:` model hash become invalid on first run after
  upgrade. `check_model_mismatch` in `src/embed/index.rs` already fires on this
  case — but today it bails with `anyhow!` telling the user to manually delete
  `.codescout/embeddings.db`. Phase 3 adds a narrow special case: when the
  *stored* model has a `local:` prefix and the *configured* model does not, the
  database is wiped automatically with a one-line log message. Other mismatches
  preserve the existing manual-delete behavior.

### Error UX

When `[embedding] url` is absent the factory returns `RecoverableError`. The
message must carry the entire migration in this order:

1. Which config key is missing (`[embedding] url`)
2. Suggested docker image to pull
3. URL to the codescout GitHub documentation for embedding setup

`RecoverableError` keeps sibling tool calls alive — only `semantic_search` /
`index` fail, the rest of the session continues working.

## 3. Phasing

The Yak's walk. Each phase is one green commit. Tests pass after every step.

### Phase 0 — Safety net (audit only, no code change)

- Baseline `cargo test` pass count recorded.
- `grep -r "LocalEmbedder\|fastembed\|local:" src/ crates/ tests/` enumerates
  every call site.
- Classify tests into three buckets:
  - **Plumbing** (assert on chunk emission, vector storage, factory wiring) —
    migrate to mock.
  - **Quality** (assert on similarity scores, top-k ranking, semantic
    equivalence) — convert to ignored integration tests or delete.
  - **Implementation-coupled** (assert on fastembed internals) — delete.

**Exit condition:** every `LocalEmbedder` reference classified before Phase 1
begins.

### Phase 1 — Introduce `MockEmbedder`

Single commit. Additive only.

- Add `crates/codescout-embed/src/mock.rs` behind
  `#[cfg(any(test, feature = "test-mock"))]`.
- Behavior: hash input text → seed deterministic RNG → emit orthogonal unit
  vector of configured dimension. Orthogonality is the discipline that
  forces test authors to assert on plumbing, not quality.
- Factory recognizes `mock:<dim>` URL prefix when the test feature is enabled.
- One self-test in `mock.rs` proves determinism + orthogonality.

**Green gate:** `cargo test` still passes. Release binary size unchanged
because `mock.rs` is cfg-gated.

### Phase 2 — Migrate tests file-by-file

N commits, one file per commit. Easy bisect.

- Each commit migrates one test file from `LocalEmbedder` to `MockEmbedder`.
- Quality-assertion tests are converted to `#[ignore]` and moved to
  `tests/embedding_integration.rs`, gated on the `CODESCOUT_TEST_EMBED_URL`
  env var.

**Green gate:** `cargo test` passes after every commit. `cargo test --
--ignored` runs only when the env var is set.

### Phase 3 — Delete `LocalEmbedder` and fastembed dep

Single commit.

- Delete `crates/codescout-embed/src/local.rs`.
- Remove `mod local` from `lib.rs`.
- Remove `parse_model` (fastembed-specific).
- Drop `fastembed` from `crates/codescout-embed/Cargo.toml`.
- `cargo update` to shrink the lockfile.
- Drop the `local:` prefix branch from `create_embedder_with_config`.
- Update the factory error: missing URL → `RecoverableError` with the wording
  defined in Section 2 (Error UX).
- Update `check_model_mismatch` in `src/embed/index.rs`: when the stored model
  starts with `local:` and the configured model does not, wipe
  `.codescout/embeddings.db` automatically with a one-line `tracing::info!`
  noting the auto-wipe reason. All other mismatch cases keep the existing
  manual-delete behavior. Unit tests added for both branches.

**Green gate:** `cargo test`, `cargo clippy -- -D warnings`,
`cargo build --release` all pass. Record the binary size delta in the commit
message.

### Phase 4 — Chunk size policy change

Single commit. Behavior change, not pure refactor — done after the removal
has landed.

- Delete `chunk_size_for_model` and its model-name substring tables.
- Add `pub const DEFAULT_CHUNK_SIZE_CHARS: usize = 1600` in `lib.rs`.
- Wire `[embedding] chunk_size` from `project.toml` through the indexer into
  the chunker (fixes the latent bug from archived tracker
  `fa795feac3878eac`).
- Rename `enforce_max_chunk_size` → `prefer_chunk_size`. The AST chunker uses
  1600 chars as a target for *sub-boundary* splits (impl block → methods,
  module → functions) but never truncates a leaf symbol. A 2400-char method
  is emitted whole.
- Re-run the 20-query benchmark from spec `76b7e842b04bdc3c`. Append the
  result as a new `### YYYY-MM-DD` entry under `## History` in tracker
  `82e601ad8472c2a5`.

**Green gate:** unit tests pass. Benchmark hits the success criteria from
the metadata-enriched-chunks spec (total ≥ 30/60).

### Phase 5 — Config schema, docs, error wording, version bump

Single commit. User-facing surface polish.

- `project.toml` `[embedding]` schema documented: `url` required, `model`
  narrowed, `chunk_size` newly honored and documented.
- Update three prompt surfaces (the CLAUDE.md rule):
  - `src/prompts/server_instructions.md`
  - `src/prompts/onboarding_prompt.md`
  - `src/prompts/builders.rs::build_system_prompt_draft()`
- Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` (the onboarding
  prompt is a surface that produces the stored per-project system prompt).
- Update `README.md`, `docs/ARCHITECTURE.md`, and the embedding setup page
  under `docs/manual/src/`.
- Write the ADR at `docs/adrs/2026-05-11-remote-only-embedding.md` (outline
  in Section 5).
- Write the `CHANGELOG` entry for the major bump.
- Bump `Cargo.toml` `version` (major).

**Green gate:** `prompt_surfaces_reference_only_real_tools` test passes.
Manual smoke via `cargo build --release` + `/mcp` restart against a live
docker embedding service.

### Blast radius summary

| Phase | Files touched (approx) | Behavior change? | Revertible? |
|---|---|---|---|
| 0 | 0 (audit only) | no | n/a |
| 1 | 3 new | no | yes |
| 2 | ~10 (one per commit) | no | yes per commit |
| 3 | ~5 | yes (test impl removed, error reworded) | yes |
| 4 | ~4 | yes (chunk policy) | yes |
| 5 | ~8 (prompt surfaces + docs) | yes (error UX, prompt) | yes |

## 4. Test Strategy

Three layers. Each proves something different. Do not let them blur.

### Layer 1 — Unit tests with `MockEmbedder`

**Proves:** plumbing. Chunk emitted, embedder called, vector stored, query
returns the chunk.

**Does not prove:** retrieval quality, ranking, semantic similarity. The
mock returns orthogonal vectors — any ranking assertion against it is
tautological.

**Rule for test authors:** assertions reference `chunk.id`,
`chunk.start_line`, `len(results)`, `result.metadata`. Never `result.score`
or `results[0].id == "expected"`.

**Coverage targets:**

- `chunker.rs` — splits, overlaps, AST boundaries, soft-1600 target with
  symbol preservation.
- `lib.rs::create_embedder_with_config` — factory error paths, missing URL,
  malformed URL.
- `remote.rs` — request shape, batching, retry policy, HTTPS/loopback guard
  (existing tests keep `wiremock`).
- `src/index/` — index build, refresh, invalidation cycles.
- `src/tools/semantic.rs` — output guard, pagination, `by_file` aggregation.

### Layer 2 — Ignored integration tests

**Proves:** contract with a real docker embedding service. Request shape
still valid. Server-side batching limits enforced. Error mapping survives
upstream changes.

**Gate:** `#[ignore]` by default. Run via `cargo test -- --ignored` only when
`CODESCOUT_TEST_EMBED_URL` is set.

**CI integration:** a new scheduled GitHub Action runs daily against a
`docker compose up` of a known-good embedding image. Failure opens an issue
tagged `embedding-contract-drift`. Does **not** block PRs — drift is
detected, not gated.

**Scope:** approximately five tests, one per code path that touches the
wire. Not a quality suite.

**Location:** `tests/embedding_integration.rs`.

### Layer 3 — Retrieval quality benchmark

**Proves:** retrieval quality, recorded over time. Not a `cargo test`.

**What it is:** the 20-query benchmark in
`docs/research/2026-04-03-embedding-model-benchmark.md`, scored 0–60.

**When it runs:** before Phase 4 ships (to validate the 1600-char soft
target). On demand thereafter when retrieval changes.

**Where results land:** appended as a new `### YYYY-MM-DD` entry under
`## History` in tracker `82e601ad8472c2a5` (Retrieval Stack — Benchmark
Results). This tracker is the canonical record of quality over time.

A failing benchmark is a design signal, not a test regression.

### Decision matrix

| Test asserts on… | Layer | Runs in CI? |
|---|---|---|
| "chunker produced N chunks for input X" | 1 (mock) | every PR |
| "factory returns error when URL absent" | 1 (mock) | every PR |
| "RemoteEmbedder retries on 503" | 1 (wiremock) | every PR |
| "real Ollama accepts our batch size of N" | 2 (ignored) | daily scheduled |
| "real provider returns vectors of expected dim" | 2 (ignored) | daily scheduled |
| "TC-04 retrieval scores ≥ 2/3" | 3 (benchmark) | manual, recorded in tracker |

## 5. ADR Outline

Path: `docs/adrs/2026-05-11-remote-only-embedding.md`

Sections:

1. **Context** — two `Embedder` impls existed; remote matured to where
   consolidation pays off.
2. **Decision** — remove `LocalEmbedder`; `[embedding] url` becomes required;
   no fallback, no auto-discovery, no bundled service; trait survives for
   test mock + future provider swap; major version bump.
3. **Constraints driving the decision** — the four drivers from Section 1.
4. **Alternatives considered:**
   - Auto-spawn docker on first index — rejected: codescout becomes a process
     supervisor, lifecycle bugs ahead.
   - Bundle compose + degrade gracefully — rejected: silent partial-feature
     mode hides misconfiguration.
   - Default localhost probe — rejected: implicit dependencies break in
     production; an explicit URL is one line of config.
   - Deprecation window with warning logs — rejected: pre-1.0 project; the
     "kindness" of a window costs an extra release of double-backend
     maintenance.
5. **Consequences:**
   - Positive — smaller binary, faster CI, Windows path unblocked, single
     error surface for embedding failures, no model-id schema sprawl.
   - Negative — docker is now a hard install dependency for semantic search;
     first-run experience requires both binary and embedding service;
     mitigated by good error message and docs.
   - Neutral — retrieval quality unchanged (mock + benchmark protect both
     directions); existing `local:`-prefix indexes auto-wiped on upgrade via
     a narrow `check_model_mismatch` special case.
6. **When to revisit:**
   - A pure-Rust embedding implementation (e.g. `candle`, `burn`) reaches
     production-grade with stable model coverage.
   - GitHub issues tagged `embedding-setup` accumulate beyond a quarterly
     threshold.
   - The embedding service space consolidates enough that auto-spawn becomes
     a one-line concern.
7. **References:**
   - This spec.
   - Spec `76b7e842b04bdc3c` (Metadata-Enriched Chunks + Chunk Size Tune).
   - Archived tracker `fa795feac3878eac` (oversized chunks / ignored config).
   - Tracker `82e601ad8472c2a5` (Retrieval Stack — Benchmark Results).
