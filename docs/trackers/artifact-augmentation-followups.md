---
id: null
kind: tracker
status: active
title: Artifact augmentation - followups and enhancement roadmap
owners: []
tags: []
topic: null
time_scope: null
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
|     1 | `render_template` + `params_schema` | done | All 5 acceptance criteria met 2026-05-02. A2 commit `b1431a5`, A5 dogfood on artifact `79a6276776a1b5da`. |
|   1.5 | `tracker_design` teaching tool | done | All 6 acceptance criteria verified 2026-05-02. |
|     2 | `refresh_stale` discovery tool | done | Shipped as `refresh_stale.rs`; registered, 6 tests. Verified 2026-05-02. |
|     3 | `GatherSource::ConfigValue` | done | Implemented 2026-05-02 — commit `00e57f7`. TOML/YAML/JSON + git blame annotation. |
|     4 | `append_mode` + history cap | done | Implemented 2026-05-02 — commit `5a2797c`. 13 tests, trim_history, dated prepend, history cap. |
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
| T-9 | `artifact_refresh_stale` tool — shipped + verified | done | 2 | `crates/librarian-mcp/src/tools/refresh_stale.rs`, 6 tests. |
| T-10 | `GatherSource::ConfigValue` design (which formats? key path syntax?) | done | 3 | Detailed spec written 2026-05-02 — see Phase 3 section. |
| T-19 | `GatherSource::ConfigValue` implementation | done | 3 | Implemented 2026-05-02 — commit `00e57f7` on `experiments`. 7 tests, TOML/YAML/JSON + git blame annotation. |
| T-11 | `append_mode` design — where does the date come from? cap policy? | done | 4 | Detailed spec written 2026-05-02 — see Phase 4 section. |
| T-20 | `append_mode` + history cap implementation | done | 4 | Implemented 2026-05-02 — commit `5a2797c` on `experiments`. 13 tests: catalog roundtrip, augment tool, trim_history unit (4), update integration (4), refresh hint. |
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
- [x] Template+schema persists across `artifact_refresh_commit` — migration v4 in `catalog::run_migrations` confirmed (idempotent ALTER TABLE).
- [x] `artifact_update_params` rejects merge-patches violating schema — `merge_params()` validates against `params_schema` at catalog layer (commit `b1431a5`).
- [x] `librarian_context` renders state table — template rendered + injected in `context.rs`.
- [?] Refresh cycle — `artifact_refresh` returns params separately; body untouched. Params update still needs a separate `artifact_update` call.
- [x] At least one tracker in `docs/` ships with template+schema (dogfood) — augmentation-followups tracker (`79a6276776a1b5da`) augmented 2026-05-02; renders live phase table via `librarian_context`.

## Phase 1.5 — `tracker_design` teaching tool (DONE)

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
- [x] 6 archetypes covering cross-project tracker patterns from research
- [x] Each archetype's `params_shape_example` validates against its `params_schema_example`
- [x] `existing_trackers` capped at 30 with overflow hint
- [x] `system_prompt` covers: archetype selection criteria, prompt-writing rules, params/schema discipline, anti-patterns
- [x] Tool registered in `tools/mod.rs::all_tools`
- [x] Mentioned in `server_instructions.md` tool-selection table

## Phase 3 — `GatherSource::ConfigValue`

### Why

`GatherSource::File` gives you the whole file. `ConfigValue` gives you one key from a structured config file — plus the last commit that touched it. Use case: a Kotlin backend flag-tracker that always surfaces the current flag value and when it last changed, without the LLM grepping.

### Shape

New variant in `crates/librarian-mcp/src/tools/gather.rs`:

```rust
ConfigValue {
    path: String,        // relative to project root, e.g. "Cargo.toml"
    key: String,         // dotted key path, e.g. "package.version" or "flags.dark_mode"
},
```

Format auto-detected from extension: `.toml`, `.yaml`/`.yml`, `.json`. Other extensions → warning, skip.

Key path syntax: dotted segments (`a.b.c`). Array index by position: `dependencies.0.name`. Simple and human-readable; no JSON Pointer overhead.

Return value (under `source_key = "config_value"`):
```json
{
  "path": "Cargo.toml",
  "key": "package.version",
  "value": "0.8.1",
  "last_changed_commit": "abc1234",
  "last_changed_at": "2026-04-10T11:32:00Z"
}
```
`last_changed_commit` comes from `git log -1 --format="%H %aI" -- <path>`. If git unavailable or key missing → `null`.

### Implementation steps

