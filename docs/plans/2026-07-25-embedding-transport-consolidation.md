---
id: '0da5bd672ef60dfc'
kind: plan
status: active
title: Embedding transport consolidation — codescout-embed owns remote HTTP
tags:
- architecture
- embedding
- dependencies
- codescout-embed
- server-stack
topic: embedding transport boundary
---

# Plan: Embedding transport consolidation

Implements `docs/adrs/2026-07-25-embedding-transport-boundary.md`.

**Goal.** `codescout-embed` owns remote embedding transport. `src/retrieval`
keeps the domain vocabulary and a `server-stack`-only sparse + rerank client.
Root drops `reqwest`, `rustls`, and `install_default_crypto_provider`.

**Measured payoff.** `--no-default-features`: 274 → 226 crates (−48, the whole
`hyper`/`h2`/`tower`/`rustls`/`ring` stack). Two CI lanes
(`.github/workflows/ci.yml:46`, `:48`) stop compiling it. Default build is
unchanged at 339 — the win is CI time and manifest honesty, not shipped binary
size. Do not oversell this to reviewers.

---

## The load-bearing invariant (verified 2026-07-25)

Everything below rests on one property. It is currently true, and currently
maintained by three files agreeing with each other by accident — nothing pins
it.

> **`!cfg(server-stack)` ⟹ `lite == true` ⟹ (`dense_only == true` ∧ reranker never invoked)**

Proof chain, each link read this session:

1. `src/retrieval/client.rs:65-71` — under `#[cfg(not(feature = "server-stack"))]`,
   `qdrant_code_store` is `anyhow::bail!("CODESCOUT_VECTOR_BACKEND=qdrant
   requires the `server-stack` build feature…")`.
2. `src/retrieval/client.rs:77` — `from_config_only` is itself
   `#[cfg(feature = "server-stack")]`, so `from_env` is the *only* constructor
   in a lean build.
3. `from_env:31-36` routes `VectorBackend::Qdrant` to that bail. Therefore a
   lean build can only reach `VectorBackend::SqliteVec`, so `lite = true` (`:32`).
4. `client.rs:43` — `dense_only = lite || config.disable_sparse` → `true`.
5. `src/retrieval/search.rs:84` — `if !opts.rerank || self.lite || candidates.is_empty()`
   returns before `self.reranker.rerank(...)` is ever called (`:90`).

**Consequence:** in a lean build the sparse leg (`embed_batch:353-440`) and the
entire reranker are already dead code. Gating them is a no-op at runtime.

**Task 1.0 pins this with a test before anything depends on it.** An invariant
this load-bearing must not stay implicit across three files.

---

## Stage 0 — Reconcile contracts (design only, no dependency changes)

No code moves in this stage. Output is three decisions written into the ADR.

### 0a — Connect-error marker

**Problem.** `src/retrieval/embedder.rs:221` emits `"dense embed connect failed:
{url} — …"`. `src/tools/semantic/semantic_search.rs:46` matches
`err_str.contains("embed connect failed")` to route the user to the embedder
hint instead of the misleading "check qdrant logs" fallback. Regression tests
on both sides: `embedder.rs:486-499`, `semantic_search.rs:495`. Stage 2 promotes
this substring contract to a *crate* boundary, where nothing makes the two tests
fail together.

**Decide:** typed `EmbedError::Connect { url }` in `codescout-embed`, or a
documented stable marker with a crate-side test asserting it.
**Constraint:** the dependency points one way. `codescout-embed` must not learn
that `semantic_search.rs` exists.

### 0b — Dimension contract

**Resolved, no work.** Validation already lives in the *callers*
(`embed_batch:338`, `:432`; `dense_query:257`), not in `dense_batch`, so it
survives the swap untouched. Root keeps validating against configured
`expected_dim`; the crate keeps discovering into `cached_dims`. No new
abstraction, no crate widening. Recorded here so a later reader does not
re-litigate it.

### 0c — Query prefix (the silent one)

**Problem.** Root reads `CODESCOUT_QUERY_PREFIX`, defaulting to **empty**
(`new():127`), applied query-side only (`dense_query:249`). The crate derives
from the model name — `query_prefix_for:105-111` returns the CodeRankEmbed
prefix iff the model contains `coderank`, else `None` — applied query-side only
(`embed_query:340`). Same structure, different source of truth. All three
disagreements fail as degraded recall, never as an error:

| model | `CODESCOUT_QUERY_PREFIX` | root today | crate | swap effect |
|---|---|---|---|---|
| CodeRank | unset | *nothing* | prefix | **REGRESSION** — see correction below |
| CodeRank | custom | custom | hardcoded | **regression** — operator overridden |
| other | set deliberately | applied | *nothing* | **regression** — config silently dropped |

**Correction 2026-07-25 (row 1 flipped).** `docs/manual/src/concepts/retrieval-stack.md`
§ Dense embedder benchmarks CodeRankEmbed **Q4_K_M with no prefix at 37 (champion)**
vs f16+prefix at 34, and states "Q4 loses asymmetric subspace if a prefix is
forced" / "We default to Q4 no-prefix." So root applying *nothing* is correct on
the default model, and the crate's unconditional model-derived prefix is the
defect. An earlier draft of this plan had row 1 as a fix; it is a regression.

**Decide:** a two-state override is insufficient — `Option<String>` cannot express
"explicitly no prefix" distinctly from "derive from model name", and on Q4 the
former is what we want. `RemoteEmbedder` needs **three** states: *derive*,
*explicit value*, *explicitly suppressed*. Root maps unset `CODESCOUT_QUERY_PREFIX`
→ **suppressed**. Blocking for Stage 2.

**Field note (2026-07-25):** all four active env profiles (`.env`, `.env.amd`,
`.env.cpu`, `.env.gpu`) were setting the prefix, contradicting the benchmark;
`.env.lite` already had it commented out. Now commented out in all four with the
benchmark citation. A running codescout MCP server keeps the old exported value
until restarted — `/mcp` reconnect required for the change to take effect.

**Same pass:** `dense_model_name` defaults to the **empty string**
(`new():126`), so root sends `{"input": […], "model": ""}` today. Tolerated by
llama-server, rejected by stricter gateways. Decide whether the crate's
required-model contract is adopted (preferred) or an empty model stays legal.

**Gate:** all three written into the ADR's *Consequences* section. No code.

---

## Stage 1 — Split the module, gate the HTTP half

> **SHIPPED 2026-08-28 — but NOT as written below. Read `ET-7` before this section.**
>
> Six defects were found executing this stage; five were still standing in the text
> below when execution began. Full account, with evidence, in
> `resume-embedding-transport-stages-1-3:ET-7`. The load-bearing ones:
>
> - **1.2's sibling row is wrong the same way 1.2 was.** `RerankerHttp` → `server-stack`
>   breaks the lean build *and* the default build: `search.rs:154` references
>   `self.reranker` from ungated `search_in`, so the type must exist in every
>   configuration. "Never invoked in a lean build" is a *runtime* invariant
>   (`should_rerank` gates on `lite`) and does not remove a code path. **Correct gate:
>   `remote-embed`**, for the reranker and for everything else in this stage.
> - **1.5/Task 6 is wrong.** `server-stack` is tonic/gRPC and never touches `reqwest`.
>   `reqwest`/`rustls` go under `remote-embed` **only**; `server-stack` gains
>   `"remote-embed"` as a feature dependency, because `from_config_only` is
>   `server-stack`-gated yet constructs both `EmbedderHttp` and `RerankerHttp`.
>   F-5's earlier `any(remote-embed, server-stack)` correction is therefore moot:
>   the implication makes plain `remote-embed` sufficient.
> - **1.1's surface list omits five items**, not the three its own note admits — add
>   `DEFAULT_INFLIGHT` and `embed_chunks_ordered` (both `dead_code` otherwise), all
>   three top-of-file `use` lines, and `is_https_or_loopback` (which this plan wrongly
>   asserts stays ungated — its only consumer chain is HTTP-only).
> - **1.0 prescribes `EnvGuard` / `serial_test`** "per `docs/conventions/test-env-isolation.md`".
>   That convention **bans both crate-wide**. ET-1 avoided env mutation entirely.
> - **The file split did not happen, deliberately.** The HTTP items are interleaved with
>   ungated ones, so a split meant hand-moving ~1,300 lines. Gated in place with 15
>   `#[cfg]` attributes; `cargo check --no-default-features` proves completeness.
>
> Measured outcome: **bare 274 → 226 (−48)**, `remote-embed` **+1 → +49**. This also
> closes `ET-6` (re-measure before quoting the −48 figure).

