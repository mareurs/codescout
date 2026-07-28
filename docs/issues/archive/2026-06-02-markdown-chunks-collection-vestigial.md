---
status: fixed
opened: 2026-06-02
closed: 2026-06-04
severity: low
owner: marius
related:
  - docs/issues/2026-06-02-preflight-sync-walker-divergence.md
tags:
  - embeddings
  - retrieval
  - dead-code
  - markdown
kind: bug
---

# BUG: `markdown_chunks` collection + `search_markdown()` are vestigial (never written, never called)

## Summary
The retrieval layer defines a dedicated `markdown_chunks` Qdrant collection and a
`RetrievalClient::search_markdown()` method, but **nothing ever writes to that
collection and nothing ever calls that method**. Markdown is actually embedded
into the shared `code_chunks` collection (tagged `language="markdown"`) by
`sync_project`, and surfaced only via `semantic_search(mode="full")` (the default
`mode="code"` adds a `must_not language=markdown` filter). The separate
collection/method is dead code that reads as live — a trap for the next reader.

## Symptom (Effect)
- `src/retrieval/search.rs:120-129` — `pub async fn search_markdown(...)` queries
  `self.config.collection("markdown_chunks")`.
- No writer targets `markdown_chunks`. Every `ensure_collection` / `upsert_points`
  / `scroll_chunk_refs` / `delete_points` in the codebase targets `code_chunks`
  (or `memories`, for the separate memory channel).
- `search_markdown` has no callers.

So a future caller wiring up `search_markdown()` would get an **empty result set
with no error** — the collection exists in code but is never populated.

## Reproduction
Code inspection at `git rev-parse HEAD`:

```
grep -n "markdown_chunks|search_markdown|\.collection\(" src   # via codescout grep
```

Result: `markdown_chunks` appears only at `src/retrieval/search.rs:127` (the read
side). All write sites use `code_chunks`.

## Environment
codescout v0.14.0, Rust, MCP stdio. Project: code-explorer.

## Root cause
Two coexisting designs were never reconciled:

- **Design A (live):** one `code_chunks` collection holds code + markdown + toml;
  markdown is distinguished by the payload `language` field and filtered at query
  time (`SearchOpts.exclude_languages`, `src/retrieval/search.rs:14`).
- **Design B (vestigial):** per-content-type collections (`code_chunks`,
  `markdown_chunks`, `memories`, `library_chunks`) with matching `search_*`
  methods. Only `code_chunks` and `memories` are wired to writers; `markdown_chunks`
  and `library_chunks`'s `search_markdown`/`search_libraries` lack a populating path
  (`search_libraries` also returns the documented-unsupported lib scope — see L-12
  in `docs/trackers/2026-05-07-legacy-retrieval-removal.md`).

## Evidence
`grep` for `markdown_chunks|search_markdown|.collection(` across `src/`:
- `src/retrieval/search.rs` — `search_markdown` + `collection("markdown_chunks")` (read only).
- `src/retrieval/sync.rs`, `src/tools/semantic/index.rs`, `src/tools/config/mod.rs`,
  `src/tools/onboarding.rs`, `src/dashboard/api/index.rs` — all `code_chunks`.
- `src/agent/mod.rs:1574` — `memories` (the live memory channel).

## Hypotheses tried
N/A — straightforward dead-code/inconsistency finding, not an intermittent bug.

## Fix

**Implemented 2026-06-04 on `experiments`** — option 1 (delete). Removed `RetrievalClient::search_markdown` from `src/retrieval/search.rs` (zero callers, confirmed via `references`). Markdown stays intentionally co-mingled in `code_chunks` (tagged `language="markdown"`) and is reachable via `semantic_search(mode="full")`. Also corrected a `src/retrieval/config.rs` doc comment that listed `markdown_chunks` as a "live collection" example (→ `memories`).

`library_chunks`/`search_libraries` (the sibling vestigial pair, L-12) left untouched — separate scope. clippy clean; no test churn (deletion of uncalled code).
## Tests added

None — dead-code deletion. `references(search_markdown)` returned only the (now-removed) definition, and `cargo clippy --all-targets -- -D warnings` confirms nothing references it.
## Workarounds
To search documentation/markdown today: `semantic_search(mode="full")` (queries
`code_chunks` without the `must_not language=markdown` filter). `search_markdown()`
must not be relied on.

## Resume

**Fixed 2026-06-04 on `experiments`** (deleted `search_markdown`, corrected the config doc comment). Not yet on master — ship via Standard Ship Sequence, then `git mv` to `docs/issues/archive/` citing the **master-side** SHA. The companion preflight question (whether the indexer should honour `ignored_paths` for `.codescout/`/`.claude/` markdown in `code_chunks`) remains separate — see `docs/issues/2026-06-02-preflight-sync-walker-divergence.md`.
## References
- `src/retrieval/search.rs:120-129` (`search_markdown`, `markdown_chunks`)
- `src/retrieval/search.rs:14` (`SearchOpts.exclude_languages`)
- `src/tools/semantic/semantic_search.rs:228` (`mode` → `exclude_languages`)
- `src/retrieval/sync.rs` (`sync_project`, writes `code_chunks`)
- docs/issues/2026-06-02-preflight-sync-walker-divergence.md
