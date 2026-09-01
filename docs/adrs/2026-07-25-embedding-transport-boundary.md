---
id: '74c1aa5018287728'
kind: adr
status: proposed
title: codescout-embed owns remote embedding transport; retrieval keeps sparse + vocabulary
tags:
- architecture
- embedding
- boundary
- dependencies
- codescout-embed
topic: embedding transport boundary
---

# ADR: codescout-embed owns remote embedding transport; retrieval keeps sparse + vocabulary

- **Date:** 2026-07-25
- **Status:** proposed (no code written; supersedes nothing)
- **Deciders:** Marius (with the Architecture Snow Lion)
- **Commits:** none yet. Diagnosis from a dependency-leanness review on
  `experiments` at `52fcaf01`. Implementation plan:
  `docs/plans/2026-07-25-embedding-transport-consolidation.md`.

## Decision

`codescout-embed` becomes the single owner of **remote embedding transport**.
`src/retrieval` keeps two things and gives up one:

- **keeps** the domain vocabulary — `SparseVector`, `EmbedOutput`,
  `BatchEmbedder`, `DenseEmbedder` (`src/retrieval/embedder.rs:5-27`, `:457-459`)
- **keeps** the sparse client, moved behind `#[cfg(feature = "server-stack")]`
- **gives up** its own dense HTTP client (`EmbedderHttp`), which is replaced by
  `codescout_embed::RemoteEmbedder`

Root then drops `reqwest`, `rustls`, and `install_default_crypto_provider`
entirely.

## Context / forces

**Two live embedding stacks exist in one workspace, split by consumer rather
than by capability.**

| Stack | Entry | Consumers |
|---|---|---|
| `codescout_embed::create_embedder_with_config` → `RemoteEmbedder` \| local ONNX | the extracted crate | **librarian only** — `src/librarian/mod.rs:98`, `:367` |
| `retrieval::embedder::EmbedderHttp` → `HttpDenseEmbedder` | root crate, inline | **retrieval + memory** — `src/retrieval/client.rs:16-17`, `src/agent/mod.rs:1743`, `src/migrate/memories.rs:43` |

The crate's own description is *"Shared embedding primitives for codescout +
librarian-mcp"*. It is shared by exactly one of its two intended consumers.

**The law was declared and the call site never converted.** `Cargo.toml`
carries, directly above the tree-sitter block:

> `# HTTP client and local ONNX embeddings are now provided by the codescout-embed crate.`

`EmbedderHttp` predates that comment and was never migrated. This is the
`platform-law-leaks-at-call-sites` pattern, second independent occurrence —
the first was git-spawn elimination (WIN-14 converted `resolve_head_sha`,
left `legibility_scan::git_head` for six days). A law declared in one module
leaks at sibling call sites that a diff of the declaring module cannot show.

**The duplication is acknowledged in prose, not enforced by anything.**

- `is_https_or_loopback` exists twice: `src/retrieval/embedder.rs:39-68` and
  `crates/codescout-embed/src/remote.rs:48-77`. The root copy's doc comment
  reads *"Mirrors the codescout-embed `RemoteEmbedder` guard."* This is a
  plaintext-API-key security policy maintained by comment.
- Two rustls provider installers: `src/lib.rs:10`, `remote.rs:84`.
- Two OpenAI wire-struct pairs, same JSON: root
  `OpenAiEmbedReq`/`OpenAiEmbedResp`/`OpenAiEmbedItem` (`:97-111`) vs crate
  `EmbedRequest`/`EmbedResponse`/`EmbedData` (`:29-43`).

**On the dense path the crate is strictly better on five axes**, the root on
one:

| | `EmbedderHttp` | `RemoteEmbedder` |
|---|---|---|
| wire shape, auth, index-sort | identical | identical |
| URL normalization | `format!("{base}/v1/embeddings")` | `from_url` normalizes bare host / `/v1` / `/v1/embeddings` / trailing slash, 4 tests |
| retry | **none** on the dense leg | 3 attempts, 500 ms doubling, 5xx only; 4xx fails fast |
| empty inputs | not filtered on the dense leg | filtered + zero-reconstructed; all-empty → `bail!` citing `2026-05-17-reindex-embedding-dim-mismatch.md` |
| response cap | none | 32 MiB before json-decode |
| dim | validates vs configured `expected_dim` | discovers + caches `cached_dims` |
| connect error | lifts URL + literal `"connect"` marker | plain `anyhow!(e)` |

