# Heuristic Code Analysis During Indexing

**Status:** deferred — brainstormed 2026-04-19, not yet scoped
**Owner:** TBD
**Related:** `docs/trackers/embedding-chunk-size-2026-04.md`

## Motivation

AST walk already happens during indexing (for chunking). Marginal cost of emitting code-health metrics in the same pass is near-zero. This fills a discovery gap that clippy/ruff/eslint don't: they find *rule violations*; this ranks *where to look first*.

codescout's sweet spot is concept-level questions — "where's the messy code", "what are the largest functions", "what's dead" — answered via ranked tool outputs that fit progressive discovery.

## Design Principles

- **No LLM.** Pure heuristics, tree-sitter + LSP only.
- **No linting.** Don't duplicate clippy/ruff/eslint. Rank, don't judge.
- **Incremental.** Computed in the existing AST pass, invalidated by file hash.
- **Progressive disclosure.** Compact summary → ranked list → full body on demand.
- **Language-aware thresholds.** Rust tolerates longer functions than Python.

## Metrics (in order of implementation cost)

### Cheap (tree-sitter only, single pass)

| Metric | Node kind | Actionable as |
|---|---|---|
| LOC per function | span line count | `large_methods` hotspot |
| Cyclomatic (approx) | count `if`/`match`/`for`/`while`/`&&`/`\|\|` | `complex_methods` |
| Max nesting depth | block depth walk | `deeply_nested` |
| Parameter count | signature parameter list | `too_many_params` |
| File LOC | line count | `god_files` |
| TODO/FIXME/HACK/XXX | text scan | `debt_inventory` |
| Comment density | comment LOC / total LOC | outlier detection |
| Boolean parameters | param type = bool | smell signal |

### Medium (requires AST cross-referencing within file)

| Metric | Approach |
|---|---|
| Symbol fan-out (intra-file) | count identifier references in function body |
| Dead private symbols (intra-file) | no references in same module |
| Duplicate token n-grams | rolling hash on token stream, threshold N |

### Expensive (requires LSP; defer to "deep scan" pass)

| Metric | LSP call |
|---|---|
| Zero-reference public symbols | `find_references` — dead code |
| Inter-file fan-out | `find_references` — god objects |
| Unused imports | LSP diagnostics scrape |

## Storage

New table in project sqlite DB:

```sql
CREATE TABLE symbol_metrics (
    file TEXT NOT NULL,
    symbol TEXT NOT NULL,
    kind TEXT NOT NULL,
    loc INTEGER,
    complexity INTEGER,
    nesting INTEGER,
    params INTEGER,
    fan_out INTEGER,
    file_hash TEXT NOT NULL,
    PRIMARY KEY (file, symbol)
);
CREATE INDEX idx_metrics_kind ON symbol_metrics(kind, complexity DESC, loc DESC);
```

Invalidation: keyed on `file_hash`, same lifecycle as chunks. File unchanged → skip re-compute.

## Surfacing

### New tool: `code_hotspots`

```
code_hotspots(
    kind: "large_methods" | "complex_methods" | "deep_nesting" | "too_many_params"
        | "god_files" | "dead_code" | "todos" | "duplicates",
    limit: int = 20,
    detail_level: "compact" | "full" = "compact",
    path_glob: str? = None,  // filter to subdirectory
) -> [{symbol, file:line, metric, severity}]
```

Default output: JSON list of top-N by metric, compact.
`detail_level="full"` includes symbol body snippet.

### `project_status` augmentation

Add `health` section:
```json
{
  "health": {
    "large_methods": 12,
    "complex_methods": 3,
    "todos": 47,
    "god_files": 1
  }
}
```

One-liner signal for the LLM to decide whether to drill in.

## Implementation Phases

1. **Phase 1 — cheap metrics + storage.** LOC, complexity, nesting, params, file LOC, TODOs. `code_hotspots` tool. `project_status.health`. ~1 week effort.
2. **Phase 2 — language-aware thresholds.** Per-language severity calibration. User-configurable via `project.toml`.
3. **Phase 3 — medium metrics.** Duplicate n-grams, intra-file dead code.
4. **Phase 4 — LSP-backed deep scan.** Inter-file dead code, true fan-out. Run on-demand, not every index.
5. **Phase 5 (speculative) — hotspot-boosted semantic search.** Queries like "messy error handling" get a ranking boost for high-complexity matches. Requires A/B testing.

## Open Questions

- Severity calibration: percentile-based (top 5% = severe) vs absolute thresholds (LOC > 100 = severe)? Percentile is self-adjusting but noisier for small codebases.
- Should `code_hotspots` compose with `semantic_search`? e.g. "semantic_search(q, filter_hotspot='complex_methods')" — useful but adds surface area.
- Thresholds per language in memory or code? Memory lets users tune; code is cleaner. Probably hybrid: defaults in code, overrides in `project.toml`.
- Caveman vs clippy overlap: for Rust, clippy already flags some of this (`too_many_arguments`, `cognitive_complexity`). Do we piggyback on clippy output or reimplement? Reimplementing keeps language symmetry.

## Anti-Goals

- Not a linter.
- Not a code-review bot (no "you should refactor this").
- No LLM calls in the analysis path.
- No per-language rule DSL — hardcoded heuristics are fine.

## References

- Brainstorm context: see session transcript 2026-04-19.
- Progressive disclosure canon: `docs/PROGRESSIVE_DISCOVERABILITY.md`.
- Existing AST chunker: `src/embed/ast_chunker.rs`.