1. Add `ConfigValue { path, key }` variant to `GatherSource` enum (`gather.rs:13`)
2. Add `gather_config_value(ctx, path, key) -> Result<Value>`:
   - Read file via project root path resolution (same as `gather_file`)
   - Parse with `toml::Value` / `serde_yaml::Value` / `serde_json::Value` by extension
   - Walk dotted key segments; return `RecoverableError` if key not found
   - Run `git log -1 --format="%H %aI" -- path` for last-changed commit (best-effort)
3. Add match arm in `gather_all` (line 62), source_key `"config_value"`
4. Tests: TOML key found, YAML key found, JSON key found, key not found → warning not error, unknown extension → warning

### Dependencies

- `toml` — already in `Cargo.toml` (used elsewhere)
- `serde_yaml` — check if present; add if needed
- No new MCP tool needed — this is a gather source variant only

### Acceptance

- [ ] `ConfigValue` variant deserialises from `{"source": "config_value", "path": "...", "key": "..."}`
- [ ] Value extracted correctly for TOML, YAML, JSON
- [ ] `last_changed_commit` populated when git available
- [ ] Missing key produces a warning (not a hard error) — gather continues
- [ ] Unknown extension produces a warning and skips
- [ ] Tests cover all 5 cases above

---

## Phase 4 — `append_mode` + history cap

### Why

Currently `artifact_refresh` returns a package → LLM rewrites the whole body → `artifact_update(body=...)`. For session logs, experiment journals, and incident timelines, rewriting destroys history. `append_mode` flips the write semantics: each refresh prepends a new dated section; old sections accumulate up to `history_cap`.

### Shape

Two new columns on `artifact_augmentation` (migration v5):

```sql
ALTER TABLE artifact_augmentation ADD COLUMN append_mode INTEGER NOT NULL DEFAULT 0;
ALTER TABLE artifact_augmentation ADD COLUMN history_cap  INTEGER;  -- NULL = unlimited
```

Set via `artifact_augment`:
```
artifact_augment(id, prompt, append_mode=true, history_cap=10)
```

Changed write flow when `append_mode = true`:
1. `artifact_refresh` — unchanged; returns same package. Prompt should instruct LLM to write only the new delta block.
2. `artifact_update(id, patch={body: "<delta>"}, commit_refresh=true)` — detects `append_mode` from augmentation row:
   - Reads current file body
   - Prepends `## <ISO date>\n\n<delta>\n\n` at top of body section
   - If `history_cap` set: counts `## YYYY-MM-DD` headers; drops oldest entries beyond cap
   - Writes file
3. Date: `chrono::Utc::now().format("%Y-%m-%d")` at write time in `artifact_update`

### Implementation steps

1. Migration v5: add `append_mode` + `history_cap` to `artifact_augmentation`; update `AugmentationRow` struct
2. `artifact_augment` tool: add `append_mode: Option<bool>` + `history_cap: Option<usize>` to input schema; persist to row
3. `artifact_update/call`: after resolving body write path, check augmentation row for `append_mode`:
   - If true: load current body, prepend dated block, apply history cap trim, write
   - If false (default): existing replace behaviour unchanged
4. Helper `trim_history(body: &str, cap: usize) -> String`: scan for `## \d{4}-\d{2}-\d{2}` headers, keep first `cap`, drop rest
5. `artifact_refresh` response: when `append_mode=true`, add `"append_mode": true` hint so LLM knows to write a delta not a full body rewrite

### Key decision: cap trim strategy

Keep the **N most recent** sections (top of file = newest). Trim by scanning for dated `##` headers top-to-bottom; drop everything from entry N+1 onward. Prose above the first dated header (intro paragraph etc.) is preserved.

### Tests

- Append prepends new dated section at top
- Second append produces two sections in reverse-chronological order
- `history_cap=2` with 3 appends keeps only 2 newest
- Intro prose above first dated header is not dropped by trim
- `append_mode=false` (default): existing replace behaviour unaffected
- `artifact_refresh` hints `append_mode: true` when set

### Acceptance

- [ ] `append_mode` + `history_cap` persist across `artifact_augment` → `artifact_refresh` → `artifact_update`
- [ ] `artifact_update` prepends dated block when `append_mode=true`
- [ ] `history_cap` trims oldest entries correctly
- [ ] Intro prose preserved through trim
- [ ] `artifact_refresh` response includes `append_mode` hint
- [ ] Default (`append_mode=false`) behaviour unchanged — no regression
- [ ] All 6 test cases above pass

---

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
