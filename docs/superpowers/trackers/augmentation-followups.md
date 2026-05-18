---
id: null
kind: tracker
status: draft
title: Artifact Augmentation — Implementation Followups
owners: []
tags:
- librarian-mcp
- augmentation
- tracker
topic: null
time_scope: null
---

# Artifact Augmentation — Implementation Followups

**Started:** 2026-05-01
**Status:** active — Phase 1 + 1.5 code-complete, Phase 2 open.

## Why this exists

Tracks implementation progress for the artifact augmentation system: the prompt + params + render_template layer that enables AI-maintained tracker artifacts in the librarian catalog.

References:
- Spec: `docs/superpowers/specs/2026-05-01-artifact-augmentation-design.md`
- Plan: `docs/superpowers/plans/2026-05-01-artifact-augmentation.md`

## Phase descriptions

### Phase 1 — render_template + params_schema
Core augmentation system: schema, CRUD, gather sources, all tools, and `[LIVE]` rendering in `librarian_context`. Extended with `render_template` (MiniJinja evaluation) and `params_schema` (JSON Schema validation on augment/update).

**Acceptance:** All augmentation tools registered, prompt surfaces updated, tests passing.

### Phase 1.5 — tracker_design teaching tool
Pre-creation teaching: 6 archetypes with schemas, render templates, body skeletons, and a 7-step system_prompt guiding the agent through tracker design before calling `tracker_create`.

**Acceptance:** Self-consistency tests prove each archetype's example validates against its own schema and renders against its own template.

### Phase 2 — artifact_refresh_stale tool
Discovery tool: surfaces augmented artifacts whose `last_refreshed_at` is stale (beyond a configurable threshold). Returns a ranked list so the agent knows what to refresh without scanning the full catalog.

**Open questions:**
- Staleness threshold: per-artifact config in params or global default?
- Sort order: oldest-first or priority-weighted?
- Scope: current project only, or workspace-wide?

**Acceptance:** Tool registered, prompt surface updated, tests cover empty/partial/full stale sets.

## History

### 2026-05-01 — Phase 1 + 1.5 land on feat/augmentation-render-template

Phase 1 (commits `fe29a17`): `render_template` field on augmentation, `params_schema` validation, MiniJinja evaluation in `librarian_context`.
Phase 1.5 (commit `47f011a`): `tracker_design` tool with 6 archetypes, self-consistency tests, server instructions updated.
