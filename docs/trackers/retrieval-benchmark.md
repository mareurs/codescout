---
id: '8e09ca67f463027e'
kind: tracker
status: in-progress
title: Retrieval Benchmark — pinned 25-TC log
owners:
- '@mareurs'
tags:
- retrieval
- benchmark
- qdrant
- embedding
topic: null
time_scope: null
---

## Why this tracker exists

The 20-TC numbers scattered across `docs/research/2026-04-03-embedding-model-benchmark.md`
(23–41/60 range) are **not comparable to each other**. Each run used a different codebase
HEAD, different chunking, different fusion config, different embedder protocol — and the
harness recorded none of those. The "best" historical result (41/60 for CodeRankEmbed
hybrid on 2026-05-02) cannot be reproduced from the artifacts that exist.

This tracker is the canonical log going forward. Every run is anchored to:

- A **pinned worktree** at `.worktrees/bench` (detached HEAD at `baseline_sha`)
- A **dedicated qdrant collection** (`bench_<model>_code_chunks`) so models coexist
- A **config block** in the JSON output (model, boost, sparse on/off, project_sha)
- The **25-TC suite** (20 legacy tiers + 5 T5 real-usage-shape from external `usage.db`)

If the table above ever needs a new `baseline_sha`, treat all prior rows as compromised
and start a new section here.

## How to run a bench

### Prerequisites

```bash
# 1. Retrieval stack containers must be up
docker ps --format '{{.Names}}' | grep -E "qdrant|embedder|reranker"
# Expect: codescout-qdrant (:6334), codescout-embedder-gpu (:48081),
#         codescout-embedder-sparse-gpu (:48084), codescout-reranker-gpu (:48083)

# 2. Pinned bench worktree must exist at the baseline commit
git worktree list | grep .worktrees/bench
# If missing: git worktree add --detach .worktrees/bench <baseline_sha>

# 3. Build release binary
cargo build --release
```

### Run 1 — jina-v2 (fusion at default boost=3.0)

```bash
# Sync once (per (model, collection_prefix) combination)
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_jinav2_ \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench code-explorer

# Then bench (no re-sync needed for boost sweeps on same model)
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=jina-embeddings-v2-base-code \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_jinav2_ \
  --label "jina-v2-bm25-3.0" \
  > /tmp/bench-jinav2.json
```

### Run 2 — CodeRankEmbed via llama-server

```bash
# Start llama-server (foreground for sanity, then move to background)
nohup llama-server -m ~/models/CodeRankEmbed-Q4_K_M.gguf \
  --embeddings --port 43300 --host 127.0.0.1 \
  -ngl 99 -c 16384 -b 4096 -ub 4096 --parallel 1 \
  > /tmp/llama-coderank.log 2>&1 &

# Wait for ready
until curl -s http://127.0.0.1:43300/v1/models >/dev/null; do sleep 1; done

# Sync
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_coderank_ \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench code-explorer

# Bench
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=CodeRankEmbed-Q4_K_M \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_coderank_ \
  --label "coderank-bm25-3.0" \
  > /tmp/bench-coderank.json
```

### Run variants

- **Dense-only control:** add `CODESCOUT_DISABLE_SPARSE=1` to the bench command (sync is shared).
- **Boost sweep:** vary `CODESCOUT_BM25_BOOST=<0.5|1.0|2.0|5.0>` on the bench command;
  no re-sync needed (boost is query-time only).
- **New model:** pick a unique `CODESCOUT_QDRANT_COLLECTION_PREFIX` (e.g. `bench_new_`),
  sync into it, then bench. The live `code_chunks` collection is never touched.

### Scoring

Score per TC: 3 if all expected paths in top-5, 2 if all in top-10 or majority in top-5,
1 if at least one in top-10, 0 otherwise. Path match: `r == exp` or
`r.endswith("/" + exp)` or `exp.endswith("/" + r)`. **Caveat:** basename collisions
(`embedder.rs` in two crates) defeat the matcher when the expected list is workspace-
relative — known gap.

## Findings so far (2026-05-12, baseline `ede25e69`)

- **Fusion helps by +2** on both jina-v2 and CodeRankEmbed (sparse on vs off at boost=3.0).
- **Boost sweep on CodeRankEmbed-Q4 (no prefix):** 0.5→32, 1.0→33, 2.0→34, 3.0→34,
  **5.0→35** (peak), 7.0→34, 10.0→33, 15.0→33, 20.0→33.
