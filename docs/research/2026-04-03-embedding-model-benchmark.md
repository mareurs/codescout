# Embedding Model Benchmark for Semantic Search

**Date:** 2026-04-03
**Status:** Active
**Purpose:** Reproducible quality comparison of embedding models for codescout's semantic search.

## Methodology

### Setup
1. Configure model in `.codescout/project.toml` under `[embeddings]`
2. Run `index_project(force: true)` — wait for completion
3. Run each test case via `semantic_search(query)` (default top-10)
4. Score each result set against the expected files

### Scoring

Each test case has **expected files** — the ground-truth files that a good search should surface.

**Per-query score (0-3):**
- **3** — All expected files appear in top 5
- **2** — All expected files appear in top 10, or majority in top 5
- **1** — At least one expected file in top 10
- **0** — No expected files in top 10

**Model score** = sum of all 20 query scores. Maximum = 60.

### What to record per model

| Field | Description |
|-------|-------------|
| Model | Full model string (e.g. `local:AllMiniLML6V2Q`) |
| Dimensions | Vector dimensionality |
| Index time | Wall-clock time for `index_project(force: true)` |
| Chunk count | From `index_status()` after indexing |
| DB size | `ls -lh .codescout/embeddings/project.db` |
| Total score | Sum of 20 query scores (max 60) |
| Per-query scores | Array of 20 individual scores |

---

## Test Cases

### Tier 1: Direct Concept (1-5)

Single concept, expects exact-match files. Tests basic retrieval.

#### TC-01: Exact type name
- **Query:** `RecoverableError`
- **Concepts:** Named type lookup
- **Expected files:**
  - `src/tools/mod.rs` (definition + impl)
  - `src/server.rs` (routing tests)
  - `docs/FEATURES.md` (documentation)

#### TC-02: Single feature area
- **Query:** `embedding model configuration`
- **Concepts:** Configuration subsystem
- **Expected files:**
  - `src/embed/mod.rs` (Embedder trait)
  - `docs/manual/src/configuration/embeddings.md`
  - `docs/manual/src/configuration/embedding-backends.md`

#### TC-03: Named module
- **Query:** `LSP client implementation`
- **Concepts:** Specific module
- **Expected files:**
  - `src/lsp/client.rs`
  - `src/lsp/ops.rs`
  - `src/lsp/manager.rs`

#### TC-04: Specific tool name
- **Query:** `run_command shell execution`
- **Concepts:** Tool by name
- **Expected files:**
  - `src/tools/workflow.rs` (RunCommand — was `src/tools/command.rs`, merged 2026-04)
  - `docs/manual/src/concepts/shell-integration.md`
  - `docs/manual/src/concepts/output-buffers.md`

#### TC-05: Data structure
- **Query:** `OutputGuard progressive disclosure capping`
- **Concepts:** Named pattern
- **Expected files:**
  - `src/tools/output.rs`
  - `docs/PROGRESSIVE_DISCOVERABILITY.md`

---

### Tier 2: Two-Concept Composition (6-12)

Requires understanding the relationship between two concepts.

#### TC-06: Feature + storage
- **Query:** `how are tool calls recorded in the usage database`
- **Concepts:** Usage tracking + SQLite schema
- **Expected files:**
  - `src/usage/db.rs`
  - `src/usage/mod.rs`
  - `docs/plans/2026-04-02-usage-traceability-design.md`

#### TC-07: Algorithm + domain
- **Query:** `section boundary detection in markdown editing`
- **Concepts:** Heading parsing + edit operations
- **Expected files:**
  - `src/tools/markdown.rs` (compute_section_end, perform_section_edit)
  - `src/tools/file_summary.rs` (parse_all_headings, heading_level)

#### TC-08: Migration + error
- **Query:** `dimension mismatch when switching embedding models`
- **Concepts:** Schema migration + model change
- **Expected files:**
  - `src/embed/index.rs` (build_index dimension check, maybe_migrate_to_vec0)
  - `src/embed/schema.rs`

#### TC-09: Security + feature
- **Query:** `dangerous command detection and safety checks`
- **Concepts:** Shell security + command validation
- **Expected files:**
  - `src/util/path_security.rs` (is_dangerous_command, check_tool_access)
  - `src/tools/workflow.rs` (RunCommand dangerous-check integration — was `src/tools/command.rs`, merged 2026-04)

#### TC-10: Pattern + overflow
- **Query:** `how overflow hints guide the agent to narrow results`
- **Concepts:** Progressive disclosure + agent guidance
- **Expected files:**
  - `src/tools/output.rs` (OutputGuard)
  - `docs/PROGRESSIVE_DISCOVERABILITY.md`
  - `src/prompts/server_instructions.md`

#### TC-11: Refactoring operation
- **Query:** `renaming a symbol across all references in the codebase`
- **Concepts:** LSP rename + file mutation
- **Expected files:**
  - `src/tools/symbol.rs` (RenameSymbol — was `src/tools/symbol_edit.rs`, merged 2026-04)
  - `src/lsp/ops.rs`