**The sparse leg is genuinely unique and genuinely hard-won.**
`EmbedderHttp::embed_batch:353-440` encodes: batch cap 8 (the TEI/SPLADE
server returns HTTP 422 above it), empty-string filtering with positional
re-expansion to stay aligned with the dense response, and 8-attempt
exponential backoff on 424/429/5xx. This is one deployment's operational
knowledge; it does not belong in a crate librarian also consumes.

**The cut line already exists in the code as a runtime flag.**
`src/retrieval/client.rs:43` sets `dense_only = lite || config.disable_sparse`,
and `embed_batch:329-353` branches on it: the `dense_only` arm is exactly
`dense_batch` in chunks of 8 with an empty `SparseVector` padded in. That arm
is a strict subset of what `RemoteEmbedder` already does. The refactor promotes
an existing runtime branch to a compile-time boundary.

**Measured cost of the status quo** (A/B on the real manifest, restored):

| build | today | with root reqwest/rustls gated |
|---|---|---|
| `--no-default-features` | 274 crates | **226** (−48) |
| default | 339 | 338 |

The 48 are the whole `hyper`/`h2`/`tower`/`tower-http`/`rustls`/`ring`/`webpki`/
`native-certs` stack, compiled by the two `--no-default-features` CI lanes
(`.github/workflows/ci.yml:46`, `:48`) for code those lanes do not run.
Corroborating signal: `remote-embed` currently measures at **+1 crate**. It
should measure ~48. It is cheap only because the stack is already
unconditionally linked — the feature table is lying, and that is the
fingerprint of the mis-gating.

## Alternatives considered

- **Split `embedder.rs` in place and `#[cfg]`-gate the HTTP half, stop there.**
  Buys the 48 crates for a contained change. Rejected as the *endpoint*: it
  makes the duplicate a permanent, well-partitioned resident of the root crate.
  Tidy duplication survives for years. Retained as **Stage 1** of the plan and
  as the fallback if Stage 2 stalls — it is a real waypoint, not a wrong turn.
- **Lift the sparse leg into `codescout-embed` too, retiring `EmbedderHttp`
  wholesale.** Rejected: the crate would widen to carry SPLADE/TEI batch caps
  and 424-retry policy for a single consumer, and librarian would compile
  knowledge it never uses.
- **Leave it.** Rejected: a plaintext-API-key policy duplicated across two
  files is one edit away from silent divergence, and the CI cost recurs on
  every run.
- **Namespace/abstract over both embedders behind a new root-side trait.**
  Rejected on this project's rule-of-three discipline
  (`tool-registration-rule-of-three`) — and unnecessary: the `Embedder`
  abstraction already exists in the crate with two implementations
  (`RemoteEmbedder`, local fastembed). This decision collapses a duplicate
  *into* an existing abstraction rather than inventing one.

## Mechanism

Four stages, each independently shippable, each with its own gate. Full task
breakdown in `docs/plans/2026-07-25-embedding-transport-consolidation.md`.

0. Reconcile three contracts that differ between the two clients (design only,
   no dependency changes) — connect-error marker, dim validation, query prefix.
1. Split `src/retrieval/embedder.rs`: vocabulary stays ungated, HTTP client
   moves behind `server-stack`. **This is where the 48 crates are banked.**
2. Replace the dense leg with `codescout_embed::RemoteEmbedder`, holding batch
   size at 8 so the change is behaviour-preserving.
3. Delete the duplicates and the root manifest entries.

## Consequences

- **Now easier:** one place to add a provider, change the plaintext-key policy,
  or install the crypto provider. `remote-embed` starts costing its true ~48
  crates, so the feature-cost table becomes honest and can serve as the
  regression test for this decision. The lean CI lanes stop compiling a TLS
  stack they never call. Root's dense path inherits the crate's retry,
  empty-input handling, response cap, and URL normalization — five real
  robustness gains, none of which root has today.
- **Now harder:** the connect-error contract (below) becomes a *cross-crate*
  string dependency and must be made explicit. `codescout-embed` gains a small
  amount of configurability it did not need before (explicit query-prefix
  override). One more crate boundary sits between a retrieval bug and its
  fix.