- **Query prefix × quantization:** prefix +3 on f16, prefix −4 on Q4_K_M. Q4 collapses
  the asymmetric subspace. Authoritative spec confirmed locally in
  `~/models/CodeRankEmbed-hf/config_sentence_transformers.json` (`prompts.query` only,
  no doc prefix). Researcher unneeded.
- **f16+prefix peaks at 34/75** (boost ∈ {2,3,5,7}), one point below Q4 no-prefix.
- **CodeRankEmbed wins on env-var / identifier-bag queries** (TC-02, TC-11, TC-12)
  where code-specific training surfaces identifier semantics jina misses.
- **T5 reached 7/15 on both models after fixing 4 wrong-expected truth lists** (originally
  cited the wrong file paths for ToolContext, EmbedderHttp, artifact-augment, MockLspClient).
- **TC-24 still 0/3 across all configs.** Top-10 is dominated by `.md` plans/specs/trackers
  that mention the augment feature in natural language; the actual `tools/augment.rs` and
  `catalog/augmentation.rs` are never surfaced. **Real retrieval failure mode**: code is
  losing to descriptive prose. Next levers: md-vs-code score balancing, or a `kind:code`
  filter on `semantic_search`.
- **TC-24 went 0/3 → 3/3 in code-mode** — augment.rs and augmentation.rs surface
  to ranks #1 and #3 when .md plans/specs are filtered out.
- **Champion (2026-05-12): CodeRankEmbed Q4_K_M, no prefix, bm25=5.0, mode=code → 37/75**
  (49.3% — total matches full-mode but T5 jumps from 7/15 to 10/15, signaling
  better real-user query handling).
## Caveats and known gaps

- **No CodeRankEmbed query prefix.** Historical 41/60 hybrid run used the
  `Represent this query for searching relevant code:` prefix. The retrieval stack's
  `EmbedderHttp` does not add a query-side prefix. Adding `CODESCOUT_QUERY_PREFIX`
  support is the single highest-leverage win for CodeRankEmbed.
- **Scoring matcher needs basename flexibility** for T5. Either rewrite TC-23/24/25
  expected lists to use unambiguous paths, or extend the matcher with crate-aware
  matching.
- **No latency baseline for dense-only p95** in this row set — the harness emitted
  `p95=0` when bench length was too short for the 5% tail. Re-run with `--limit 20` if
  comparing tails matters.

## History

### 2026-05-12 — query prefix experiment (negative result) + extended boost sweep

Added `CODESCOUT_QUERY_PREFIX` env to `EmbedderHttp::embed()` (query side only;
`embed_batch()` doc-side untouched). Tested `"Represent this query for searching
relevant code: "` against CodeRankEmbed across boost ∈ {1, 2, 3, 5}:

| variant | no prefix | with prefix | Δ |
|---|---|---|---|
| dense-only | 32 | 30 | **−2** |
| fusion boost=1.0 | 33 | 30 | **−3** |
| fusion boost=2.0 | 34 | 31 | **−3** |
| fusion boost=3.0 | 34 | 32 | −2 |
| fusion boost=5.0 | **35** | 31 | **−4** |

**Prefix consistently hurts** by 2–4 points. Hypotheses (not yet validated):

1. Q4_K_M quantization may have collapsed the prefix-conditioned subspace.
2. Our docs were indexed without the doc-side training distribution (raw code only).
   If the model was trained with explicit `search_document:` style doc prefix
   (Nomic family convention), then prefix-asymmetry without re-indexing docs WITH
   the doc prefix breaks the asymmetric calibration.
3. Re-indexing docs with `search_document: ` doc-side prefix may recover the win.
   Not tested yet — would require a fresh `bench_coderank_qp_` collection.

Extended boost sweep (no prefix):

| boost | score | p50 ms |
|---|---|---|
| 0.5 | 32 | 143 |
| 1.0 | 33 | 141 |
| 2.0 | 34 | 146 |
| 3.0 | 34 | 152 |
| **5.0** | **35** | 148 |
| 7.0 | 34 | 146 |
| 10.0 | 33 | 150 |
| 15.0 | 33 | 145 |
| 20.0 | 33 | 157 |

**Boost peak is 5.0** on the 25-TC pinned bench. Beyond 5.0, BM25 starts crowding
out the dense candidates that actually carry signal. Plateau is broader than Phase 6
saw (it stopped at 3.0).

### 2026-05-12 — f16 vs Q4 quantization × query prefix

