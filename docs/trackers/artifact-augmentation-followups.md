---
title: Artifact augmentation — followups & enhancement roadmap
status: in-progress
last_updated: 2026-05-01
owner: @mareurs
kind: tracker
related:
  - docs/superpowers/plans/2026-05-01-artifact-augmentation.md
  - docs/superpowers/specs/2026-05-01-artifact-augmentation.md
  - .worktrees/artifact-augmentation (branch feat/artifact-augmentation)
---

# Artifact augmentation — followups

Tracks enhancements to the `artifact_augmentation` feature shipped on `feat/artifact-augmentation` (15 commits, not yet merged to `experiments`).

The v1 feature added: `prompt + params` per artifact, RFC 7396 merge, gather sources (`git_log`, `artifacts`, `observations`, `file`, `grep`), refresh cycle (`artifact_refresh` → synthesize → `artifact_update` → `artifact_refresh_commit`), `kind: tracker` priority sort + `[LIVE]` header in `librarian_context`.

Followup work below comes from the 2026-05-01 cross-project tracker pattern research (codescout, MRV-poc, backend-kotlin) — see commit/research notes for the full matrix of manual-vs-AI tracker patterns.

### 2026-05-01 — v1 augmentation merged to experiments
Merge commit `3765e1b`: 18 files, +2092 lines. 267 lib tests passing.
## Phase status

| Phase | Title | Status | Notes |
|------:|-------|--------|-------|
|     0 | v1 augmentation feature | done | Merged `3765e1b` on 2026-05-01. |
|     1 | `render_template` + `params_schema` | code-complete | All code/tests green on `feat/augmentation-render-template`. T-8 (manual docs) deferred. |
|   1.5 | `tracker_design` teaching tool | code-complete | All tasks done. 5 new tests, 292 lib tests total (was 287). |
|     2 | `refresh_stale` discovery tool | open | Lists augmented artifacts where any gathered source is newer than `last_refreshed_at`. |
|     3 | `GatherSource::ConfigValue` | open | Read flag/setting from YAML/TOML/JSON/.conf + last-changed commit. Backend-kotlin flag-tracker use case. |
|     4 | `append_mode` + history cap | open | Refresh produces dated delta blocks instead of rewriting body. Session-log pattern (MRV-poc `retrieval-improvement.md`). |
|     5 | `kind: experiment` first-class | open | Promote `benchmarks/experiments/<dated>/` to artifacts; trackers fan in via existing `artifacts` source. |

Skipped for now (in brainstorm but deferred):
- `GatherSource::ProcessOutput` — security review needed for command exec
- Dirty-propagation hooks on `artifact_observe` — premature, prefer explicit `refresh_stale`

## Tasks

| ID | Task | Status | Phase | Notes |
|---:|------|--------|------:|-------|
| T-1 | Merge `feat/artifact-augmentation` → `experiments` | done | 0 | Merged 2026-05-01 as `3765e1b`. |
| T-2 | Add `render_template` column to `artifact_augmentation` (schema v4) | done | 1 | Migration v4 in `catalog::run_migrations` (idempotent). |
| T-3 | Add `params_schema` column (JSON Schema text) | done | 1 | Validates on both `artifact_augment` (initial) and `artifact_update_params` (merge). |
| T-4 | Render integrated into `librarian_context` (chose embed over separate tool) | done | 1 | `tools/render.rs` + injection between `[LIVE]` header and body. |
| T-5 | `librarian_context` uses rendered output for trackers w/ template | done | 1 | `render_template_projects_params_into_context` + error-surface test. |
| T-6 | Spike: pick template engine (`minijinja` vs `handlebars-rust`) | done | 1 | Picked `minijinja` per user. |
| T-7 | Tests: schema-validate-on-update, template-render, fall-through when absent | done | 1 | 287 lib tests (was 267); 20 new across `schema_validate`, `render`, `update_params`, `augment`, `context`, `catalog`. |
| T-8 | Docs: `docs/manual/src/experimental/artifact-augmentation.md` extends to v2 surface | done | 1 | Shipped as `augmentation-render-template.md`. |
| T-9 | `artifact_refresh_stale` tool design | done | 2 | Shipped as `crates/librarian-mcp/src/tools/refresh_stale.rs`. |
| T-10 | `GatherSource::ConfigValue` design (which formats? key path syntax?) | open | 3 |  |
| T-11 | `append_mode` design — where does the date come from? cap policy? | open | 4 |  |
| T-12 | `kind: experiment` schema + auto-discovery from filesystem | open | 5 |  |
| T-13 | New tool `tracker_design` (archetype-driven) | done | 1.5 | `tools/tracker_design.rs`. |
| T-14 | 6 archetypes: deployment_state, failure_table, metric_baseline, audit_issues, reflective, task_list | done | 1.5 | All 6 shipped. |
| T-15 | `system_prompt` teaching markdown (archetype selection, prompt-writing, schema discipline, anti-patterns) | done | 1.5 | ~3.5KB; covers 7 steps + anti-patterns. |
| T-16 | Inline existing-trackers list (cap 30, overflow hint) | done | 1.5 | `EXISTING_TRACKERS_CAP=30`, hint references `artifact_find`. |
| T-17 | Tests: response structure + each archetype's example params validates against its example schema | done | 1.5 | 5 tests including self-consistency (schema↔example) and template-renders-against-example. |
| T-18 | Register `tracker_design` in `tools/mod.rs::all_tools` + prompt-surface mention | done | 1.5 | Tool selection table row + augmentation-and-refresh section paragraph. |