#### TC-12: Configuration + resolution
- **Query:** `how the embedding URL and model prefix determine which backend is used`
- **Concepts:** Config resolution order + backend selection
- **Expected files:**
  - `src/embed/mod.rs` (backend resolution)
  - `docs/manual/src/configuration/embeddings.md` (resolution order section)

---

### Tier 3: Multi-Concept Cross-Cutting (13-17)

Three or more concepts; requires understanding architectural patterns.

#### TC-13: Crash + recovery + routing
- **Query:** `what happens when an LSP server crashes mid-request and how does the circuit breaker recover`
- **Concepts:** LSP lifecycle + error handling + resilience
- **Expected files:**
  - `src/lsp/client.rs`
  - `src/lsp/manager.rs`
  - `docs/manual/src/troubleshooting.md`

#### TC-14: Dispatch + error classification
- **Query:** `how does the tool dispatch pipeline handle both recoverable errors and fatal failures differently`
- **Concepts:** Tool trait + call_content + error routing
- **Expected files:**
  - `src/tools/mod.rs` (Tool trait, route_tool_error)
  - `src/server.rs` (dispatch + error tests)
  - `src/usage/mod.rs` (outcome classification)

#### TC-15: Force rebuild + migration
- **Query:** `end-to-end force re-indexing flow including dimension migration and vec0 table recreation`
- **Concepts:** Indexing pipeline + schema migration + vec0
- **Expected files:**
  - `src/embed/index.rs` (build_index, maybe_migrate_to_vec0)
  - `src/embed/mod.rs`

#### TC-16: Search pipeline
- **Query:** `how a semantic search query flows from input through embedding to KNN ranked results`
- **Concepts:** Embed → vec0 → search_scoped → ranking
- **Expected files:**
  - `src/tools/semantic.rs` (SemanticSearch tool — was `src/tools/search.rs`, renamed 2026-04)
  - `src/embed/index.rs` (search_scoped_vec0, search_multi_db)
  - `src/embed/mod.rs`

#### TC-17: Plugin integration
- **Query:** `how does the companion plugin route native Read and Grep calls to codescout MCP tools`
- **Concepts:** PreToolUse hooks + routing plugin + tool redirection
- **Expected files:**
  - `docs/manual/src/concepts/routing-plugin.md`
  - `docs/manual/src/getting-started/companion-plugin.md`

---

### Tier 4: Architectural Insight (18-20)

Requires understanding design decisions, consistency invariants, and cross-module patterns.

#### TC-18: Dual-path consistency
- **Query:** `why heading detection in parse_all_headings and compute_section_end must use the same code block tracking`
- **Concepts:** Two code paths that must agree + fenced block state
- **Expected files:**
  - `src/tools/markdown.rs` (compute_section_end)
  - `src/tools/file_summary.rs` (parse_all_headings)
  - `docs/TODO-tool-misbehaviors.md` (BUG-035)

#### TC-19: Activation wiring
- **Query:** `relationship between project activation, LSP server lifecycle, and tool context wiring`
- **Concepts:** Agent state + ActiveProject + ToolContext + LspManager
- **Expected files:**
  - `src/agent/mod.rs` (Agent, ActiveProject — was `src/agent.rs`, split to module 2026-04)
  - `src/lsp/manager.rs` (LspManager)
  - `src/server.rs` (ToolContext construction)

#### TC-20: Prompt surface consistency
- **Query:** `how to keep the three prompt surfaces consistent when tools are renamed or behavior changes`
- **Concepts:** server_instructions.md + onboarding_prompt.md + build_system_prompt_draft
- **Expected files:**
  - `src/prompts/server_instructions.md`
  - `src/prompts/onboarding_prompt.md`
  - `src/tools/workflow.rs` (build_system_prompt_draft)

---

## Results

### Model: local:AllMiniLML6V2Q