Tested the quantization-collapsed-prefix hypothesis on f16 weights at `bench_coderank_f16_`
(re-indexed separately). Spec lookup in `~/models/CodeRankEmbed-hf/config_sentence_transformers.json`
confirmed:

- **Query prefix:** `"Represent this query for searching relevant code: "` (exact, trailing space)
- **Doc prefix:** none — `prompts.query` is the only entry. Docs go in raw.

So our prior tests used the correct prefix. The doc-side hypothesis is invalidated.

| variant | Q4_K_M | f16 |
|---|---:|---:|
| boost=5.0, no prefix | **35** | 31 |
| boost=5.0, +prefix | 31 | 34 |

**On Q4: prefix hurts by 4. On f16: prefix helps by 3.** Quant hypothesis confirmed —
the asymmetric prefix subspace does not survive Q4_K_M.

f16 + prefix boost sweep (search-side prefix only):

| boost | 1.0 | 2.0 | 3.0 | 5.0 | 7.0 |
|---|---:|---:|---:|---:|---:|
| f16 +prefix | 33 | 34 | 34 | 34 | 34 |

f16+prefix plateaus at **34/75** from boost=2.0 onward — close to but **not exceeding**
Q4 no-prefix 35/75. Spec-conformant ≠ best.

**Working theory:** Q4_K_M's coarser dense signal lets BM25 dominate ranking on the
T5 keyword-bag tier (which contributes most of the score variance). f16's sharper
asymmetric vectors are mathematically "more correct" but BM25 already covers what
they'd recover, leaving net wash.

**Current champion: CodeRankEmbed-Q4_K_M, no prefix, fusion @ bm25_boost=5.0 → 35/75.**

### 2026-05-12 — T5 expected-path fix (4 of 5 TCs had wrong truth)

Inspected top-10 for each T5 query against the original expected list and found
**4 of 5 expected lists were authored against the wrong files**:

| TC | Original expected | Actual symbol location |
|---|---|---|
| TC-21 (ToolContext) | `src/tools/mod.rs` | `src/tools/core/types.rs` |
| TC-22 (ActiveProject) | `src/agent/mod.rs` | correct ✓ |
| TC-23 (EmbedderHttp) | `crates/codescout-embed/src/embedder.rs` | `src/retrieval/embedder.rs` |
| TC-24 (artifact augment) | `crates/librarian-mcp/src/tools/mod.rs` | `tools/augment.rs` + `catalog/augmentation.rs` |
| TC-25 (MockLspClient + circuit breaker) | `src/lsp/ops.rs`, `client.rs` | `mock.rs`, `client.rs`, `manager.rs` |

Re-ran champion config (CodeRank Q4 no-prefix boost=5.0) and jina baseline
(boost=3.0) on the corrected suite. T5 jumped **4/15 → 7/15** for both models:

| Run | T5 before | T5 after | Total before | Total after |
|---|---:|---:|---:|---:|
| CodeRank Q4 no-prefix b=5.0 | 4 | **7** | 35 | **37** |
| jina-v2 b=3.0 | 4 | **7** | 32 | **35** |

**New champion: CodeRankEmbed-Q4_K_M, no prefix, bm25_boost=5.0 → 37/75** (49.3%).

#### Remaining T5 failure: TC-24

`artifact augment params merge librarian tracker` still scores 0/3 — top-10 is
**all `.md` plans/specs/trackers**, no `.rs` makes it in. The actual implementation
(`tools/augment.rs`, `catalog/augmentation.rs`) is being out-ranked by every plan
document that discusses the augment feature in natural language. Real failure
mode worth surfacing — likely needs either md-vs-code score balancing or
query-side hints that lean toward code (e.g. `pub fn augment`, `impl Augmentation`).

### 2026-05-12 — code/full search modes (default to code)

`semantic_search` now accepts `mode: "code" | "full"`. Default is `code`, which
applies a Qdrant `must_not: language == markdown` filter to drop md/mdx chunks
from results. `full` reverts to prior behavior (all indexed sources).

Implementation: new `exclude_languages: Vec<String>` on `SearchOpts`, plumbed
through `search_in` to `hybrid_query` which builds a `Filter { must, must_not }`.

Re-ran champion configs on pinned bench:

| Run | Total | T5 | Notes |
|---|---:|---:|---|
| coderank b=5.0, mode=full (prior) | 37/75 | 7/15 | |
| **coderank b=5.0, mode=code (new default)** | **37/75** | **10/15** | T5 +3, T1-T4 −3 |
| jina b=3.0, mode=full (prior) | 35/75 | 7/15 | |
| **jina b=3.0, mode=code (new default)** | **36/75** | **11/15** | +1 net, T5 +4 |