**This stage banks the entire 48-crate win on its own.** If Stages 2–3 never
happen, this is still worth shipping — but the ADR records it as a waypoint,
not the answer.

- **1.0** Add the invariant test. In `src/retrieval/client.rs` tests, assert
  that under `#[cfg(not(feature = "server-stack"))]` a `from_env` with
  `CODESCOUT_VECTOR_BACKEND=qdrant` errors, and that a successful lean
  `from_env` yields `lite == true` and `dense_only == true`. Use `EnvGuard` /
  `serial_test` per `docs/conventions/test-env-isolation.md`.
- **1.1** In `src/retrieval/embedder.rs`, keep ungated: `SparseVector`,
  `EmbedOutput`, `BatchEmbedder` (`:5-27`), `DenseEmbedder` (`:457-459`).
  These are the five vocabulary-only importers' entire surface — `sync.rs`,
  `code_store.rs`, `sqlite_code_store.rs`, `qdrant.rs`, `tools/memory/tests.rs`.
- **1.2** Move `EmbedderHttp` + `is_https_or_loopback` + the **five** wire
  structs + `HttpDenseEmbedder` (`:30-34`, `:39-441`, `:463-480`) into a new
  `src/retrieval/embedder/http.rs`, gated `#[cfg(feature = "server-stack")]`.
  **Correction (F-4):** `:92-117` holds five structs, not four, and only three
  are dense/OpenAI. `EmbedReq` (`:92-94`, TEI-shaped `inputs`, used by
  `embed:277`) and `SparseEntry` (`:114-117`, used by `embed:293,303` and
  `embed_batch:372,386,412`) are **sparse-side**. All five move here safely
  because their consumers (`EmbedderHttp::embed` / `embed_batch`) move with
  them — but the distinction is load-bearing in 3.1, which *deletes*.
- **1.3** Gate `pub mod reranker;` (`src/retrieval/mod.rs:14`) on `server-stack`,
  and make `RetrievalClient`'s `reranker` field (`client.rs:17`) plus its bare
  `use crate::retrieval::reranker::RerankerHttp;` (`client.rs:6`)
  cfg-conditional alongside it. No runtime path is lost: `search_in`
  short-circuits on `lite` at `search.rs:84` before the reranker is reached, and
  `src/tools/memory/mod.rs:412` sets `rerank: true` in `SearchOpts` but only
  reaches the reranker through `search_in`.

  **Do NOT gate `pub mod client;` — corrected 2026-07-25 (F-3, high).** An
  earlier draft gated it alongside `reranker`. That breaks three ways:

  1. `RetrievalClient::from_env` has **14 ungated consumers**, two of them in
     ungated *sibling* modules — `src/retrieval/search.rs:3` (bare `use`, and
     `pub mod search;` is ungated at `mod.rs:15`) and `src/retrieval/sync.rs:195`
     (ungated inherent `impl`). The rest: `tools/semantic/index.rs:124,314,410`,
     `tools/semantic/semantic_search.rs:223`, `tools/memory/mod.rs:402`,
     `tools/config/mod.rs:330,425`, `tools/onboarding.rs:744`,
     `agent/mod.rs:1581,1742`, `main.rs:269,301`, `dashboard/api/index.rs:14`.
     Gating a `pub mod` is a subtree delete, so `--no-default-features` fails at
     every one.
  2. It contradicts this plan's own load-bearing invariant, whose proof chain
     (steps 1-4) requires `from_env` to exist and be reachable under
     `not(server-stack)`. Gating `client` makes the invariant vacuous rather
     than proven.
  3. **It would delete Task 1.0's test from every configuration.** 1.0 puts a
     `#[cfg(not(feature = "server-stack"))]` test *inside* `client.rs`; gate the
     module on `server-stack` and the test is absent under `not(server-stack)`
     (file not compiled) and excluded under `server-stack` (cfg false). It
     compiles in **zero** configs, `cargo test` stays green, and the invariant
     is recorded as pinned while guarded by nothing. This is the expensive
     failure — silent, green, and wrong.

  Keeping `client` ungated preserves the invariant proof, keeps 1.0's test
  compilable in the lean build it guards, and leaves all 14 consumers untouched.