| Field | Value |
|-------|-------|
| Dimensions | 384 |
| Context window | 256 tokens |
| Index time | ~70 seconds |
| Chunk count | 32,098 |
| DB size | 71 MB |
| **Total score** | **34/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | 2 | FEATURES.md #1 (RecoverableError docs), mod.rs #3+#5+#6+#7 (struct + tests). server.rs missed |
| 02 | 1 | embeddings.md #3+#7, config/project.rs #6 (EmbeddingsSection). embed/mod.rs missed |
| 03 | 3 | All 3: lsp/client.rs #2+#8+#9, lsp/ops.rs #6 (LspClientOps), lsp/manager.rs #3 |
| 04 | 2 | workflow-and-config.md #1+#2, output-buffers.md #4, workflow.rs #10. shell-integration.md missed |
| 05 | 3 | output.rs #6+#8, PROGRESSIVE_DISC.md #2+#4. Strong across code + docs |
| 06 | 1 | ARCHITECTURE.md #1 (Usage Recorder section), FEATURES.md #2, traceability-design #6. usage/db.rs missed |
| 07 | 2 | document-section-editing.md #1, file_summary.rs #6, BUG-035 #8, markdown.rs #9 |
| 08 | 2 | embed/index.rs #3+#5+#6+#7 (dimension check code), embed/mod.rs #4. schema.rs missed |
| 09 | 2 | path_security.rs #1+#2+#3+#8 (dominates). command.rs missed |
| 10 | 1 | research-progressive-disclosure #1, PROGRESSIVE_DISC.md #2+#3. output.rs and server_instructions.md missed |
| 11 | 2 | symbol-navigation.md #2, editing.md #3, symbol.rs #7+#8+#10. lsp/ops.rs missed |
| 12 | 2 | config/project.rs #2+#3+#4+#9 (EmbeddingsSection), embeddings.md #7. embed/mod.rs missed |
| 13 | 1 | lsp/manager.rs #6+#9, kotlin-lsp-mux #2, server.rs #3+#7+#10. lsp/client.rs and troubleshooting.md missed |
| 14 | 1 | server.rs #6+#7+#8+#9 (route_tool_error), FEATURES.md #2+#4. tools/mod.rs and usage/mod.rs missed |
| 15 | 2 | vec0-migration.md #1+#2+#3+#4 (perfect!), embed/index.rs #5+#6+#7+#8+#9. embed/mod.rs missed |
| 16 | 0 | No expected source files. Docs about semantic search dominate. search.rs, embed/index.rs both missed |
| 17 | 2 | routing-plugin.md #5+#10, companion-plugin.md #8, CLAUDE.md #7, agents/claude-code.md #3 |
| 18 | 3 | All 3: markdown.rs #3+#8+#10, file_summary.rs #5 (parse_all_headings_skips_code_blocks), BUG-035 #2+#4 |
| 19 | 1 | lsp/manager.rs #8+#10, tools/mod.rs #9 (ToolContext). agent.rs and server.rs missed |
| 20 | 1 | CLAUDE.md #1+#4 (Prompt Surface Consistency!), api-naming #3, onboarding-versioning #5+#7. All 3 actual files missed |

**Observations:**
- Better on code-level queries (TC-01: 2 vs 1, TC-03: 3 vs 2) — smaller chunks focus on individual declarations
- Weaker on concept composition (TC-10: 1 vs 3, TC-13: 1 vs 3) — 256-token context misses broader context
- No "closing brace `}`" noise — smaller chunks rarely end with boilerplate
- Finds config/project.rs for embedding queries (TC-02, TC-12) — nomic-embed-code misses this
- Both models completely fail on TC-16 (search pipeline source code)
### Model: nomic-embed-code (Q4_K_M, via llama.cpp on AMD GPU)


| Field | Value |
|-------|-------|
| Dimensions | 3584 |
| Context window | 32,768 tokens |
| Index time | ~25 minutes |
| Chunk count | 11,868 |
| DB size | 372 MB |
| **Total score** | **36/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | 1 | mod.rs #1 (RecoverableError impl), rest are generic closing-brace chunks |
| 02 | 1 | embed/mod.rs #1, but no config docs surfaced — drowned by generic chunks |
| 03 | 2 | lsp/client.rs #1, lsp/manager.rs #2+#3. ops.rs missed |
| 04 | 2 | workflow.rs (RunCommand) #3+#4, shell-integration.md #2. output-buffers missed |
| 05 | 3 | output.rs dominates (3 hits), PROGRESSIVE_DISCOVERABILITY.md #10 |
| 06 | 1 | traceability design doc #6, but usage/db.rs and usage/mod.rs missed entirely |
| 07 | 2 | markdown.rs #1+#5, BUG-035 #6, file_summary.rs missed |
| 08 | 2 | embed/index.rs #1 (model mismatch test), config docs #2+#3. schema.rs missed |
| 09 | 2 | path_security.rs dominates top 5 (3 hits). command.rs missed |
| 10 | 3 | Overflow hint pattern #1, PROGRESSIVE_DISC.md #2, output.rs #6, output-modes.md #5 |
| 11 | 2 | symbol.rs #2+#3+#5+#7 (RenameSymbol). lsp/ops.rs missed |
| 12 | 3 | embed/mod.rs #1 (resolution logic), embeddings.md #3, unified-config specs #5+#6 |
| 13 | 3 | All 3: lsp/client.rs #3, lsp/manager.rs #9+#10, troubleshooting.md #2 |
| 14 | 2 | server.rs #5+#7 (route_tool_error), usage/mod.rs→tools/usage.rs #1. tools/mod.rs missed |
| 15 | 2 | embed/index.rs #8 (dimension check code), vec0-migration.md #7. embed/mod.rs missed |
| 16 | 0 | No source files. All docs/concept pages about semantic search. search.rs totally missed |
| 17 | 3 | routing-plugin.md #1, companion-plugin.md #5, CLAUDE.md #2 |
| 18 | 2 | file_summary.rs #1+#3+#4+#5+#6+#10 (parse_all_headings tests/impl). markdown.rs #2. BUG-035 missed |
| 19 | 0 | No expected source files. Got workspace.rs, config.rs instead of agent.rs/server.rs |
| 20 | 1 | Prompt Surface docs dominate but the 3 actual files (server_instructions.md, onboarding_prompt.md, workflow.rs) all missed |

