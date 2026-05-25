# Agent-Memory Frameworks — Research & codescout Integration Analysis

Research done 2026-05-25, in the context of codescout's own memory surfaces. Exploratory only — no spec/plan committed.

## TencentDB Agent Memory (Tencent, MIT, released ~2026-05-23, 4.1k★ in days)
Local-first (SQLite + sqlite-vec, zero external API by default; optional TCVDB cloud). Two subsystems:
- **Symbolic short-term memory**: offloads bulky tool logs to `refs/*.md`, injects only a compact Mermaid state-graph (~hundreds of tokens); each node has a `node_id` for grep recovery. ≈ codescout's `@ref` buffers, but persisted to disk.
- **4-tier long-term pyramid**: L0 Conversation → L1 Atom → L2 Scenario → L3 Persona, LLM-distilled on a heuristic schedule (every-N-turns, default 5; persona regen every 50). Hybrid BM25+vector retrieval via RRF, deterministic drill-down (result_ref/node_id). White-box markdown top layer under `~/.openclaw/memory-tdai/`.
- Benchmarks are AGENTIC, not the standard memory benchmarks: WideSearch 33→50% (−61% tokens), SWE-bench 58.4→64.2% (−33% tokens), PersonaMem 48→76%. NOT comparable to Mem0/Letta/Zep numbers on LoCoMo/LongMemEval.

## Landscape (the axis matters more than the brand)
Mem0 (vector+graph+KV, drop-in framework-agnostic SDK, self-editing dedup) · Letta/MemGPT (OS-tiered core/recall/archival, agent self-manages movement) · Zep/Graphiti (temporal knowledge graph, fact validity windows; 63.8% vs Mem0 49% on LongMemEval) · Cognee (local poly-store, swappable graph backends) · LangMem (procedural / self-editing system prompt; coupled to LangGraph) · Supermemory (MCP-native, explicit forgetting/expiry, targets coding agents) · MS Kernel Memory (Azure, RAG-centric) · Redis Agent Memory (infra layer, not a framework).

## Survey taxonomy — arXiv 2603.07670 "Memory for Autonomous LLM Agents: Mechanisms, Evaluation, and Emerging Frontiers"
Three ORTHOGONAL axes (blog comparisons conflate them):
1. **Temporal scope**: working / episodic / semantic / procedural
2. **Substrate**: context-text / vector / structured (SQL/KV/graph) / executable / hybrid
3. **Control policy**: heuristic / prompted-self-control / learned(RL)
Mechanism families: context-compression · retrieval-augmented · reflective/self-improving · hierarchical/virtual-context · policy-learned · parametric. 4-layer metric stack: task-effectiveness, memory-quality (incl. contradiction rate, staleness), efficiency, governance. Mantra: **"long context is not memory."** Standard benchmarks (LoCoMo/LongMemEval) are shallow — 85–94% of questions need evidence from only 2 sessions.

## KEY CONCLUSION for codescout (the load-bearing finding)
codescout already spans ALL 3 substrate types:
- atoms ≈ `ObservationRow` (src/librarian/catalog/observations.rs:8)
- working tier ≈ `OutputBuffer`/`@ref` (src/tools/output_buffer.rs)
- vector semantic store: Qdrant with buckets (code/system/preferences/unstructured) + `create_semantic_anchors` (src/tools/memory/mod.rs); `QdrantWrap` in src/retrieval/memory.rs
- structured graph: librarian `Catalog` (ArtifactRow/EdgeRow/EventRow/LinkRow)
- white-box markdown everywhere

The ONLY real gap is **Axis 3 — control policy**: every write is hand-authored (prompted self-control); no automated consolidation (family #1) or reflection (#3).

**Critical invariant:** codescout is a PASSIVE EMBEDDER — `Embedder`/`RemoteEmbedder::openai` are EMBEDDINGS-only; there is NO generative/chat client anywhere in the tree. TencentDB's distillation REQUIRES a generative LLM. So the real design fork is: *does codescout cross from passive tool-provider to active LLM-invoking agent?*

## Integration approaches (pros/cons brainstormed; Approach C deepened)
- **A — adopt external framework (Mem0/Supermemory) as backend**: rejected — violates local-first/single-binary/agent-agnostic ethos; two overlapping vector stores; white-box loss.
- **B — build TencentDB-style native distillation pipeline**: elegant substrate fit (L0→L3 maps ~1:1 onto librarian rows) BUT requires adding a generative LLM in-server = identity break; whose model/tokens?; silent-fact-loss + non-determinism vs strict test culture.
- **C — host-driven scaffolding (RECOMMENDED, ethos-preserving)**: keep codescout passive; the HOST LLM does generation via new tools. Three additive, deterministic, independently-shippable pieces:
  1. `memory consolidate` action — host proposes atomic facts, codescout dedups (existing embedding path) + stores as `ObservationRow` (no new fact schema).
  2. Persist `@ref` buffers across sessions — spill `BufferEntry` to `.codescout/buffers/<output_id>.md`, rehydrate on startup; `node_id` recovery = existing `grep @ref` made durable. Precedent: `BufferEntry.source_path` already round-trips `@file_*`.
  3. Add `valid_until`/`superseded_by` to `ArtifactRow` → Zep-style temporal + Supermemory-style forgetting, reusing `HIDDEN_STATUSES` filter.
  Conclusion: this isn't "add a memory system" — it's adding the missing connective tissue between surfaces codescout already has, leaving the generative trigger in the host's hands.

## Side effect
Surfaced the `HIDDEN_STATUSES` split-brain bug → docs/issues/2026-05-25-hidden-statuses-context-missing-retired.md (find.rs has "retired", context.rs doesn't).

## Sources
- github.com/Tencent/TencentDB-Agent-Memory
- arXiv 2603.07670 (survey)
- mem0.ai/blog/state-of-ai-agent-memory-2026; atlan.com/know/best-ai-agent-memory-frameworks-2026