- **1.4** Gate `src/lib.rs:10` `install_default_crypto_provider` body on
  `feature = "server-stack"` alone — **not** `any(server-stack, remote-embed)`
  (corrected 2026-07-25, F-5). `rustls::` appears exactly once in root
  (`src/lib.rs:14`), and 1.5 puts `dep:rustls` under `server-stack` only, so the
  `remote-embed` disjunct would compile the body in a configuration where
  `rustls` is not linked — `--no-default-features --features remote-embed`,
  which the verification script below exercises (`cargo tree` will not surface
  it; only `cargo check` will). The disjunct is also unnecessary: under
  `remote-embed` root delegates to `codescout_embed::RemoteEmbedder`, which
  installs its own provider at `crates/codescout-embed/src/remote.rs:84,96`.

  Leave all **four** callers unchanged so the fn degrades to a no-op rather than
  disappearing: `agent/mod.rs:383`, `main.rs:226`, plus the two an earlier draft
  omitted — `retrieval/reranker.rs:79` (in `RerankerHttp::new`) and
  `retrieval/embedder.rs:168` (in `EmbedderHttp::with_config`). The latter two
  move or gate with 1.2 / 1.3, so they need path fixups during the move anyway.
- **1.5** Make `reqwest` and `rustls` `optional = true` in `Cargo.toml`;
  add `dep:reqwest`, `dep:rustls` to the `server-stack` feature.

**Gate:** `cargo check --no-default-features` passes; `cargo test` passes;
`cargo clippy -- -D warnings` clean. Then re-run the feature-delta measurement
(below) and confirm 274 → 226.

---

## Stage 2 — Swap the dense leg to `codescout_embed::RemoteEmbedder`

- **2.0** Implement the 0a and 0c decisions in `crates/codescout-embed`.
- **2.1** Replace `EmbedderHttp::dense_batch` / `dense_query` internals with
  calls into `RemoteEmbedder`. **Hold batch size at 8**, not the crate's
  `BATCH_SIZE = 32` — the 8 exists to match the sparse server's HTTP 422 cap
  (`embed_batch:346`), and in `dense_only` mode it is simply the incumbent
  behaviour. Raising it is a separate, measured change against a real server.
- **2.2** Root inherits, for free, five things it does not have today: retry
  (3× 500 ms doubling, 5xx only), empty-input filtering with zero
  reconstruction, a 32 MiB response cap, `from_url` normalization, and the
  cached-dims path. Add regression tests for retry and empty-input on the
  retrieval path — the crate's tests cover the crate's callers, not root's.

**Gate:** the two connect-marker regression tests still pass, now spanning the
crate boundary via 0a. `retrieval-e2e` if a live stack is available; otherwise
say so explicitly rather than claiming coverage.

---

## Stage 3 — Delete the duplicates

- **3.1** Delete root `is_https_or_loopback` (`:39-68`) and the **three**
  dense/OpenAI wire structs, **by name**: `OpenAiEmbedReq` (`:97-100`),
  `OpenAiEmbedResp` (`:103-105`), `OpenAiEmbedItem` (`:108-111`).

  **Keep `EmbedReq` (`:92-94`) and `SparseEntry` (`:114-117`)** — corrected
  2026-07-25 (F-4). The sparse leg stays in root (explicitly out of scope) and
  Stage 2 replaces only `dense_batch` / `dense_query` internals, so
  `embed`/`embed_batch` still serialize `EmbedReq` at one site and deserialize
  `Vec<Vec<SparseEntry>>` at four. An earlier draft said "delete the four OpenAI
  wire structs (`:92-117`)", which is five structs and breaks the sparse leg.
  Cite deletion targets by **name**, never by line range.