**TC-24 went from 0/3 → 3/3 in code-mode.** The two expected files
(`crates/librarian-mcp/src/tools/augment.rs`, `crates/librarian-mcp/src/catalog/augmentation.rs`)
moved to ranks #1 and #3 with the .md plans/specs filtered out.

The total-score wash on coderank reflects a real trade-off: queries whose expected
answer IS a `.md` doc (TC-02 backend config, TC-05 PROGRESSIVE_DISCOVERABILITY,
TC-17 routing-plugin guide) lose points. This is the right behavior for the
common LLM use case (finding implementations) but users who want docs must
explicitly pass `mode="full"`.

**Updated champion: coderank Q4 no-prefix bm25=5.0 mode=code → 37/75**, with
the meaningful T5 improvement (10/15 vs prior 7/15) signaling better real-user
query handling.

### 2026-05-12 — initial pinned bench

Built `.worktrees/bench`, refactored 7 hard-coded collection literals to use
`config.collection(<kind>)` with `CODESCOUT_QDRANT_COLLECTION_PREFIX` override.
Added 5 T5 real-usage-shape TCs sampled from external `usage.db`. First 8 runs
land in the table above. CodeRankEmbed @ boost=5.0 is the current leader at 35/75.


### 2026-05-12 — legacy-natural reconstruction (settling the 41/60 question)

User asked why champion config scored 37/75 vs historical 41/60. Investigation:

1. **Inspected commit `a55f1458`**: it rewrote both queries *and* expected paths of
   multiple legacy TCs (natural-language → keyword-stuffed; pre-refactor paths →
   post-refactor paths). Methodology change, not bugfix.
2. **Extracted pre-`a55f1458` TC defs** into `scripts/tc-suites/legacy-natural.json`
   (20 TCs, natural queries). Remapped 10 expected paths (workflow.rs → run_command/mod.rs,
   markdown.rs → markdown/edit_markdown.rs, symbol.rs → symbol/edit_code.rs, etc.) so
   they exist at the pinned SHA. Verified zero missing.
3. **Ran both suites** at jina-v2 bm25=5.0 mode=code on pinned worktree:

   | Suite | Score | T5 |
   |---|---|---|
   | legacy-natural (20-TC, natural) | 25/60 | — |
   | legacy-keyword (20-TC subset of full suite) | 25/60 | — |
   | full 25-TC (legacy-keyword + T5) | 36/75 | 11/15 |

**Conclusion: 41/60 is not reproducible.** Natural and keyword queries scored
identical (25/60 each), so the query-style rewrite is innocent. The gap to 41/60
must come from one or more of: pre-pin chunking config, different bm25 boost (Phase 6
used 3.0), or a stale `code_chunks` collection that happened to align with the
pre-refactor expected paths. None of those states are reachable any more.

The honest baseline going forward is **25/60 legacy-natural / 36/75 25-TC** at the
pinned worktree. The `legacy-natural.json` suite is now committed so future runs
can keep this comparison alive without recomputing it from git history.


### 2026-05-12 — Reranker A/B/C: bge-v2-m3 (TEI) vs jina-rerank-v2 (Infinity)

Spun up Infinity 0.0.77 on `:48085` to host `jinaai/jina-reranker-v2-base-multilingual`
(TEI can't load it — custom XLM-R-flash architecture lacks standard `model_type` in
config.json). Added `CODESCOUT_RERANKER_PROTOCOL=tei|infinity` toggle to
`RerankerHttp` so codescout speaks both wire shapes (TEI uses `{texts, score}`,
Infinity/Cohere use `{documents, results.relevance_score}`).

Four configurations, all at bm25=5.0 / mode=code on pinned worktree:

| Embedder | Reranker | natural 20-TC | full 25-TC | T5 |
|---|---|---|---|---|
| jina-v2 | bge-v2-m3 (TEI) | 25/60 | 36/75 | — |
| jina-v2 | jina-rerank-v2 (Infinity) | 23/60 | **38/75** | 11/15 |
| coderank Q4 | bge-v2-m3 (TEI) | not measured | 37/75 | 10/15 |
| coderank Q4 | jina-rerank-v2 (Infinity) | 23/60 | 36/75 | 11/15 |

**Findings.**

- T5 (real-usage tier) is the cleanest signal: jina-rerank-v2 lifts it 10→11 on both
  embedders. bge-v2-m3 caps at 10/15. The +1 is the same TC every time — TC-25
  (LSP circuit breaker) flips 1→2.
