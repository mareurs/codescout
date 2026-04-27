---
id: null
kind: tracker
status: archived
title: experiments → master promotion tracker
owners: []
tags:
- release
- promotion
topic: null
time_scope: null
---

# experiments → master promotion tracker

All 265 commits promoted to master on 2026-04-21 (`aa6bff1`). 1986 tests passing, clippy clean.

Individual cherry-pick strategy proved unworkable due to deep cross-file dependencies; final approach took end state of `src/`, `tests/`, `crates/`, `Cargo.toml`, `docs/` from experiments directly into a single verified commit.

See promotion plan: `docs/superpowers/plans/2026-04-21-experiments-to-master.md`

**Columns:** ✅ Promoted | ⚠ Experimental on master (docs carry `⚠ Experimental` callout)

---

## Foundational / Infrastructure

### Cargo workspace conversion
**Status:** ✅ Ready  
**Key commit:** `e33532e` chore: convert to cargo workspace  
**Note:** Everything downstream depends on this. Must be first in any promotion sequence.

### jemalloc global allocator
**Status:** ✅ Ready  
**Key commit:** `88efd2c` perf: switch global allocator to jemalloc  
**Note:** Single commit, no API surface change. Reduces memory fragmentation on long-running server.

### `codescout-embed` crate extraction
**Status:** ✅ Ready  
**Key commits:** `62fe478`→`ed42e7a` (refactor(embed): extract + migrate callsites)  
**Note:** Pure refactor — moves `Embedder` trait, chunker, local/remote embedders to new `codescout-embed` crate. All callsites updated. Required for metadata-enriched chunks.

---

## Core Tool Improvements

### Cancel handling
**Status:** ✅ Ready  
**Key commits:** `04fce16` cancel-aware dispatch + child reaping, `b5121f2` suppress cancel response  
**Note:** Fixes MCP disconnect on Escape. Low risk, high value.

### `list_dir`: disable gitignore filtering
**Status:** 🔧 Needs review  
**Key commit:** `cced68e`  
**Note:** Disables `.gitignore`, `.git/info/exclude`, global gitignore, and `.ignore` filtering so `list_dir` returns all files. Intentional? Verify against expected behaviour before promoting — may surface unwanted files in large repos.

### BUG-035/037/039 — path disambiguation, ANSI stripping, attr walk-back guard
**Status:** ✅ Ready  
**Key commit:** `102b4cf` fix(output,symbol)  
**Note:** Fixes path disambiguation note spam, ANSI stripping in symbol bodies, attribute walk-back on `replace_symbol`.

### BUG-040 — atomic_write preserves Unix exec bit
**Status:** ✅ Ready  
**Key commit:** `98faa30` fix(fs)  
**Note:** Prevents `chmod +x` being lost after any write-tool call on shell scripts.

### BUG-041 — retry on stale LSP positions
**Status:** ✅ Ready  
**Key commit:** `bfeeed1` fix(symbol)  
**Note:** `replace_symbol`/`insert_code` retry when LSP returns stale start_line. Fixes flaky edits after large rewrites.

### BUG-042/043 guards — body-only `new_body` + section EOF wipe
**Status:** ✅ Ready  
**Key commit:** `fef3aa8` fix(edit)  
**Note:** Detects and rejects body-only `new_body` in `replace_symbol`; guards `edit_markdown replace` from wiping file tail when section runs to EOF.

### BUG-044 — sibling-preservation on nested symbol edits
**Status:** ✅ Ready  
**Key commits:** `be345cd` fix(tools), `e391913` test(tools)  
**Note:** Symmetric parent clamp + sibling-drop rollback. Regression tests added.

### `replace_symbol` BUG-036 — stale start_line validation
**Status:** ✅ Ready  
**Key commit:** `6de19d9` fix(symbol)  
**Note:** Tightens `validate_symbol_position` to catch stale positions early.

---

## Three-level Guidance Taxonomy

### Hint / Warning / MustFollow levels
**Status:** ✅ Ready  
**Key commits:** `1ca5566` feat(errors), `8d5acb1` feat(read_markdown), `072b61b` docs  
**Note:** Replaces flat "hint" with structured severity. All tools updated. Documented in `PROGRESSIVE_DISCOVERABILITY.md`.

---

## Prompt / Onboarding