**Observations:**
- Strong on docs-heavy queries (TC-10, 12, 13, 17) — good at matching concept-level headings
- Weak on source-only queries (TC-16, 19) — tends to return docs about the concept instead of the implementation
- Many results are generic closing-brace `}` chunks (TC-01, 02) — large chunk windows include trailing boilerplate
- Best at cross-cutting queries that have both code and doc matches (TC-13, 18)
- The 32K context window creates some "kitchen sink" chunks that match broadly but imprecisely
### Model: nomic-embed-text (F16, via Ollama)

| Field | Value |
|-------|-------|
| Dimensions | 768 |
| Context window | 8,192 tokens |
| Index time | ~60 seconds |
| Chunk count | 11,887 |
| DB size | 55 MB |
| **Total score** | **32/60** |

| TC | Score | Notes |
|----|-------|-------|
| 01 | 3 | All 3: mod.rs #1+#5+#7+#8+#9+#10, server.rs #7, FEATURES.md #6. Best result of all models |
| 02 | 2 | embed/mod.rs #6, embedding-backends.md #2+#3, config/project.rs #4. embeddings.md missed |
| 03 | 2 | lsp/client.rs #2+#9, lsp/ops.rs #6 (LspProvider trait). lsp/manager.rs missed |
| 04 | 2 | shell-integration.md #5, output-buffers.md #8, workflow.rs #10. All in top 10 |
| 05 | 3 | output.rs #5+#10, PROGRESSIVE_DISC.md #4. Clean |
| 06 | 1 | traceability-design.md #1, FEATURES.md #2, ARCHITECTURE.md #3. usage/db.rs missed |
| 07 | 1 | markdown.rs #7 only. file_summary.rs missed. Mostly doc results |
| 08 | 2 | embed/index.rs #2+#6 (mismatch tests), embed/mod.rs #1. schema.rs missed |
| 09 | 2 | path_security.rs #1+#2+#4+#6+#7+#8 (dominates). command.rs missed |
| 10 | 1 | PROGRESSIVE_DISC.md #3+#6+#7, format.rs #9. output.rs and server_instructions.md missed |
| 11 | 1 | symbol.rs #5 only. Mostly doc results (editing.md #1, tool-workflows #9) |
| 12 | 2 | embeddings.md #3+#7, embedding-backends.md #2+#8, unified-config specs #6+#9. embed/mod.rs missed |
| 13 | 0 | No expected files. server.rs #1 (test name match), lsp-idle-ttl docs dominate |
| 14 | 2 | tools/mod.rs #6, server.rs #7. Both found! research-progressive-disclosure #1, tool-trait.md #9 |
| 15 | 1 | embed/index.rs #10 (vec0 migration test). vec0-migration docs dominate (#1-#7) |
| 16 | 0 | No source files. All docs about semantic search |
| 17 | 3 | companion-plugin.md #1, routing-plugin.md #5+#7+#9, agents/claude-code.md #4 |
| 18 | 3 | All 3: file_summary.rs #3+#4+#5 (parse_all_headings), markdown.rs #9, BUG-035 #2 |
| 19 | 0 | No expected source files. workspace-multi-project design dominates |
| 20 | 1 | CLAUDE.md #1 (Prompt Surface Consistency). All 3 actual files missed |

**Observations:**
- Best TC-01 score of all models (3) — 768 dims with 8K context balances precision and breadth
- Struggles on cross-cutting source queries (TC-13, 19: score 0) — same as other models
- Doc-heavy like nomic-embed-code but without the `}` noise problem
- Smaller chunks than nomic-embed-code (8K vs 32K) but larger than AllMiniLML6V2Q (256 tok)
- Best storage efficiency: 55 MB (vs 71 MB mini, 372 MB code)

### Model: CodeRankEmbed (via local server, http://localhost:43300)

| Field | Value |
|-------|-------|
| Model string | `CodeRankEmbed` + `url = "http://localhost:43300/v1"` |
| Dimensions | 768 |
| Context window | 4096 tokens |
| Index time | ~1 min (fast — GPU accelerated) |
| Chunk count | 20,840 |
| DB size | 173 MB |
| Query prefix | `"Represent this query for searching relevant code: "` — auto-detected from model name |
| **Total score** | **23/60** |