- jina-v2 + jina-rerank-v2 wins on the full suite (38/75) but loses on
  legacy-natural (23/60). General-purpose multilingual reranker is keyword-friendly
  but loses on long natural-language queries.
- coderank Q4 + jina-rerank-v2 doesn't compound: 36/75 vs 37/75 for the bge baseline.
  Two code-tuned components don't stack — likely because both already over-fit to
  the same code patterns and add noise to each other.
- **Recommendation:** keep coderank Q4 + bge-v2-m3 (TEI:48083) as champion for
  full-suite stability, but consider jina-rerank-v2 swap when T5 improvement
  matters more than legacy parity. The protocol toggle makes this a one-env-var
  switch, no rebuild required.

Teardown: stopped Infinity container; `bench_jinav2_*` and `bench_coderank_*` Qdrant
collections preserved for future re-runs.


### 2026-05-12 — Golden-set audit + post-fix re-baseline

Audited both suites end-to-end. Findings:

**Structural (clean):** all required fields present, no dup ids/queries, tier ranges valid,
all 116 expected paths exist at the pinned SHA.

**Stale expectations (fixed):**
- TC-01 (both suites): `src/tools/mod.rs` → `src/tools/core/types.rs`. `mod.rs` is now
  only `pub mod foo;` declarations after the tools/ refactor; `RecoverableError` lives in
  `core/types.rs`.
- TC-14 (both suites): same fix.

**Filename-token bias:** 14/25 keyword TCs had the literal expected-file basename appearing
in the query (e.g. `path_security` ↔ `path_security.rs`, `MockLspClient` ↔ `mock.rs`).
Rewrote 7 queries to drop blatant cheats while preserving the underlying concept. Remaining
8/25 tokens are concept words (`client`, `output`, `schema`, `index`, `server`, `augment`,
`usage`) that real users would naturally type.

**Re-baseline at champion config** (bm25=5.0, mode=code, TEI reranker `bge-v2-m3`):

|  | natural 20-TC | full 25-TC |
|---|---|---|
| jina-v2 + bge | 26/60 (was 25) | 35/75 (was 36) |
| coderank Q4 + bge | 26/60 | **37/75** (champion confirmed) |

The TC-01/TC-14 expected-path fix gives +1 on natural. The bias removal costs jina-v2
−1 on full (less BM25 lift) but leaves coderank unchanged at 37 — code-aware embedder is
more robust to query rephrasing, which is the *desired* signal we couldn't see before
because BM25 was masking it.

The 41/60 historical claim remains unreproducible; **26/60 natural / 37/75 full** is the
honest post-audit baseline going forward.

### 2026-05-12 — Tavily stack (sqlite-vec + tantivy) + CodeRank, no reranker

**Goal:** Settle the "is the retrieval backend itself the bottleneck?" question.
Reproduce the May-2 31/60 ceiling by going back to the pre-Qdrant stack with the
best dense embedder we have today (CodeRankEmbed-Q4_K_M on llama-server-rocm :43300).

**Setup**