### Prompt efficiency overhaul (D1–D5)
**Status:** ✅ Ready  
**Key commits:** `394f47c`→`17a0bea` (5 refine(prompts) commits) + `bf80963` ONBOARDING_VERSION=6  
**Note:** Strips Tool Reference to routing+gotchas, deduplicates Anti-Patterns, moves workflows to tool-guide resource, dynamic Kotlin filtering. Reduces session prompt token cost significantly.

### ONBOARDING_VERSION bumps (4, 6, 7)
**Status:** ✅ Ready (bundled with above)  
**Note:** Each bump triggers system-prompt refresh for all projects. Versions 4, 6, 7 each tied to specific surface changes above.

---

## MCP Resources

### Resource registry + doc:// / memory:// / project://summary
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/mcp-resources.md`  
**Key commits:** `164854d`→`ba8ac5a` (resource registry scaffold → tool-guide resource)  
**Note:** Adds `resources/list` + `resources/read` MCP handlers. Three provider types: `doc://`, `memory://`, `project://summary`. Tool descriptions capped; full guide moved to `doc://codescout-tool-guide`.

### doctor://tool-usage resource + project_hints
**Status:** ⚠ Experimental  
**Experimental doc:** `docs/manual/src/experimental/tool-usage-doctor.md`  
**Key commit:** `77c6029`  
**Note:** Surfaces per-tool call counts for usage analysis. Useful but niche — keep experimental until value proven in practice.

### Progress notifications (index_project, semantic_search, run_command)
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/mcp-resources.md` (consolidated page)  
**Key commits:** `f050c82`, `b29ee82`  
**Note:** 2 Hz throttled `$/progress` notifications. ProgressSink trait makes it testable.

---

## read_markdown Improvements

### Adaptive three-tier output + @file_* buffer refs
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/read-markdown-improvements.md`  
**Key commits:** `5be8e50` three tiers, `a73b6e7` @file_* line-range, `8d5acb1` heading nav on @file_*, `8820f3a` ONBOARDING_VERSION=7  
**Note:** Small files → full content; medium → content+hint; large → heading map + recipe. `@file_*` refs work for line-range reads. MustFollow overflow hint primes buffer reuse.

---

## list_symbols Progressive Directory

### Three-mode progressive dispatch
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/list-symbols-progressive-dir.md`  
**Key commits:** `bce2042`→`b1d220a` (threshold constants → format_compact rendering)  
**Note:** Auto-selects flat/class_overview/directory_map based on file count thresholds. `force_mode` param for explicit override. Avoids token overload on large directories.

---

## Bash / Shell Support

### tree-sitter-bash + AST chunker + LSP config
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/bash-language-support.md`  
**Key commits:** `fb439ad` grammar, `8281775` use import, `f0e1f4d` LSP config, `2739921` AST symbols, `f5b796b` embed chunker  
**Note:** Bash promoted to full support alongside Rust/Python/TS/Java. `bash-language-server` must be installed separately.

---

## Write Serialization

