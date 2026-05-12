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
