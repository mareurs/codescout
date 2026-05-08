---
id: '8e09ca67f463027e'
kind: tracker
status: draft
title: Retrieval Stack — Benchmark Results
owners: []
tags:
- retrieval
- benchmark
- embedding
topic: null
time_scope: null
---

# Retrieval Stack — Benchmark Results

Tracks 60-point TC suite results across embedding model runs.
Each TC scores 0–3 points (one per expected file found in top-10 results).
20 TCs × 3 pts each = 60 max.

## TC Suite

- **Tier 1** (5 TCs) — exact symbol/config lookups
- **Tier 2** (7 TCs) — implementation pattern queries
- **Tier 3** (5 TCs) — architectural / cross-cutting queries
- **Tier 4** (3 TCs) — documentation / prompt surface queries

## Notes

- Legacy (tantivy) indexes ALL file types; stack initially indexed code only
- Expanding to `.md`/`.sh` scope expected to recover Tier 4 gap
- `p50`/`p95` latency measured per-query including reranker round-trip

## History

### 2026-05-07 — initial runs

Three model runs on `retrieval-stack` branch against code-explorer project.
BGE-M3 and Jina both landed at 18/60 with code-only indexing.
Root cause of gap vs legacy: `.md`/`.sh` files excluded from Qdrant sync — Tier 4 queries target prompt surface docs.
Next: re-run with expanded index scope.