### Cross-process write lock
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/cross-process-write-serialization.md`  
**Key commits:** `6960923`→`c924bf8` (fd-lock dep → gate write-tool dispatch)  
**Note:** `fd-lock` RAII guard on `.codescout/write.lock` serialises all write-tool calls across concurrent MCP server instances. Smoke test + cross-process contention test included.

---

## Global Config

### Two-layer global + project config merge
**Status:** ⚠ Experimental  
**Key commits:** `dc17100` GlobalConfig + XDG path, `eac732a` two-layer merge, `bbc7736` malformed TOML error  
**Note:** `~/.config/codescout/config.toml` (or `$XDG_CONFIG_HOME`) + per-project `.codescout/config.toml` merged at load time. File-size guard, HOME fallback. Marked experimental until user-facing config docs written.

---

## Index Scope Guard / Preflight

### check_index_scope + elicitation confirmation
**Status:** ⚠ Experimental  
**Experimental doc:** `docs/manual/src/experimental/index-scope-guard.md`  
**Key commits:** `2a2a1f8`→`eff725b` (scaffold → preflight + elicit)  
**Note:** Stat-walks root before `index_project`; triggers confirmation elicitation when root looks too broad (home dir, system path, or over `security.max_index_bytes`). Experimental until elicitation UX confirmed stable across clients.

---

## LSP Multiplexer — Rust Rollout

### Per-language mux + rust enabled
**Status:** ⚠ Experimental  
**Experimental doc:** `docs/manual/src/experimental/mux-rust.md`  
**Key commits:** `d8c5d80` per-language config, `5efcc54` rust enabled (idle 180s), `699629e` coherence test  
**Note:** Shares one rust-analyzer instance across concurrent agents. `idle_timeout_secs` field, per-language opt-out in `[lsp.<lang>]` config. Coherence test validates LSP state consistency. Keep experimental until broader soak.

---

## Metadata-Enriched Chunks

### Chunk metadata column + header prefix
**Status:** ⚠ Experimental  
**Experimental doc:** `docs/manual/src/experimental/metadata-enriched-chunks.md`  
**Key commits:** `44b9189`→`22a4616` (RawChunk metadata field → persist + embed text prefix)  
**Note:** Adds `metadata` column to `chunks` table (migration-guarded). Embed text prefixed with `{file} > {container} > {kind}` header for better KNN recall. `build_metadata_header`, `kind_keyword_for_node`, `extract_signature` helpers.

### Asymmetric query prefix (CodeRankEmbed)
**Status:** ⚠ Experimental  
**Experimental doc:** `docs/manual/src/experimental/asymmetric-query-prefix.md`  
**Key commit:** `7c00eaa`  
**Note:** Prepends `"Represent this query for searching relevant code: "` to query embeddings only. Required for CodeRankEmbed-style models. Harmless for symmetric models. Keep experimental until model family confirmed.

---

## Semantic Search

### Per-file diversity cap
**Status:** ✅ Ready  
**Experimental doc:** `docs/manual/src/experimental/file-diversity-rerank.md`  
**Key commit:** `a64197a` (+ earlier `ba07b05` codescout-embed side)  
**Note:** Caps same-file chunks at 3 (default) to prevent one large file monopolising top-K. Overfetches by cap factor, trims to `limit`. Three unit tests.

---

## librarian-mcp (New Crate)

### Full workspace artifact registry MCP server
**Status:** ⚠ Experimental (ship as a unit)  
**Experimental doc:** `docs/manual/src/experimental/librarian-mcp.md`  
**Key commits:** `5660add` scaffold → `8d1e0ed` (entire crate history, ~80 commits)

**What's included:**
- SQLite catalog with schema migrations + artifact CRUD
- Filter AST + SQL compiler (and/or/not, all field ops)
- Frontmatter parser + `update_in_place` round-trip
- Classification rules (TOML → globset)
- Indexer: repo walker + classify + upsert + SHA shortcut (BUG-046 fixed)
- sqlite-vec embedding + KNN search with cascade delete trigger
- Read tools: `artifact_get`, `artifact_find`, `artifact_list_by_kind`, `artifact_links`, `artifact_graph`, `librarian_context`
- Write tools: `artifact_create`, `artifact_update`, `artifact_link`, `artifact_observe`
- `librarian_reindex` with parallel embedding + `--force` CLI flag
- `import-codescout` CLI subcommand
- POSIX path normalisation + orphan repo cleanup
- Path-traversal guard on `artifact_create`
- `artifact_get` kind-aware previews: spec (headings+summary), plan (checklist progress), memory (observations), default (first paragraph)
- BUG-045: vec0 upsert via DELETE+INSERT
- BUG-046: classify-before-SHA-check fix
- MCP subprocess integration test
- Server instructions (`server_instructions.md`)

**Promotion note:** Promote as a single feature block. Depends on `codescout-embed` crate extraction and cargo workspace conversion being on master first.

---

## Summary Table

| Cluster | Status | Notes |
|---------|--------|-------|
| Cargo workspace | ✅ Promoted | |
| jemalloc | ✅ Promoted | |
| codescout-embed extraction | ✅ Promoted | |
| Cancel handling | ✅ Promoted | |
| BUG-035/037/039/040/041/042/043/044 fixes | ✅ Promoted | |
| Three-level guidance taxonomy | ✅ Promoted | |
| Prompt efficiency overhaul | ✅ Promoted | |
| MCP Resources (doc/memory/project/progress) | ✅ Promoted | |
| read_markdown improvements | ✅ Promoted | |
| list_symbols progressive dir | ✅ Promoted | |
| Bash/shell support | ✅ Promoted | |
| Write serialization | ✅ Promoted | |
| Per-file diversity cap | ✅ Promoted | |
| list_dir gitignore fix | ✅ Promoted | Promoted as-is; review behaviour post-promotion |
| Global config | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| Index scope guard / preflight | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| LSP mux — rust rollout | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| Metadata-enriched chunks | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| Asymmetric query prefix | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| librarian-mcp (full crate) | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
| doctor://tool-usage | ⚠ Promoted (experimental) | Docs carry ⚠ callout |
