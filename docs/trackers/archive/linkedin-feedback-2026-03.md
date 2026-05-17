---
id: null
kind: null
status: archived
title: null
owners: []
tags: []
topic: null
time_scope: null
---
# LinkedIn Feedback — March 2026

Actionable items from external feedback on codescout.

## Deferred

### Token efficiency benchmarks
Create first-party before/after measurements comparing context usage with
codescout tools vs native Read/Grep for equivalent tasks. We cite external
research (AgentDiet, SWE-Pruner, MCP-Zero) but have zero quantitative data
from codescout itself. Could instrument token counts from `usage.db` and
compare against native tool baselines.

### Semantic search scaling benchmarks
Document indexing time and search quality at scale. Current pipeline (4-way
concurrent embedding, sqlite-vec KNN) works for small-to-medium codebases but
has no benchmarks for large repos (100k+ files). Test on a few large
open-source repos, measure indexing time by backend (Ollama CPU/GPU, OpenAI),
and document results.

## Active

_(none)_

## Resolved

### Restore HTTP transport (rmcp 1.x migration)
Already working. Verified 2026-04-15 via smoke test: `codescout start
--transport http --port 39991 --auth-token X`, `curl POST /mcp initialize`
returns a full response with tools list. `http` is enabled in default features;
`rmcp/transport-streamable-http-server` is wired in `src/server.rs`. Tracker
entry was stale — the migration landed before this tracker was opened.

### Improve onboarding skip behavior
Implemented 2026-04-15 as `project_hints` field in the `activate_project`
response. New module `src/mcp_resources/project_hints.rs` probes manifest
files at the project root (Cargo.toml, package.json, pyproject.toml, go.mod,
pom.xml, build.gradle{,.kts}) and returns `primary_language`, `manifest`,
`entry_points`, `build_commands`, and `onboarded` flag. Agents that never call
the `onboarding` tool still get meaningful project context. Experimental doc:
`docs/manual/src/experimental/project-hints.md`.
