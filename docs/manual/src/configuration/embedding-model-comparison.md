# Embedding Model Comparison

Which embedding model should you use with codescout? This page summarizes benchmark
results and real-world usage data to help you choose.

> This comparison is based on the codescout codebase (417 files, ~32K chunks) as of
> 2026-04-03. Results may vary with different codebases. We will update this page as
> we collect more real-world data.

## Models Tested

| Model | Dims | Context | Size | Backend | Setup |
|-------|------|---------|------|---------|-------|
| `local:AllMiniLML6V2Q` | 384 | 256 tok | 22 MB | Bundled ONNX (CPU) | None — works out of the box |
| `nomic-embed-text` | 768 | 8,192 tok | 274 MB | Ollama | `ollama pull nomic-embed-text` |
| `nomic-embed-code` (Q4_K_M) | 3584 | 32,768 tok | 4.1 GB | llama.cpp (GPU) | Download GGUF + start server |

## Benchmark Results

We tested 20 queries across 4 complexity tiers, scoring each 0-3 based on whether
the expected source files appeared in the top 10 results.

### Overall Scores (max 60)

```
nomic-embed-code ████████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░  36/60
AllMiniLML6V2Q   ██████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  34/60
nomic-embed-text ████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  32/60
```

### By Complexity Tier

| Tier | What it tests | Best model | Score |
|------|--------------|------------|-------|
| **1. Direct Concept** (5 queries) | Single named type, module, or feature | nomic-embed-text | 12/15 |
| **2. Two-Concept** (7 queries) | Relationship between two concepts | nomic-embed-code | 17/21 |
| **3. Cross-Cutting** (5 queries) | Three+ concepts, architectural flows | nomic-embed-code | 7/15 |
| **4. Architectural** (3 queries) | Design invariants, consistency patterns | AllMiniLML6V2Q | 5/9 |

No single model dominates all tiers.

### Practical Metrics

| Metric | AllMiniLML6V2Q | nomic-embed-text | nomic-embed-code |
|--------|---------------|-----------------|------------------|
| Index time (417 files) | **70 seconds** | 60 seconds | 25 minutes |
| DB size | 71 MB | **55 MB** | 372 MB |
| Chunk count | 32,098 | 11,887 | 11,868 |
| Requires | Nothing | Ollama running | GPU + llama.cpp server |

## How Agents Actually Use Semantic Search

Analysis of 31,674 tool calls across 70+ real projects:

- `symbols` — **17.8%** of all calls (the workhorse)
- `grep` — **2.3%**
- `semantic_search` — **1.1%** (349 calls total)

Agents use semantic search as a **last resort** — when they don't know the exact name
of what they're looking for. The typical query is a short 3-6 word concept phrase:

```
"error handling and recovery from tool failures"
"embedding index build and incremental update"
"security path validation and write access control"
"ollama embedding configuration"
"intent classifier ONNX model prediction"
```

These are mostly Tier 1-2 queries (direct concept or two-concept composition). Tier 3-4
queries (complex architectural questions) are rare in organic usage.

## Recommendation

**Use the default: `local:AllMiniLML6V2Q`.**

| Factor | Why the default wins |
|--------|---------------------|
| **Score** | 34/60 — within 2 points of the best model (36/60) |
| **Speed** | 70 seconds vs 25 minutes — 21x faster indexing |
| **Setup** | Zero. No Ollama, no GPU, no server to manage |
| **Storage** | 71 MB — reasonable for any machine |
| **Precision** | Best at Tier 4 (finding specific functions and patterns) — matches how agents actually query |

The 7B code-specialized model's 2-point advantage doesn't justify 21x slower indexing,
5x more storage, and a GPU requirement. For the 1.1% of calls that reach semantic search,
the bundled model is good enough.

### When to consider alternatives

- **nomic-embed-text via Ollama** — if Ollama is already running for other tasks, add
  `url = "http://localhost:11434/v1"` and `model = "nomic-embed-text"` for slightly better
  Tier 1 results at the same speed. Smallest storage footprint (55 MB).

- **nomic-embed-code via llama.cpp** — if you have a GPU and primarily use semantic search
  for concept-level exploration (architecture questions, onboarding to a new codebase).
  Best at Tier 2-3 queries.

## Methodology

Full benchmark details, per-query scores, and test case definitions are in
[`docs/research/2026-04-03-embedding-model-benchmark.md`](https://github.com/mareurs/codescout/blob/experiments/docs/research/2026-04-03-embedding-model-benchmark.md).

The benchmark will be updated as we collect more real-world query data via the `--debug`
flag's usage traceability feature (see [Debug Mode](../concepts/diagnostic-logging.md)).
