---
id: '437c392eefa936fc'
kind: tracker
status: done
title: Agent Memory & Timeline Research — April 2026
owners: []
tags:
- librarian-mcp
- research
- agent-memory
- timeline
- versioning
topic: null
time_scope: null
---


## Purpose

Research collected 2026-04-28 while designing the Artifact Timeline spec
(`docs/superpowers/specs/2026-04-28-librarian-timeline-design.md`).
Tracks findings on agent memory frameworks, temporal state patterns, and
additions to consider for librarian-mcp.

Related spec: `docs/superpowers/specs/2026-04-28-librarian-timeline-design.md`

---

## Q1: What is "TimeVault"?

No GitHub project, crate, npm package, or paper with that name exists.
It is an informal label for a versioned, append-only, time-indexed memory
store for agents — the "Git for AI memory" concept in community discussions.
Not a canonical project.

Closest real thing: **Temporal** (temporal.io) — replayable event history
for agent workflows — but that is workflow orchestration, not memory.

---

## Q2: Agent memory projects with temporal state

| Project | Stars | Temporal queries? | Git-anchored? |
|---|---|---|---|
| **Zep / Graphiti** (`getzep/graphiti`) | >2k | Yes — validity-windowed edges, episode replay | No — wall-clock |
| **MemGPT / Letta** | >13k | No — tiered memory, no time-travel | No |
| **Mem0** | active | No — silent async overwrite | No |
| **LangMem** | LangChain ecosystem | No — extraction + summarize | No |
| **ReMe** (`agentscope-ai/ReMe`) | — | No — JSONL compaction | No |
| **AgentMemory** (`rohitg00/agentmemory`) | — | No — BM25+vector+graph | No |
| **GCC** (arXiv 2508.00031, 2025) | prototype | Yes — COMMIT/BRANCH/MERGE primitives | Yes (prototype only) |

**Most relevant:** Zep/Graphiti (arXiv 2501.13956). Architecture closest to
the librarian timeline model.

---

## Q3: Git-anchored vs. wall-clock anchoring

Wall-clock dominates in all production systems. No production OSS library
anchors memory state to external git SHAs.

The GCC paper (arXiv 2508.00031) is the sole academic proposal treating
agent context like a git repo — 80.2% SWE-Bench Verified — but is a
preprint prototype, not a released library.

**The librarian `commits` table + `anchor_commit` SHA fills a real gap.**

---

## Q4: Freshness / staleness patterns

- **Graphiti:** validity windows on edges (`valid_from`/`valid_to`). Superseded = `valid_to` set.
- **Kafka-style compaction:** tombstone (null payload) = deleted. Freshness implicit in compacted head.
- **Mem0 / LangMem:** silent async overwrite, no explicit stale/fresh enum.

None use a discrete `{fresh, unknown, stale, superseded}` enum derived from
topological distance from HEAD. That is the spec's novel contribution.

---

## Q5: SQLite event log + state_at replay patterns

- **Dual-contract pattern (well-validated):** `events` table append-only +
  `snapshots` projection. `state_at(T)` = load nearest snapshot + replay
  delta. Directly maps to the librarian schema.
- **CQRS with SQLite:** write side appends to `events`; read side maintains
  a projection table rebuilt on demand or via triggers.
- **Graphiti episode model:** each episode is immutable; graph projections
  derived. The `reviewed` event maps cleanly to a Graphiti episode updating
  a validity window.

---

## Takeaways / Additions for librarian timeline

### ✅ Schema validated
`events` + `commits` + `sources` + `event_edges` maps to canonical SQLite
event-sourcing dual-contract. No structural changes needed.

### 🔲 Consider validity windows on events
When a newer `reviewed` event arrives, set `closed_at = new_commit_sha` on
the prior open event (Graphiti pattern). Enables:
`"freshness at SHA X" = event WHERE opened_at <= X AND closed_at > X (or NULL)`

Currently the spec derives freshness from `topo_distance(HEAD, newest_reviewed.head_commit)`.
Adding `closed_at` would make individual events self-contained for range queries
without replaying the full chain.

**Trade-off:** adds a mutation to an otherwise append-only log. Could be a
separate `event_windows` projection instead.

### ✅ Keep the freshness enum as-is
`{fresh, unknown, stale, superseded}` derived from topo-distance has no
precedent in surveyed systems. It is the spec's strongest differentiator.
Do not collapse to a timestamp comparison.

### 🔲 Compaction safety for state_at
If event compaction is ever added: write full snapshots at boundaries, or
mark events with a `compaction_safe` flag only when a snapshot exists at
that commit. Kafka tombstone pattern covers artifact deletion in timeline.

---

## Recall.it — researched 2026-04-28

**Verdict: not applicable to either project.**

- Personal AI knowledge base (cloud-only, 500k+ users) that saves, summarizes,
  and connects web pages, YouTube transcripts, PDFs, and notes into a searchable
  knowledge graph with spaced-repetition review. Target: knowledge workers, not
  developers building infra.
- Has an MCP server (`https://backend.getrecall.ai/mcp/`, OAuth, **read-only**).
  Four tools: `search`, `filter_by_metadata`, `get_document_content`, `explore_kb`.
  Write support listed as "coming soon."
- **No temporal queries.** No versioning, no event log, no `state_at(T)`. Date-range
  metadata filtering only.
- **Cloud-only, proprietary.** No self-hosted option. Fair Use Policy explicitly
  prohibits automated/bot usage — rules out MCP agent integration.
- No code awareness (no AST, no symbol graph, no diff/commit data).

**For codescout:** no overlap in architecture, data model, or problem space.

**For librarian-mcp timeline:** the core feature (git-commit-anchored event log,
`state_at(commit)`, freshness derivation) has no analogue in recall.it. Not useful
as a storage backend or design reference.

---

## Sources

- arXiv 2501.13956 — "Zep: A Temporal Knowledge Graph Architecture for Agent Memory"
- arXiv 2508.00031 — GCC (Git Context Controller) paper
- `getzep/graphiti` GitHub repo
- sqliteforum.com CQRS / event-sourcing guide
- temporal.io agent demo (`temporal-community/temporal-ai-agent`)
- docs.recall.it — MCP, public API, FAQ, Fair Use Policy