- **Neutral, worth stating:** none of this touches the LLM-facing surface. By
  the `agentic-surface-as-moat` weighting this is internal structure — do it
  deliberately, not urgently. The moat is not at risk either way.

### The three contracts that must be reconciled first

These are the reason Stage 0 exists as a design step rather than a coding step.

1. **Connect-error marker (cross-module today, cross-crate after).**
   `src/retrieval/embedder.rs:221` emits `"dense embed connect failed: {url} — …"`.
   `src/tools/semantic/semantic_search.rs:46` matches
   `err_str.contains("embed connect failed")` to route the user to the embedder
   hint instead of the misleading "check qdrant logs" fallback. Both sides carry
   regression tests (`embedder.rs:486-499`, `semantic_search.rs:495`). Moving the
   producer into `codescout-embed` promotes a substring contract to a crate
   boundary, where nothing makes the two tests fail together.
   **Resolution:** the crate publishes the contract — a typed
   `EmbedError::Connect { url }` variant, or at minimum a documented stable
   marker with its own crate-side test. The crate must *not* learn about
   `semantic_search.rs`; the dependency points one way.

2. **Dimension contract.** Root validates against a configured `expected_dim`
   and errors on mismatch; the crate discovers and caches `cached_dims`. Both
   have a bug scar behind them.
   **Resolution:** no change needed. The validation already lives in the
   *caller* (`embed_batch:338`, `:432`, `dense_query:257`), not in
   `dense_batch`, so it survives the swap untouched. Root keeps validating;
   the crate keeps discovering. No new abstraction, no widening.

3. **Query prefix — the silent one.** Root reads
   `CODESCOUT_QUERY_PREFIX`, defaulting to **empty** (`new():127`), and applies
   it query-side only (`dense_query:249`). The crate derives it from the model
   name — `query_prefix_for:105-111` returns the CodeRankEmbed prefix iff the
   model string contains `coderank`, else `None` — and applies it query-side
   only (`embed_query:340`). Same structure, different source of truth. They
   disagree in three ways, and every one of them fails silently as degraded
   recall rather than as an error:
   - model is CodeRank, env unset → root applies nothing, crate applies the
     prefix. **Swap is a REGRESSION.** *Corrected 2026-07-25:* the benchmark in
     `docs/manual/src/concepts/retrieval-stack.md` § Dense embedder rates
     Q4_K_M **no-prefix** the champion (37) and states verbatim "We default to
     Q4 no-prefix"; forcing the prefix drops to the f16+prefix tier (34), and
     "Q4 loses asymmetric subspace if a prefix is forced." An earlier draft of
     this ADR called root's no-prefix behaviour a latent recall bug — it is the
     benchmarked-correct configuration, and the crate's unconditional
     model-derived prefix is the defect.
   - model is CodeRank, env set to something custom → root honours the
     operator, crate overrides with its hardcoded string. Swap is a
     **regression in configurability**.
   - model is not CodeRank, env deliberately set → root applies it, crate
     applies nothing. Swap **silently drops operator config**. This is the
     dangerous one.
   **Resolution (strengthened 2026-07-25):** a two-state override is not enough.
   Because `query_prefix_for` *derives* from the model name, an
   `Option<String>` cannot distinguish "derive one" from "explicitly use none"
   — and on Q4, "explicitly none" is precisely the configuration the benchmark
   wants. `RemoteEmbedder` needs **three** states: *derive* (default),
   *explicit value*, and *explicitly suppressed*. Root must map an unset
   `CODESCOUT_QUERY_PREFIX` to **suppressed**, not to *derive* — otherwise the
   swap silently costs ~3 benchmark points on the project's default model, with
   no error and no failing test.

   Related and worth fixing in the same pass:
   `dense_model_name` defaults to the **empty string** (`new():126`), so root
   currently sends `{"input": [...], "model": ""}`. Tolerated by llama-server,
   rejected by stricter gateways.

## Change scenarios absorbed

- Add or swap a remote embedding provider — one implementation to change.
- Change the plaintext-API-key policy — one guard, not two kept in sync by a
  doc comment.
- Build with no HTTP embedding backend — the two `--no-default-features` CI
  lanes stop paying 48 crates.

Explicitly **not** absorbed: "run two different sparse backends", or "librarian
needs sparse". No data suggests either.

## Revisit-when

- Sparse embedding leaves the retrieval stack — then `EmbedderHttp`'s remaining
  justification disappears and the `server-stack` sparse module can be deleted
  outright rather than gated.