- **3.2** Delete `src/lib.rs::install_default_crypto_provider` and its **four**
  call sites — `agent/mod.rs:383`, `main.rs:226`, `retrieval/reranker.rs:79`,
  `retrieval/embedder.rs:168` (an earlier draft said three, corrected
  2026-07-25, F-5); `codescout-embed` installs its own (`remote.rs:84`).
- **3.3** Remove `reqwest` and `rustls` from root `[dependencies]` entirely —
  including from `server-stack`, if `RerankerHttp` is the last holdout, decide
  then whether the reranker moves to the crate or keeps a minimal root reqwest
  under `server-stack`. **This is an open branch in the plan, not a decision.**

**Gate:** `remote-embed` shows its true delta (~48 crates, not `+1`) in the
feature table. That number is the regression test for the whole ADR.

---

## Verification: the feature-delta measurement

Re-run after Stages 1 and 3. This is the objective gate; run it, do not
estimate it.

```bash
count() { cargo tree --edges normal --no-default-features $1 \
            --prefix none --format '{p}' 2>/dev/null | sort -u | wc -l; }
base=$(count "")
for f in librarian http remote-embed server-stack dashboard; do
  echo "$f: $(count "--features $f")  (base $base)"
done
```

**Baseline, 2026-07-25 (before any change):**

| feature | delta over bare 274 |
|---|---|
| `local-embed` | +160 |
| `server-stack` | +49 |
| `librarian` | +47 (`jsonschema` is 28 of it) |
| `http` | +17 |
| `remote-embed` | **+1** ← the bug |

**Target after Stage 3:** bare ≈ 226, `remote-embed` ≈ +48.

Method note: measure by A/B-ing the real manifest and `comm -23` on sorted
`cargo tree` output, restoring from a backup with `git status --porcelain` as
the exit gate. Do **not** compose per-package closures — `cargo tree -p X
--no-default-features` applies the flag to `X`, not the root, and returns
degenerate results (`docs/trackers/archive/dependency-review-session-log-2026-08-25.md` F-2).

---

## Risks

| Risk | Mitigation |
|---|---|
| The invariant breaks later — someone makes a lean build reach a non-lite path | Task 1.0 pins it with a test before anything depends on it |
| 0c gets skipped as "just a prefix" and recall degrades silently | It fails as *worse search results*, never as an error. No test catches it. Treat 0c as blocking for Stage 2, not optional |
| Connect-marker contract rots across the crate boundary | 0a makes it a published, crate-tested contract rather than an incidental string |
| Stage 2 lands without a live embedding server to test against | State that plainly in the PR. Do not claim `retrieval-e2e` coverage that did not run |
| Scope creep into lifting sparse/rerank into the crate | Explicitly out of scope — see below |

## Out of scope

- Lifting the sparse leg or the reranker into `codescout-embed`. Both encode
  one deployment's TEI/SPLADE/Infinity operational knowledge; librarian would
  compile what it never uses.
- Raising `BATCH_SIZE` from 8 to 32. Separate, measured, needs a real server.
- The `jsonschema` carve-out (default 339 → 312). Independent finding from the
  same review; unrelated boundary.
- Anything touching the LLM-facing surface. By `agentic-surface-as-moat` this
  work is internal structure — deliberate, not urgent.

## References

- [ADR: embedding transport boundary](../adrs/2026-07-25-embedding-transport-boundary.md)
  (`74c1aa5018287728`) — the decision
- [Dependency review session log](../trackers/archive/dependency-review-session-log-2026-08-25.md)
  (`228a7a2f4dc2378d`) — F-1, F-2, W-1, and the pre-dispatch reconnaissance
  corrections applied to Stage 1 + Stage 3 on 2026-07-25: **F-3** (high — do not
  gate `pub mod client;`), **F-4** (wire-struct inventory), **F-5**
  (crypto-provider gate vs `dep:rustls` placement), **W-2** (the scout itself)
- `docs/trackers/reconnaissance-patterns.md` — R-43 (read-side: gating claims
  from a grep hit), R-44 (write-side: enumerate the consumer set before
  accepting a proposed `#[cfg]` gate)
- `docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md` — the lite/server split
  that `dense_only` implements
- `docs/conventions/test-env-isolation.md` — `EnvGuard` for Task 1.0