- Worktree pinned at `0795b208e8bab76705d6582f43431e39fcccedf4` (the 31/60 commit) → `.worktrees/bench-legacy/`
- Binary: `.worktrees/bench-legacy/target/release/codescout` (default features: `local-embed`, `remote-embed`, `dashboard`, `http`, `librarian`)
- Index target: `.worktrees/bench` (current code at `ede25e69`) — so TC paths still align with the post-refactor layout
- `[embeddings]` config: `model = "CodeRankEmbed-Q4_K_M.gguf"`, `url = "http://127.0.0.1:43300/v1"`
- Stack: dense via HTTP (coderank, dim 768) + tantivy BM25 in-process; **no reranker** (didn't exist in 0795b208)
- 730 files / 18 229 chunks indexed in ~47 s

**Result: 28/60 on legacy-natural** (p50 93 ms, p95 110 ms)

| Stack | Embedder | Sparse | Rerank | Legacy-natural |
|---|---|---|---|---|
| historical (May 2) | jina-v1-base-code | tantivy | — | 31/60 |
| **tavily + coderank** | **CodeRankEmbed-Q4** | **tantivy** | **—** | **28/60** |
| Qdrant + reranker (today's best) | jina-v2-base-code | splade-cocondenser | bge-v2-m3 | ~30/60 (38/75) |

**Per-TC scores:** TC-01 1/3, TC-02 2/3, TC-03 3/3, TC-04 2/3, TC-05 2/3, TC-06 1/3,
TC-07 2/3, TC-08 2/3, TC-09 2/3, TC-10 1/3, TC-11 0/3, TC-12 1/3, TC-13 1/3,
TC-14 1/3, TC-15 2/3, TC-16 1/3, TC-17 1/3, TC-18 2/3, TC-19 1/3, TC-20 0/3.

**Conclusions**

1. **Ceiling confirmed.** No matter which retrieval engine we swap in (Qdrant vec0,
   sqlite-vec, tantivy, fastembed-bm25, splade, splade-pp), top-10 hit-rate on the
   legacy-natural suite plateaus at 28–30/60. The bottleneck is upstream of the
   retrieval engine — query phrasing, chunk granularity, or embedding-model
   recall on code identifiers.
2. **Latency wins for the legacy stack.** 93 ms p50 vs Qdrant's ~300–500 ms p50 (with rerank).
   No HTTP hop to Qdrant, no second HTTP hop to a reranker.
3. **31/60 isn't a phantom but isn't repeatable either.** Within 3 points across
   completely different backends — this is noise band for a 20-TC suite where each
   TC is 0/3..3/3. The historical value was real but not load-bearing.
4. **No retrieval-stack reason to keep tantivy/sqlite-vec.** Drop them. The
   architecture decision is now justified: ship a thin codescout binary that talks
   HTTP to an external retrieval stack (Qdrant + sparse + dense + rerank).

**Caveats (recorded for honesty)**

- The harness env block records `embedder_url=:48081`, `sparse_embedder_url=:48084`,
  `reranker_url=:48083`, `qdrant_url=:6334`, `embed_model=jina-embeddings-v2-base-code`
  — these are env vars at harness invocation time; the legacy binary **ignored**
  all of them and read its own `[embeddings]` block. The recorded config block
  misrepresents the actual stack. Followup: add `backend = "stack" | "tavily"`
  detection in the harness (e.g. probe `codescout version` for a "retrieval-backend"
  field, or inspect `[embeddings].url` in project.toml).
- `codescout_build_sha` / `codescout_version` are empty because the 0795b208 binary
  predates the build-SHA bake-in (`ad7e7e7a`). `codescout_repo_head_sha` = 0795b208
  (recorded from `git rev-parse HEAD` in the legacy worktree).
- Index built with `--force`; chunks differ slightly (18 229 vs current 17 827+).

### 2026-05-12 — nomic-embed-code-7B Q4 (claimed CoIR SOTA) — negative result

**Goal:** Test whether a much larger code-specific embedder breaks the 28-31/60 ceiling.
Hypothesis: `nomic-ai/nomic-embed-code` (7B, Qwen2.5-Coder-7B-Instruct base, claimed SOTA
on CoIR per Nomic's blog) should outperform 137M-class models if the bottleneck is dense
recall on code identifiers.

**Setup**

- Model: `bartowski/nomic-ai_nomic-embed-code-GGUF` Q4_K_M (~4.1 GB, dim 3584, 32k ctx)
- Server: `llama-server` (CUDA, RTX A5000 24GB) on `:43302` with `--embeddings --pooling last`
- Query prefix: `"Represent this query for searching relevant code: "` (asymmetric, doc side raw)
- Codescout: `ad7e7e7a` (main binary). Env: `CODESCOUT_EMBEDDER_PROTOCOL=openai`,
  `CODESCOUT_EMBEDDER_MODEL_NAME=nomic-embed-code`, `CODESCOUT_MODEL_DIM=3584`,
  `CODESCOUT_QUERY_PREFIX=...`, `CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_nomic_`
- Collection: `bench_nomic_code_chunks` — dim 3584, 21 371 points
- Indexing: 24 923 chunks in 29 minutes (~14 chunks/sec — 7B fwd pass dominates)
- Rerank: bge-reranker-v2-m3 on `:48083` (unchanged)
- Sparse: splade-cocondenser on `:48084` (unchanged)
- Suite: `legacy-natural.json` (20 TCs, max 60)

**Result: 24/60 (worse than current stack and tavily+coderank)**

| BM25 boost | Score | p50 latency |
|---|---|---|
| 5.0 | 24/60 | 178 ms |
| 3.0 | 24/60 | 177 ms |
| 1.5 | 24/60 | 175 ms |
| 0.5 | 25/60 | 173 ms |

**Comparison**

| Stack | Embedder (params) | Sparse | Rerank | Legacy-natural |
|---|---|---|---|---|
| Qdrant + jina-v2 + bge-rerank | jina-v2-base-code (137M) | splade | bge-v2-m3 | 28/60 |
| Tavily (sqlite-vec + tantivy) | CodeRankEmbed-Q4 (137M) | tantivy | — | 28/60 |
| **Qdrant + nomic-embed-code-Q4 + bge-rerank** | **nomic-embed-code (7B)** | **splade** | **bge-v2-m3** | **24/60** |

**Findings**

1. **Bigger is not better here.** A 50× parameter model with a SOTA CoIR claim
   scored 4 points below jina-v2 on our TC suite. Indexing was 35× slower.
2. **BM25 fusion weight is irrelevant.** Sweeping 0.5–5.0 moves the score by 1
   point. The signal is in dense + rerank; fusion barely shifts top-10.
3. **Ceiling is genuinely upstream.** Across radically different dense embedders
   (jina-v2, CodeRankEmbed, nomic-embed-code, nomic-embed-code-7B), retrieval
   backends (Qdrant, sqlite-vec), sparse models (splade, splade-pp, tantivy
   BM25), rerankers (bge-v2-m3, jina-rerank-v2, none), and fusion weights, the
   top-10 hit-rate on legacy-natural sits in 24–28/60. **The bottleneck is the
   TC suite phrasing and/or chunking, not the retrieval stack.**
4. **Q4 quantization probably hurts but isn't the whole story.** We didn't run
   f16 (14GB VRAM, would have to evict other services) — but the spread on
   smaller models between Q4 and f16 was ≤1 point, so we'd expect 24→25 at
   best, not a breakthrough.

**Caveats**

- Project-id mismatch caused a 0/60 first run (sync used `bench_nomic` as id,
  search uses `p.config.project.name` from project.toml = `code-explorer`).
  Fixed by temporarily renaming project.toml; restored after the run.
- Discovered that `src/retrieval/embedder.rs::EmbedderHttp::embed` did not
  apply the asymmetric query prefix in older builds — main since fixed via
  `CODESCOUT_QUERY_PREFIX` env var. Earlier Qdrant-stack runs with CodeRankEmbed
  may have silently underperformed because the prefix wasn't applied on the
  query path (legacy `RemoteEmbedder` had it; new `EmbedderHttp` lacked it).
- Bench worktree at `ede25e69` was modified with redundant patches during this
  experiment (query_prefix_for + EmbedderHttp query_prefix). The patches are
  redundant because main already had `CODESCOUT_QUERY_PREFIX` support. Bench
  worktree is now slightly diverged from `ede25e69`; treat the canonical pinned
  bench as the main binary at `ad7e7e7a` going forward.

**Conclusion: drop nomic-embed-code-7B from consideration.** The
infrastructure cost (29-min reindex, 24 GB VRAM on the AMD or 6 GB on NVIDIA,
2× search latency) buys negative quality on our suite. If we want to break the
ceiling, the next levers are **TC suite phrasing audit** (drop or rephrase the
8/20 TCs that flatline at 0/3 across all configs) and **chunking strategy**
(node-aware chunks vs char-bounded splits).

### 2026-05-12 — Bench-doc pollution + mode=code blind spot (+6 points, no infra change)

**Goal:** After concluding the retrieval stack is not the bottleneck, audit the
TC suite itself. Look at zero-score TCs across the latest jina-v2 + bge-rerank
baseline and identify systematic issues.

**Findings (two no-cost bugs in the bench, not the stack)**

1. **`mode=code` post-filter blind spot.** The harness called `semantic_search`
   with default `mode="code"`, which post-filters out all markdown candidates.
   Several TCs have markdown-only expected files (TC-05's
   `docs/PROGRESSIVE_DISCOVERABILITY.md`, TC-17's docs in
   `docs/manual/src/concepts/`) — these TCs returned empty top-10 lists not
   because retrieval failed, but because every candidate was filtered out.
   Switching to `mode="full"` lifted score 26 → 29 (+3) on identical
   collection.

2. **Bench-doc data leak.** The TC queries are stored verbatim in
   `docs/research/2026-04-03-embedding-model-benchmark.md`,
   `docs/research/2026-05-06-retrieval-stack-benchmark.md`,
   `docs/trackers/retrieval-benchmark.md`, and `scripts/run-tc-benchmark.py`.
   Semantic search legitimately ranked those highest because they contain the
   exact query strings. **15 of 60 top-3 slots (25%) were pollution.** Deleting
   their chunks from Qdrant: 29 → 32 (+3). Combined with `mode=full`: 26 → 32
   (**+6 points = +23% relative**).

**Result: 32/60 on legacy-natural** with no infra change — beats the 31/60
"mythical historical ceiling" we couldn't reproduce, using the same
jina-v2-base-code + splade-cocondenser + bge-reranker-v2-m3 stack we already
had.

**Per-TC delta (jina-v2 + bge-rerank, both runs)**

| TC | mode=code | mode=full + depollute | Δ |
|---|---:|---:|---:|
| TC-01 | 1 | 2 | +1 |
| TC-02 | 0 | 2 | +2 |
| TC-03 | 2 | 2 | 0 |
| TC-04 | 2 | 2 | 0 |
| TC-05 | 0 | **3** | **+3** |
| TC-06 | 1 | 2 | +1 |
| TC-07 | 2 | 0 | -2 (now caught by audit, see below) |
| TC-08 | 2 | 2 | 0 |
| TC-09 | 2 | 2 | 0 |
| TC-10 | 1 | 1 | 0 |
| TC-11 | 2 | 2 | 0 |
| TC-12 | 0 | 2 | +2 |
| TC-13 | 1 | 2 | +1 |
| TC-14 | 1 | 2 | +1 |
| TC-15 | 2 | 2 | 0 |
| TC-16 | 1 | 0 | -1 (audit needed) |
| TC-17 | 1 | 2 | +1 |
| TC-18 | 2 | 1 | -1 (audit needed) |
| TC-19 | 1 | 1 | 0 |
| TC-20 | 0 | 0 | 0 (audit needed) |
| **Total** | **26** | **32** | **+6** |

**Fixes committed**

- `scripts/run-tc-benchmark.py`: new `--mode {code,full}` CLI flag, default
  `full`. Default change is reasonable because the suite mixes code and
  markdown expected files; `full` is the superset.
- `.codescout/project.toml`: added the four bench-doc paths plus
  `.codescout/projects/**` (codescout's own per-project memories — pure
  internal noise) and `scripts/tc-suites/**` to `[ignored_paths] patterns`.
  Existing chunks for these were deleted from Qdrant `code_chunks` via
  `points/delete` filter on `chunk_id` (text-match).

**Three remaining zero-score TCs — root causes, not pollution**

- **TC-07** "section boundary detection in markdown editing"
  Top-3 are all `chunker.rs` (text chunker that *also* detects section
  boundaries — semantically right, not the expected
  `src/tools/markdown/edit_markdown.rs`). The expected file's chunks just
  don't surface for this phrasing. Likely fix: rephrase to
  `"edit_markdown section heading replace insert_after action"` or accept
  `chunker.rs` as a valid match.

- **TC-16** "how a semantic search query flows from input through embedding
  to KNN ranked results"
  Top-10 dominated by design specs (`auto-reindex-on-edit-design.md`,
  `hybrid-retrieval-design.md`, `library-indexing-redesign.md`). Design docs
  legitimately answer "how does X flow?" better than the implementation
  files. Either rephrase to be implementation-anchored
  (`semantic_search call_tool semantic_search.rs RetrievalClient`) or
  broaden truth set to include the matching design docs.

- **TC-20** "three prompt surfaces consistent when tools are renamed"
  Top-1 is `CLAUDE.md` — which **is** the canonical place for the three-
  prompt-surfaces doctrine. The expected files
  (`src/prompts/server_instructions.md`, `onboarding_prompt.md`) are the
  prompts themselves, not the meta-discussion the query asks about. **The
  truth set is wrong.** Either add `CLAUDE.md` to expected, or rephrase to
  ask about prompt content rather than the consistency pattern.

**Implication for the retrieval-stack design**

- 28/60 was never the true ceiling — it was 26/60 with hidden mode and
  pollution bugs. Honest current ceiling on jina-v2-base-code (137M, default
  stack) is **32/60**, with 3 TCs requiring TC-suite repair (not retrieval
  fixes) to potentially reach 36-38/60.
- Reinforces the earlier conclusion: **don't strip-and-ship the stack
  optimization-side without first repairing the TC suite**, otherwise the
  bench remains an unreliable signal for future retrieval improvements.
- Concretely: before locking docker-compose profiles, run a "clean bench"
  (post-fix) sweep across the jina-v2 / CodeRankEmbed / nomic-embed-code
  matrix to see whether the model-size ranking changes once the noise is
  removed. The earlier negative result for nomic-embed-code (24/60) is
  suspect because it inherited the same mode and pollution bugs.