- A third consumer needs dense embedding — that forces the crate to own the
  dim contract too, and contract 2's "no change needed" resolution expires.
- `RemoteEmbedder`'s `BATCH_SIZE` is raised from the 8 that Stage 2 pins — that
  is a separate, measured change against a real embedding server, not a
  refactor detail.

## Confidence

**High** on the diagnosis: the duplication, the leaked law, the consumer split,
and the 48-crate measurement are all read from code or measured, not inferred.

**High** on Stages 0–1. Stage 1 is a mechanical module split whose gate
(`cargo check --no-default-features`) is unambiguous, and it banks the entire
measured win on its own.

**Stage 2 raised from medium to high (2026-07-25)**, after closing the gap this
section originally named. `RerankerHttp` has now been examined: it is a
TEI/Infinity cross-encoder with no counterpart in `codescout-embed`, and
`src/retrieval/search.rs:84` already skips it whenever `self.lite`. That closed
the last unknown and established a provable invariant:

> `!cfg(server-stack)` ⟹ `lite == true` ⟹ (`dense_only == true` ∧ reranker
> never invoked)

Because `qdrant_code_store` hard-bails under `#[cfg(not(feature =
"server-stack"))]` (`client.rs:65-71`) and `from_config_only` is itself
server-stack-gated (`:77`), a lean build can only reach
`VectorBackend::SqliteVec`. So in a lean build the sparse leg
(`embed_batch:353-440`) and the entire reranker are **already dead code** —
gating them is a no-op at runtime. Full proof chain in the plan.

One caveat keeps this short of certainty: that invariant is currently held up by
three files agreeing with each other and is pinned by nothing. Plan Task 1.0
adds the test before any stage depends on it.

**Medium** on Stage 3 alone — and for a different reason than this section
originally gave. `RerankerHttp` needs `reqwest` and has no crate counterpart, so
whether root sheds `reqwest` *entirely* or retains a minimal `server-stack`-only
dependency is an **open branch** (plan step 3.3), not a decided outcome. The ADR
does not pretend to have settled it.

Original wording of this section predicted "a fourth contract may exist in code
neither of us has read." It did — the reranker's runtime `lite` guard. Recorded
rather than quietly overwritten, because the prediction earning out is the
argument for Stage 0 existing at all.
## Sites (initial)

- `src/retrieval/embedder.rs` — vocabulary (`:5-27`, `:457-459`), dense client
  (`:119-441`), sparse leg (`:353-440`), connect-marker producer (`:221`)
- `src/retrieval/reranker.rs` — `RerankerHttp`, a TEI/Infinity cross-encoder
  with no `codescout-embed` counterpart; skipped at runtime whenever `lite`
  (`src/retrieval/search.rs:84`). The open branch in Stage 3
- `src/retrieval/client.rs` — `RetrievalClient`, `dense_only` derivation (`:43`)
- `src/retrieval/mod.rs` — `pub mod embedder` (`:7`) and `pub mod reranker`
  (`:14`), both currently ungated
- `src/tools/semantic/semantic_search.rs:46` — connect-marker consumer
- `src/agent/mod.rs:1743`, `src/migrate/memories.rs:43` — `EmbedderHttp` callers
- `src/lib.rs:10` — `install_default_crypto_provider` (deletable at Stage 3)
- `crates/codescout-embed/src/remote.rs` — `RemoteEmbedder`, `query_prefix_for`
  (`:105-111`), `is_https_or_loopback` (`:48-77`)
- `Cargo.toml` — `reqwest`, `rustls`, `[features]`

## References

- [Embedding transport consolidation plan](../plans/2026-07-25-embedding-transport-consolidation.md)
  (`0da5bd672ef60dfc`) — task breakdown
- [Dependency review session log](../trackers/archive/dependency-review-session-log-2026-08-25.md)
  (`228a7a2f4dc2378d`) — F-1, F-2, W-1 from the review that produced this ADR
- `docs/trackers/reconnaissance-patterns.md` — R-43 (the `#[cfg]`-from-grep miss
  that nearly shipped Stage 1 as a one-line manifest change)
- `docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md` — the lite/server split
  that `dense_only` implements
- `crates/codescout-embed/src/remote.rs` empty-input `bail!` cites
  `2026-05-17-reindex-embedding-dim-mismatch.md`