Status legend: `open` / `in-progress` / `done` / `blocked` / `dropped`

## Phase 1 — `render_template` + `params_schema` (active)

### Why
Manual trackers mix two update cadences in one document:
- **State** — flag values, F-N pass/fail, eval baselines. Updates often (per commit/run).
- **Prose** — why we ran the experiment, what we learned, options being weighed. Updates rarely.

v1 augmentation conflates them: refresh rewrites the whole body. Repeated refreshes either (a) churn the prose unnecessarily or (b) drop the prose to keep the table fresh.

Decoupling: `params` carries state; `render_template` projects it to markdown; body holds prose only.

### Shape (draft)
```sql
ALTER TABLE artifact_augmentation
  ADD COLUMN render_template TEXT,
  ADD COLUMN params_schema   TEXT;
```

Render flow in `librarian_context` for a tracker with template:
1. Render `template(params) → table_md`
2. Emit body
3. Inject `<!-- [LIVE] -->` block + `table_md` between body and prompt blockquote

### Open questions
- Engine: MiniJinja (Jinja2-like, light) vs Handlebars (LLM-friendlier prompts)?
- Where does the rendered table appear — top of body, bottom, separate section?
- Does `artifact_update_params` validate against `params_schema`, or do we add a separate `artifact_validate_params`?
- Schema source: inline JSON Schema string, or reference to a registered schema?

### Acceptance
- [ ] Setting template+schema on a tracker artifact persists across `artifact_refresh_commit`
- [ ] `artifact_update_params` rejects merge-patches that violate schema
- [ ] `librarian_context` renders state table without LLM rewriting body
- [ ] Refresh cycle still works (refresh updates params; body untouched if template covers state)
- [ ] At least one example tracker in `docs/` ships with template+schema (dogfood)

## Phase 1.5 — `tracker_design` teaching tool (active)

### Why
User asks "create tracker for X" → agent currently has no canonical playbook for picking archetype, designing params, writing the augmentation prompt, or avoiding existing-tracker collisions. `server_instructions.md` is the wrong channel (loads every request, generic). MCP prompts are user-triggered (wrong actor). A pay-per-use tool is right — agent fetches teaching when needed.

### Shape
```
tracker_design(intent?: string) -> {
  system_prompt: "<markdown teaching>",
  archetypes: [...6 patterns...],
  existing_trackers: [{ id, title, kind, last_refreshed_at }],
  design_version: "1",
  next_step: "Compose spec, call tracker_create"
}
```

Archetype-driven (not intent-driven) for v1 — server stateless, transparent. Intent-driven synthesis deferred.

### Acceptance
- [ ] 6 archetypes covering cross-project tracker patterns from research
- [ ] Each archetype's `params_shape_example` validates against its `params_schema_example`
- [ ] `existing_trackers` capped at 30 with overflow hint
- [ ] `system_prompt` covers: archetype selection criteria, prompt-writing rules, params/schema discipline, anti-patterns
- [ ] Tool registered in `tools/mod.rs::all_tools`
- [ ] Mentioned in `server_instructions.md` tool-selection table

## History

### 2026-05-01 — initiative kicked off
- v1 feature complete on `feat/artifact-augmentation`
- Cross-project tracker research done (codescout / MRV-poc / backend-kotlin)
- Brainstorm produced 5-phase roadmap; this tracker created
- Phase 1 (`render_template` + `params_schema`) selected as next pull

### 2026-05-01 — Phase 1 started
- v1 merged to `experiments` (commit `3765e1b`)
- Worktree+branch `feat/augmentation-render-template` created
- Engine: `minijinja`
- Branch strategy: stack on experiments (not feat branch)

### 2026-05-01 — Phase 1 implementation complete
- Schema v4: idempotent migration adds `render_template` + `params_schema` columns; `column_exists` helper + idempotency test
- `tools/schema_validate.rs`: thin `jsonschema` wrapper, 7 unit tests
- `tools/render.rs`: MiniJinja `render_params` + 5 unit tests (substitution, for-loop, dict, error-surface, missing-var)
- `artifact_augment`: accepts `render_template` + `params_schema`; validates initial params before persist
- `artifact_update_params`: validates merged params against stored schema before commit; failure leaves params untouched
- `librarian_context`: renders template under `[LIVE]` header; render errors surface as `<!-- render_template error: ... -->` for self-correction
- `apply_merge_patch` extracted as public helper for pre-validation merge preview
- 287 lib tests pass (267 → 287, +20 new); fmt/clippy clean on librarian-mcp

### 2026-05-01 — Phase 1.5 implementation complete
- `tools/tracker_design.rs`: 6 archetypes (deployment_state, failure_table, metric_baseline, audit_issues, task_list, reflective)
- Each archetype carries `when_to_use`, `params_shape_example`, `params_schema_example`, `render_template_example`, `body_skeleton`, `prompt_template`
- `system_prompt`: 7-step teaching markdown (~3.5KB) covering archetype selection, prompt-writing, params/schema discipline, template tips, collision-checking, anti-patterns
- `existing_trackers` capped at 30 with overflow hint pointing at `artifact_find`
- `design_version: "1"` for future invalidation
- 5 new tests including self-consistency check (each archetype's example params validates against its own schema) and template-renders-against-example
- 292 lib tests pass (287 → 292); fmt/clippy clean on librarian-mcp
- Server instructions updated: tool selection row + paragraph in augmentation-and-refresh section directing agent to call `tracker_design` BEFORE `tracker_create`
