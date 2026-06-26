---
kind: tracker
status: active
title: Headroom cross-pollination — codescout improvement candidates
owners: []
tags:
  - headroom
  - cross-pollination
  - token-efficiency
  - output-buffers
  - compression
created: 2026-06-09
---

# Headroom cross-pollination — codescout improvement candidates

A backlog of codescout improvement ideas surfaced by studying the sibling
**headroom** project (`/home/marius/work/claude/headroom`, branch `my_fixes`;
PyPI `headroom-ai` v0.24.0, github.com/chopratejas/headroom). Headroom is a
local-first **LLM context-optimization layer** — it compresses prompts /
tool-outputs / RAG / logs 50–90% before they reach a provider, exposed as a
library, a drop-in proxy, an agent `wrap`, and an MCP server. Its headline
benchmark — *"Code search, 100 results: 17,765 → 1,408 tokens (92%)"* — is
literally codescout's output domain.

> **Provenance:** 2026-06-09 session. Every codescout-side shape claim below was
> scouted against current code in reconnaissance **R-19**
> (`docs/trackers/reconnaissance-patterns.md`) — file:line refs are verified,
> not recalled. The original analysis overstated candidate **C-2** ("generic
> summarization"); R-19 corrected it and the corrected framing is what appears
> here.

## The core relationship: same problem, opposite ends

codescout compresses **at the source** — it generates the output, knows the
semantics (symbols, hits, file maps), and returns compact-by-default with
progressive disclosure. Headroom compresses **on the wire** — content-agnostic,
sitting between any agent and any provider. Source-side is smarter (understands
structure); wire-side is universal (catches uninstrumented tools too). They are
complementary **layers**, not competitors — which is why the cleanest
integration is composition, not wrapping (see Non-goals).

### Architectural mirror — Headroom CCR ≈ codescout `@ref` buffers

Independently-invented instances of the same pattern: *lossy on the wire,
lossless end-to-end.*

| | Headroom **CCR** | codescout **`@ref` buffer** |
|---|---|---|
| Trigger | compressed block is strictly smaller | result exceeds `MAX_INLINE_TOKENS` (2,500) |
| Stash key | BLAKE3 hash (24 hex) — **content-addressed** | `@tool_{now+counter:08x}` — **per-call handle** |
| Wire marker | `<<ccr:HASH>>` appended to block | `{output_id, summary, hint}` envelope |
| Retrieval | `headroom_retrieve(hash=…)` | `read_file("@tool_xyz", json_path=… / start_line=…)` |
| Strength the other lacks | dedup (identical bytes → one entry) | structured slicing (grep / json_path / line range) |

Headroom evidence: `crates/headroom-core/src/transforms/live_zone.rs`
(`compress_anthropic_live_zone_with_ccr`, `maybe_inject_ccr_marker` ~L1239).
codescout evidence: `src/tools/output_buffer.rs:250`.

## Improvement candidates

### C-1 — Content-addressed dedup for `@ref` buffers
- **Verified shape:** handles are minted as
  `format!("@tool_{:08x}", now.wrapping_add(inner.counter) as u32)` at
  `src/tools/output_buffer.rs:250-251` — time + monotonic counter, **not**
  content-addressed. Identical output across calls mints a fresh handle every
  time; no dedup.
- **Idea:** key (or secondary-index) buffer entries by content hash so repeated
  identical results (e.g. re-running `symbols` on an unchanged file) collapse to
  one stored entry, and the model can recognize "I've already seen this exact
  output."
- **Cheaper than it looks:** a SHA-256 content-hash primitive already exists in-tree (not BLAKE3 — BLAKE3 is Headroom's CCR choice; any collision-resistant hash serves equally as a dedup key) —
  `content_hash(text)` at `src/retrieval/sync.rs:34` (used for embedding dedup).
  It would need wiring into `OutputBuffer::store_tool` / `store`, not authoring
  from scratch.
- **Priority:** high — strongest of the four; primitive present, shape confirmed.
- **Status:** candidate (not started). Shape re-verified 2026-06-21 against current code (R-19 datapoint 3): `@tool_*` minting still time+counter at `output_buffer.rs:251`; `content_hash` still SHA-256 at `sync.rs:34` (line cites refreshed from the stale `:29`).

### C-2 — Error-keyword / path preservation in overflow truncation
- **Verified shape (corrected by R-19):** codescout's compact summary is
  **per-tool, not generic** — `Tool::format_compact(&self, result) -> Option<String>`
  at `src/tools/core/types.rs:435`, each tool overrides it (`None` → the generic
  "Result stored in @tool_xxx" fallback). `run_command` already prioritizes
  stderr (`src/tools/run_command/tests.rs:2034`).
- **Actual gap:** there is **no content-level error-keyword preservation
  primitive** (no analogue of Headroom's `MCPToolProfile.preserve_error_keywords`
  / `always_keep_fields`), and the per-tool summaries are hand-written rather
  than profile-driven.
- **Idea:** add a preservation pass to the truncation that feeds `format_compact`
  — "when shortening, never drop lines matching error keywords; never drop the
  path column." NOT "make summarization tool-aware" (it already is).
- **Headroom reference:** `headroom/integrations/mcp/server.py` —
  `MCPToolProfile` (L79), `DEFAULT_MCP_PROFILES` (L99–137; per-family `max_items`,
  `preserve_error_keywords`), `compress_tool_result()` (L352).
- **Priority:** med — smaller surface than C-1; genuine but narrower than first stated.
- **Status:** candidate (not started). Shape re-verified 2026-06-21 against current code (R-19 datapoint 3): `format_compact` still a per-tool trait hook at `types.rs:435` (line cite refreshed from the stale `:387`).

### C-3 — CacheAligner-style prefix stabilization (conceptual)
- **Observation:** Headroom's **CacheAligner** ("stabilize prefixes so provider
  KV caches hit") + its `frozen_message_count` / live-zone concept is the
  *automated* form of codescout's *manual* prompt-cache discipline: the 2,200-byte
  cap on the `server_instructions` slice, the `source_md_under_cap` gate, the
  "inject once per session" rule.
- **Idea:** if codescout ever wants to harden instruction-prefix stability beyond
  a static byte cap, read `live_zone.rs`'s frozen-zone logic for the pattern.
- **Priority:** low — conceptual; no concrete defect, codescout's gates already work.
- **Status:** reference only

### C-4 — SmartCrusher statistical JSON layout for `detail_level:"full"` (conceptual)
- **Observation:** codescout returns arrays-of-dicts (symbol hits, search
  results) — SmartCrusher's exact target. Compact mode is already lean; the
  residual full-detail pulls could shrink with a column/schema-factored
  representation instead of row-of-dicts.
- **Priority:** low — only touches `detail_level:"full"` output; needs measurement.
- **Status:** reference only

## Non-goals / caveats

- **Do NOT wrap codescout's MCP server behind Headroom's proxy.** codescout
  already compresses at the source and already does CCR-style retrieval —
  double-compression yields little, AND risks **marker collision**: Headroom
  appending `<<ccr:HASH>>` to a codescout summary that already says "query
  `@tool_xyz`" puts two retrieval idioms in one response and confuses the model.
- **Complementary composition is the clean win:** in a real session, let
  codescout own the *code* surface (smarter there) and let Headroom compress the
  *uninstrumented* surfaces — raw Bash, native file reads, third-party MCP servers
  codescout doesn't touch.
- **Confidence:** reasoning from headroom's memories/docs + spot-checks of
  `live_zone.rs` (CCR) and `integrations/mcp/server.py` (the profile seam). No
  benchmark of codescout output *through* Headroom was run — "residual gain on
  full-detail pulls" is a grounded hypothesis, not a measurement.

## Links

- Reconnaissance: `docs/trackers/reconnaissance-patterns.md` → **R-19** (shape scout + the C-2 correction)
- codescout progressive-disclosure model: `get_guide("progressive-disclosure")`, `docs/PROGRESSIVE_DISCOVERABILITY.md`
- Headroom repo: `/home/marius/work/claude/headroom` (branch `my_fixes`)
