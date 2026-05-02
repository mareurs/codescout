# Tracker: LanceDB Upgrade Path

**Status:** Watching — not ready  
**Created:** 2026-05-02  
**Decision deferred until:** LanceDB Rust SDK stabilizes (target: v1.0 or FTS bugs close)

## Context

Evaluated replacing `sqlite-vec` + `tantivy` with LanceDB for hybrid BM25+vector search
over code symbols and markdown documents. LanceDB Rust SDK is the right long-term direction
but has blocking issues as of research date.

## Current stack

| Component | Role | Status |
|-----------|------|--------|
| `sqlite-vec` (vec0 virtual tables) | Dense vector KNN search | Active |
| `tantivy` + `BM25Index` (`src/embed/bm25.rs`) | BM25 full-text search with `CodeTokenizer` (camelCase/snake_case splitting) | Active |
| RRF fusion (`src/embed/fusion.rs::rrf_fuse`) | Hybrid ranking, k=60 | **Already implemented** |

RRF is fully wired in `src/tools/semantic.rs`: both legs run in parallel, fused via RRF, BM25-only hits fetched from SQLite and merged. The `.codescout/tantivy/` directory is the live BM25 index built at the end of `build_index`.

**No immediate work needed on search quality** — the hybrid stack is complete.
## Why LanceDB

- Rust core (`lancedb` crate) — embedded, no separate process, ~4 MB idle RAM
- Native hybrid search API (dense + BM25) in a single query surface
- Eliminates dual-index bookkeeping
- Active development, frequent releases

## Blocking issues (as of 2026-05-02)

| Issue | Severity | Details |
|-------|----------|---------|
| FTS engine replaced | Medium | v0.21.0 switched from Tantivy to native Lance FTS; Tantivy fully removed in v0.28.x beta. BM25 ranking quality vs Tantivy unconfirmed. |
| `create_fts_index` + `.offset()` bug | High | Returns empty results (GitHub #2459, open) |
| Hybrid search regression v0.21.1→v0.23.0 | High | Returns no results; `nprobes not set` warning. Confirmed by multiple users. |
| FTS fails on S3 | Low | `Path.exists()` check bug — irrelevant for local use |
| Exact-match ranking weakness | Medium | Exact matches may not appear in top-k without inflating limit |
| Rust SDK docs thin | Low | docs.rs sparse vs Python ReadTheDocs; few embedded Rust examples in the wild |

## Signals to watch

- [ ] GitHub issue #2459 closed (FTS + offset fix)
- [ ] Hybrid search stable across 2+ consecutive minor versions
- [ ] Native Lance FTS ranking quality benchmarked vs Tantivy on code corpora
- [ ] `lancedb` crate reaches v1.0 or explicit "stable" API declaration
- [ ] Community reports of pure Rust embedded usage (not via Python bindings)

## Migration rationale\n\n1. **Done — hybrid BM25+vector with RRF already ships.** No search quality work needed now.\n2. **LanceDB migration** unifies sqlite-vec + tantivy + manual RRF into a single embedded store with a native hybrid query API. Worth it once the Rust SDK stabilizes.\n\n
## Migration scope (when ready)

- Remove `sqlite-vec` dep + vec0 index code
- Remove `tantivy` dep (or keep if Lance FTS quality falls short)
- Replace `embed/index.rs` producer pipeline with LanceDB insert API
- Replace `tools/semantic.rs` KNN query with LanceDB dense search
- Rewrite hybrid fusion to use `HybridQuery` API instead of manual RRF
- Estimated: ~300 LOC changed, index format migration required for existing users

## References

- Research session: 2026-05-02 (this conversation)
- LanceDB crate: https://crates.io/crates/lancedb (v0.27.2 stable, v0.28.0-beta.11)
- GitHub: lancedb/lancedb — filter `label:rust` for SDK-specific issues