| TC | Score | Notes |
|----|-------|-------|
| TC-01 | 1 | mod.rs ✓ (top 3), server.rs ✗, FEATURES.md ✗ |
| TC-02 | 1 | embedding-backends.md ✓ (#1), embed/mod.rs ✓ (#10), embeddings.md ✗ |
| TC-03 | 2 | All 3 LSP files in top 10; client.rs+manager.rs in top 5 |
| TC-04 | 1 | shell-integration.md ✓ (#10), command.rs ✗ |
| TC-05 | 1 | output.rs ✓ (#2,#5), PROGRESSIVE_DISCOVERABILITY.md ✗ |
| TC-06 | 1 | db.rs ✓ (#8), traceability.md ✓ (#10), usage/mod.rs ✗ |
| TC-07 | 1 | markdown.rs ✓ (#6,#10), file_summary.rs ✗ |
| TC-08 | 1 | index.rs ✓ (#1,#2), schema.rs ✗ |
| TC-09 | 1 | path_security.rs dominates (6 hits), command.rs ✗ |
| TC-10 | 0 | output.rs ✗, PROGRESSIVE.md ✗, server_instructions.md ✗ |
| TC-11 | 1 | symbol.rs ✓ (#1, contains RenameSymbol), ops.rs ✗ |
| TC-12 | 1 | embeddings.md ✓ (#3), embed/mod.rs ✗ |
| TC-13 | 2 | client.rs ✓ (#1), manager.rs ✓ (#3) — 2/3 in top 5; troubleshooting.md ✗ |
| TC-14 | 1 | server.rs ✓ (#5,#9), mod.rs ✗, usage/mod.rs ✗ |
| TC-15 | 1 | index.rs ✓ (#4+), mod.rs ✗ |
| TC-16 | 1 | semantic.rs ✓ (#9,#10), index.rs ✗, mod.rs ✗ |
| TC-17 | 3 | companion-plugin.md ✓ (#1), routing-plugin.md ✓ (#4,#5) — both top 5 |
| TC-18 | 3 | misbehaviors.md ✓ (#1), markdown.rs ✓ (#2), file_summary.rs ✓ (#3) — all top 5 |
| TC-19 | 0 | agent.rs ✗, lsp/manager.rs ✗, server.rs ✗ |
| TC-20 | 0 | server_instructions.md ✗, onboarding_prompt.md ✗, workflow.rs ✗ |

**Tier scores:** T1=6/15 · T2=6/21 · T3=8/15 · T4=3/9

**Notes:**
- Chunk size not tuned for CodeRankEmbed (project default). 4096-token context model may benefit from larger chunks — could improve Tier 2/3.
- TC-10, TC-19, TC-20 all 0 — same structural gap as other models.
- Query prefix auto-detected from model name; applied only on query side, not during indexing.

---

### Model: CodeRankEmbed + metadata-enriched chunks (2026-04-20)

Same model, same server. Delta vs. baseline: each chunk now embeds with a
`file_path :: container :: kind name(signature)` header prepended (see
`docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md`).
Container decomposition also always triggers now (not only when oversized),
so methods inside `impl`/class blocks get their own chunk with container
context.

| Field | Value |
|-------|-------|
| Model string | `CodeRankEmbed` + `url = "http://localhost:43300/v1"` |
| Dimensions | 768 |
| Index time | ~5 min (with pipelined producer/consumer) |
| Chunk count | 24,335 (+17% vs 20,840 baseline) |
| DB size | ~200 MB |
| Query prefix | same as baseline |
| **Total score** | **22/60** (−1 vs. 23/60 baseline) |

| TC | Score | Δ | Notes |
|----|-------|---|-------|
| TC-01 | 1 | = | mod.rs dominates top 5; server.rs at rank 10; FEATURES.md ✗ |
| TC-02 | 1 | = | embedding-backends.md ✓ (#1, #4, #5); embed/mod.rs ✗; embeddings.md ✗ |
| TC-03 | 1 | −1 | client.rs saturates all 10 slots — manager.rs + ops.rs crowded out |
| TC-04 | 0 | −1 | workflow.rs dominates (run_command Tool moved); command.rs + shell-integration.md ✗ |
| TC-05 | 1 | = | output.rs ✓ (#4, #6, #7, #10); PROGRESSIVE.md ✗ |
| TC-06 | 1 | = | usage/db.rs ✓ (#6, #9); mcp_resources/tool_usage.rs dominates; mod.rs ✗ |
| TC-07 | 1 | = | markdown.rs ✓ (#2, #6, #7, #8, #9); file_summary.rs ✗ |
| TC-08 | 1 | = | index.rs ✓ (#1, #2, #5, #6, #7); schema.rs ✗ |
| TC-09 | 1 | = | path_security.rs ✓ top-8; command.rs ✗ |
| TC-10 | 1 | +1 | output.rs ✓ (#10); PROGRESSIVE.md ✓ (#7); server_instructions.md ✗ |
| TC-11 | 1 | = | symbol.rs dominates (contains RenameSymbol); ops.rs ✗ |
| TC-12 | 1 | = | embeddings.md ✓ (#2, #9); embed/mod.rs ✗ |
| TC-13 | 1 | −1 | manager.rs saturates all 10 slots — client.rs + troubleshooting.md crowded out |
| TC-14 | 1 | = | server.rs ✓ (#7, #8, #9); tools/mod.rs ✗; usage/mod.rs ✗ |
| TC-15 | 1 | = | index.rs ✓ (#1, #3, #4, #6, #8, #9); embed/mod.rs ✗ |
| TC-16 | 1 | = | semantic.rs ✓ (#7, #10, counted as search.rs); index.rs ✗; mod.rs ✗ |
| TC-17 | 3 | = | companion-plugin.md ✓ (#1); routing-plugin.md ✓ (#3, #5) |
| TC-18 | 3 | = | misbehaviors.md ✓ (#1); markdown.rs ✓ (#2); file_summary.rs ✓ (#3) |
| TC-19 | 1 | +1 | lsp/manager.rs ✓ (#10); agent.rs + server.rs ✗ |
| TC-20 | 0 | = | server_instructions.md ✗; onboarding_prompt.md ✗; workflow.rs ✗ |

**Tier scores:** T1=4/15 · T2=7/21 · T3=7/15 · T4=4/9

**Observations:**

- Net change −1 / 60 — essentially flat. Metadata did not move the needle on
  this file-recall rubric.
- Wins: TC-10 (+1) and TC-19 (+1). Both are queries where the signature line
  made a single relevant chunk more findable (`OutputGuard` header in
  `output.rs`, `lsp/manager.rs` activation wiring).
- Losses: TC-03 (−1), TC-04 (−1), TC-13 (−1). In each case one file saturated
  the top 10, crowding out sibling files that the benchmark expected. Metadata
  amplified "this file is highly relevant" signal so much that file diversity
  collapsed.
- Chunk count +17%: container decomposition now always triggers, yielding many
  more method-level chunks. This is the likely cause of the diversity collapse
  — when one container has 10 methods each with its own metadata-enriched
  chunk, the top-10 easily fills with that one file.
- Ship gate (≥30/60 per plan) NOT met. Score regressed slightly.

**Decision:** Keep metadata-enriched chunks on `experiments` branch. Do not
cherry-pick to `master` in current form. Possible follow-ups:
  1. File-diversity boost (MMR-style re-rank or per-file cap in semantic_search
     output — e.g. "max N chunks per file in top K").
  2. Less aggressive container decomposition — only split when oversized
     (reverting half of Task 6). Container context in metadata still useful
     for the header chunk.
  3. Re-benchmark after any of the above.

The feature IS a latent win for code navigation (header-based retrieval is
clearly working — look at TC-10 and TC-19) but the granularity increase
undercuts it on file-recall metrics. Diversity post-processing would likely
recover the baseline and then some.

---
### Model: CodeRankEmbed — post-93-commit reindex (2026-04-21)

Same model, same server as the 2026-04-03 baseline. Codebase advanced 93
commits since the metadata-enriched run (2026-04-20). No chunking or
embedding changes — pure codebase growth effect.

| Field | Value |
|-------|-------|
| Model string | `CodeRankEmbed` + `url = "http://localhost:43300/v1"` |
| Dimensions | 768 |
| Index time | ~5 min (force rebuild) |
| Chunk count | 26,398 (+8.5% vs 24,335 metadata-enriched, +27% vs 20,840 baseline) |
| DB size | 202 MB |
| Query prefix | same as baseline (auto-detected) |
| **Total score** | **27/60** (+4 vs 23/60 baseline, +5 vs 22/60 metadata-enriched; +1 from TC-09 expected correction command.rs→workflow.rs) |

| TC | Score | Δ vs baseline | Notes |
|----|-------|---------------|-------|
| TC-01 | 2 | +1 | mod.rs ✓ (#1,#2), server.rs ✓ (#5) — majority in top 5; FEATURES.md ✗ |
| TC-02 | 1 | = | embedding-backends.md ✓ (#1,#4), embeddings.md ✓ (#10), embed/mod.rs ✗ |
| TC-03 | 2 | = | client.rs ✓ (#1,#2), manager.rs ✓ (#6), ops.rs ✓ (#7) — all in top 10 |
| TC-04 | 1 | = | shell-integration.md ✓ (#7), workflow.rs ✓ (#5,#8,#9), output-buffers.md ✗ — expected updated: command.rs → workflow.rs |
| TC-05 | 1 | = | output.rs ✓ (#5,#6), PROGRESSIVE.md ✗ |
| TC-06 | 1 | = | usage/db.rs ✓ (#4), usage/mod.rs ✗, traceability plan ✗ |
| TC-07 | 1 | = | markdown.rs ✓ (#1,#6), file_summary.rs ✗ |
| TC-08 | 1 | = | index.rs ✓ (#1,#2), schema.rs ✗ |
| TC-09 | 2 | +1 | path_security.rs ✓ (#1,#2), workflow.rs ✓ (#4,#6) — expected updated: command.rs → workflow.rs |
| TC-10 | 1 | +1 | PROGRESSIVE.md ✓ (#5), output.rs ✗, server_instructions.md ✗ |
| TC-11 | 1 | = | symbol.rs ✓ (#1,#2, contains RenameSymbol), ops.rs ✗ |
| TC-12 | 1 | = | embeddings.md ✓ (#2), embed/mod.rs ✗ |
| TC-13 | 2 | = | manager.rs ✓ (#1,#2), client.rs ✓ (#3) — majority in top 5; troubleshooting.md ✗ |
| TC-14 | 1 | = | server.rs ✓ (#6), tools/mod.rs ✗, usage/mod.rs ✗ |
| TC-15 | 1 | = | index.rs ✓ (#1,#3), embed/mod.rs ✗ |
| TC-16 | 1 | = | semantic.rs ✓ (#7,#10), index.rs ✗, mod.rs ✗ |
| TC-17 | 3 | = | companion-plugin.md ✓ (#1), routing-plugin.md ✓ (#3,#5) — both top 5 |
| TC-18 | 3 | = | misbehaviors.md ✓ (#1), markdown.rs ✓ (#2), file_summary.rs ✓ (#3) — all top 5 |
| TC-19 | 1 | +1 | manager.rs ✓ (#9), agent.rs ✗, server.rs ✗ |
| TC-20 | 0 | = | server_instructions.md ✗, onboarding_prompt.md ✗, workflow.rs ✗ |

**Tier scores:** T1=7/15 · T2=8/21 · T3=8/15 · T4=4/9

**Observations:**

- +3 vs CodeRankEmbed baseline driven entirely by codebase growth, not model
  change. New documentation and code added in 93 commits pulled two previously
  missing files into search range (TC-10, TC-19) and bumped TC-01 from 1→2 by
  making server.rs appear at rank #5.
- Persistent gaps: TC-20 (0/3) remains structural — prompt surface files
  (server_instructions.md, onboarding_prompt.md, workflow.rs) are never
  surfaced. TC-04 still misses command.rs despite the query naming the tool.
- TC-13 recovered to 2 (same as baseline): no longer has the diversity
  collapse seen in the metadata-enriched run where manager.rs saturated all 10
  slots.
- Ship gate (≥30/60 per plan) still NOT met. Score improved but gap remains.
- Chunk count +27% vs original baseline while score is only +13% — diminishing
  returns from raw size alone. Diversity post-processing (MMR / per-file cap)
  or better chunking strategy needed to close the remaining gap.

---
### Model: CodeRankEmbed — reindex run 2 (2026-04-21, commit 1561661a)

Repeat run after another force reindex. Same model, same codebase commit.
TC-09 expected corrected to `workflow.rs` (command.rs no longer exists).

| Field | Value |
|-------|-------|
| Model string | `CodeRankEmbed` + `url = "http://localhost:43300/v1"` |
| Chunk count | 26,409 |
| DB size | 202 MB |
| **Total score** | **27/60** |

| TC | Score | Δ vs run 1 (corrected) | Notes |
|----|-------|------------------------|-------|
| TC-01 | 2 | = | mod.rs ✓ (#1,#2), server.rs ✓ (#5); FEATURES.md ✗ |
| TC-02 | 1 | = | embedding-backends.md ✓ (#1,#3), embeddings.md ✓ (#10), embed/mod.rs ✗ |
| TC-03 | 2 | = | client.rs ✓ (#1,#2), manager.rs ✓ (#6,#9), ops.rs ✓ (#8) |
| TC-04 | 1 | = | shell-integration.md ✓ (#5), workflow.rs ✓ (#6,#7), output-buffers.md ✗ |
| TC-05 | 1 | = | output.rs ✓ (#5,#6), PROGRESSIVE.md ✗ |
| TC-06 | 1 | = | usage/db.rs ✓ (#4,#7), usage/mod.rs ✗, plan ✗ |
| TC-07 | 1 | = | markdown.rs ✓ (#2,#6), file_summary.rs ✗ |
| TC-08 | 1 | = | index.rs ✓ (#1,#2), schema.rs ✗ |
| TC-09 | 2 | = | path_security.rs ✓ (#1,#2), workflow.rs ✓ (#4,#6) |
| TC-10 | 1 | = | PROGRESSIVE.md ✓ (#7), output.rs ✗, server_instructions.md ✗ |
| TC-11 | 1 | = | symbol.rs ✓ (#1,#2), ops.rs ✗ |
| TC-12 | 1 | = | embeddings.md ✓ (#2), embed/mod.rs ✗ |
| TC-13 | 2 | = | manager.rs ✓ (#1,#2), client.rs ✓ (#5), troubleshooting.md ✗ |
| TC-14 | 1 | = | server.rs ✓ (#7,#9), tools/mod.rs ✗, usage/mod.rs ✗ |
| TC-15 | 1 | = | index.rs ✓ (#1,#3), embed/mod.rs ✗ |
| TC-16 | 1 | = | semantic.rs ✓ (#7,#10), index.rs ✗, mod.rs ✗ |
| TC-17 | 3 | = | companion-plugin.md ✓ (#2), routing-plugin.md ✓ (#3,#5) |
| TC-18 | 3 | = | misbehaviors.md ✓ (#1), markdown.rs ✓ (#2), file_summary.rs ✓ (#3) |
| TC-19 | 1 | = | manager.rs ✓ (#9), agent/mod.rs ✗, server.rs ✗ |
| TC-20 | 0 | = | all ✗ |

**Tier scores:** T1=7/15 · T2=8/21 · T3=8/15 · T4=4/9

**Observations:** Identical to run 1 (corrected). Score stable across two independent force rebuilds — confirms 27/60 is a reliable baseline for this codebase state. Ship gate ≥30/60 not met.

---
### Model: *(template for additional models)*

| Field | Value |
|-------|-------|
| Dimensions | |
| Context window | |
| Index time | |
| Chunk count | |
| DB size | |
| **Total score** | **_/60** |

*(Copy the TC scoring table from above)*

---

## Head-to-Head Comparison (2026-04-03)

### Score by Tier


| Tier | AllMiniLML6V2Q | nomic-embed-text | nomic-embed-code | CodeRankEmbed | Max |
|------|---------------|-----------------|------------------|---------------|-----|
| 1 (Direct Concept) | 11/15 | **12/15** | 9/15 | 6/15 | 15 |
| 2 (Two-Concept) | 12/21 | 12/21 | **17/21** | 6/21 | 21 |
| 3 (Cross-Cutting) | 6/15 | 5/15 | 7/15 | **8/15** | 15 |
| 4 (Architectural) | **5/9** | 4/9 | 3/9 | 3/9 | 9 |
| **Total** | **34/60** | **32/60** | **36/60** | **23/60** | **60** |
### Where Each Model Wins

| Query | AllMiniLML6V2Q | nomic-embed-code | CodeRankEmbed | Winner | Why |
|-------|---------------|------------------|---------------|--------|-----|
| TC-01 | 2 | 1 | 1 | Mini | Smaller chunks focus on declarations, avoid `}` noise |
| TC-03 | 3 | 2 | 2 | Mini | All 3 LSP files found vs 2 — smaller chunks = more distinct symbols |
| TC-10 | 1 | 3 | 0 | Nomic | Broader context captures overflow hint patterns across code + prose |
| TC-12 | 2 | 3 | 1 | Nomic | Larger chunks connect URL/prefix logic to backend resolution |
| TC-13 | 1 | 3 | 2 | Nomic | Multi-concept query needs broad context to link crash → recovery |
| TC-17 | 2 | 3 | **3** | Nomic/CRE tie | Companion plugin routing captured in larger doc chunks |
| TC-18 | 3 | 2 | **3** | Mini/CRE tie | Specific function-level query benefits from granular chunks |

### Key Takeaways

1. **nomic-embed-code still leads at 36/60.** AllMiniLML6V2Q (34) close behind with zero-config advantage.

2. **CodeRankEmbed trajectory: 23/60 (2026-04-03) → 22/60 (metadata-enriched, 2026-04-20) → 26/60 (post-93-commit reindex, 2026-04-21).** The +3 on the latest run is codebase growth (new docs pulled missing files into range), not a model improvement. Still last place at default chunk size. Ship gate ≥30/60 not yet met. Chunk size tuning (`chunk_size = 1024`) remains the most promising lever.

3. **CodeRankEmbed wins Tier 3 (8/15)** — best cross-cutting score across all models. Query prefix + code-tuned bias helps on multi-concept queries. If chunk size is fixed, Tier 2 could improve substantially.

4. **Each model wins a tier:**
   - nomic-embed-text wins Tier 1 (12/15) — 768 dims + 8K context sweet spot for direct lookups
   - nomic-embed-code wins Tier 2 (17/21) — 32K context excels at multi-concept composition
   - AllMiniLML6V2Q wins Tier 4 (5/9) — granular chunks find specific functions/patterns
   - CodeRankEmbed wins Tier 3 (8/15) — query prefix + code training helps cross-cutting

5. **All models fail TC-10, TC-19, TC-20** — internal pipeline queries where code doesn't use query vocabulary. Fundamental embedding limitation.

6. **Cost-effectiveness ranking (current benchmark):**
   - AllMiniLML6V2Q: 34/60, ~70s index, 71 MB — **best default** (bundled, zero-config)
   - nomic-embed-text: 32/60, ~60s index, 55 MB — needs Ollama, smallest storage
   - nomic-embed-code: 36/60, ~25min index, 372 MB — needs GPU, marginal improvement
   - CodeRankEmbed: 23/60, ~1min index, 173 MB — **needs chunk_size tuning; fast indexing is a plus**
## Notes

- **Chunk count varies by model** — models with larger context windows produce fewer, larger chunks.
  This affects retrieval: fewer chunks means each result covers more code, but may be less precise.
- **DB size scales with dimensions** — 3584-dim vectors use ~9x more storage than 384-dim.
- **Index time depends on hardware** — local ONNX models run on CPU; remote models depend on GPU/network.
- **Ground truth is subjective** — expected files are based on codescout's current architecture as of
  2026-04-03. Update them if the codebase changes significantly.
- **Run all 20 queries in the same session** to avoid MCP restart overhead between queries.
