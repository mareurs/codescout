# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [`artifact(action="move")`](./artifact-move.md) — atomic file rename + catalog `rel_path` update in one step; replaces `git mv` + manual reindex.
- [`doc://librarian-guide` MCP resource](./librarian-guide-resource.md) — dense librarian reference (filter syntax, tracker workflow, common mistakes) available via `resources/read doc://librarian-guide`.
- [Librarian tools collapse (16 → 5)](./librarian-tools-collapse.md) — 16 individual librarian tools collapsed into 5 action-dispatched tools: `artifact`, `artifact_event`, `artifact_augment`, `artifact_refresh`, `librarian`.
- [Auto-Reindex on Edit](./auto-reindex-on-edit.md)
- [Librarian (embedded in codescout)](./librarian-embedded.md) — workspace doc/spec/plan index served as part of codescout when built with the `librarian` cargo feature; **disabled by default** — opt in via `LIBRARIAN_ENABLED=1` env or `[librarian] enabled = true` in `.codescout/project.toml`.
- [workspace_state_at](./workspace-state-at.md) — Time-travel snapshot: all artifacts in scope at a given commit/timestamp, with `freshness_at_as_of` vs `freshness_now` diff.
- [Heartbeat memory fields](./heartbeat-memory-fields.md) — debug-mode heartbeat now logs `vm_size_kb` / `vm_rss_kb` / `vm_data_kb` / `vm_peak_kb` from `/proc/self/status`; gives per-instance memory time-series for OOM forensics.
- [call_graph](./call-graph.md) — transitive call graph for any symbol; supports `callers`, `callees`, and `both` directions with LSP + tree-sitter fallback, sqlite edge caching, and per-file cache invalidation.
- [Augmentation: render_template + params_schema](./augmentation-render-template.md) — MiniJinja template projecting params into `librarian_context` output; JSON Schema validation on every params write.
- [tracker_design](./tracker-design.md) — pre-creation teaching tool returning 6 archetypes + 7-step design guide + existing-tracker landscape; call before `tracker_create`.
- [artifact_refresh_stale](./artifact-refresh-stale.md) — discovery tool listing augmented artifacts due for refresh, oldest-first (never-refreshed first).
- [Librarian tool consolidation](./librarian-tool-consolidation.md) — 22 → 16 tools: six single-purpose tools absorbed into parent tools (`artifact_find`, `artifact_create`, `artifact_augment`, `artifact_get`, `artifact_update`).
- [Hybrid BM25 + Vector Retrieval](./hybrid-bm25-vector.md) — `semantic_search` now fuses dense vector search with sparse BM25 keyword search via Reciprocal Rank Fusion (RRF), using a code-aware tokenizer that handles camelCase and snake_case.
