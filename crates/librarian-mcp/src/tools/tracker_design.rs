//! `tracker_design` — teaching tool. Returns a system_prompt + a library of
//! archetypes + the existing-tracker landscape so the agent can compose a
//! well-shaped tracker spec and call `artifact_create` with confidence.
//!
//! Archetype-driven (not intent-driven) for v1: server stays stateless, no
//! synthesis cost, transparent to the agent. Intent-driven tailoring is
//! deferred until archetype selection proves frustrating in practice.

use crate::catalog::{artifact, augmentation};
use crate::tools::{Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct TrackerDesign;

#[derive(Deserialize, Default)]
struct Args {
    /// Free-form intent ("tracker for v7.1 flag rollout"). Currently
    /// echoed back; reserved for future intent-driven tailoring.
    #[serde(default)]
    intent: Option<String>,
}

const DESIGN_VERSION: &str = "1";

/// Cap for inline existing-trackers list. Above this, agent should call
/// `artifact_find {kind:"tracker"}` directly.
const EXISTING_TRACKERS_CAP: usize = 30;

#[async_trait]
impl Tool for TrackerDesign {
    fn name(&self) -> &'static str {
        "tracker_design"
    }

    fn description(&self) -> &'static str {
        "Returns a teaching system_prompt + archetype library + existing-tracker \
         landscape to guide the agent through composing a tracker. Call this \
         BEFORE artifact_create when the user asks to create a tracker — pick \
         an archetype, fill in the spec, then call artifact_create."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "intent": {
                    "type": "string",
                    "description": "Free-form intent (optional, reserved for future tailoring)"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args).unwrap_or_default();
        let cat = ctx.catalog.lock();

        // Existing-trackers landscape. Limit + overflow hint.
        let tracker_ids = augmentation::list_all_ids(&cat)?;
        let mut existing: Vec<Value> = Vec::new();
        let mut total_trackers = 0usize;
        for id in tracker_ids.iter() {
            let Some(art) = artifact::get(&cat, id)? else {
                continue;
            };
            if art.kind != "tracker" {
                continue;
            }
            total_trackers += 1;
            if existing.len() >= EXISTING_TRACKERS_CAP {
                continue;
            }
            let aug = augmentation::get(&cat, id)?;
            existing.push(json!({
                "id": id,
                "title": art.title,
                "kind": art.kind,
                "rel_path": art.rel_path,
                "last_refreshed_at": aug.as_ref().and_then(|a| a.last_refreshed_at.clone()),
                "refresh_count": aug.as_ref().map(|a| a.refresh_count).unwrap_or(0),
            }));
        }

        let mut response = json!({
            "design_version": DESIGN_VERSION,
            "system_prompt": SYSTEM_PROMPT,
            "archetypes": archetypes(),
            "existing_trackers": existing,
            "existing_trackers_total": total_trackers,
            "intent": a.intent,
            "next_step": "Pick archetype. Compose spec (prompt, params, render_template, params_schema, body). Call artifact_create with kind=tracker, status=active, and augment={prompt,params}.",
        });

        if total_trackers > EXISTING_TRACKERS_CAP {
            response["existing_trackers_overflow_hint"] = json!(format!(
                "Showing {EXISTING_TRACKERS_CAP} of {total_trackers}. For full list use artifact_find {{\"kind\":\"tracker\"}}."
            ));
        }

        Ok(response)
    }
}

fn archetypes() -> Value {
    json!([
        archetype_deployment_state(),
        archetype_failure_table(),
        archetype_metric_baseline(),
        archetype_audit_issues(),
        archetype_task_list(),
        archetype_reflective(),
    ])
}

fn archetype_deployment_state() -> Value {
    json!({
        "name": "deployment_state",
        "when_to_use": "Tracking the current state of a feature flag, env rollout, or config value across environments. State changes per commit/deploy. Examples: 'v7.1 flag pending server restart', 'recipe rollout post-fix audit'.",
        "params_shape_example": {
            "flag_name": "intent_classifier_v71",
            "envs": {
                "dev":  { "enabled": true,  "since": "2026-04-12" },
                "stage":{ "enabled": true,  "since": "2026-04-15" },
                "prod": { "enabled": false, "since": null }
            },
            "last_changed_commit": "abc1234"
        },
        "params_schema_example": {
            "type": "object",
            "required": ["flag_name", "envs"],
            "properties": {
                "flag_name": { "type": "string" },
                "envs": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "required": ["enabled"],
                        "properties": {
                            "enabled": { "type": "boolean" },
                            "since": { "type": ["string", "null"] }
                        }
                    }
                },
                "last_changed_commit": { "type": ["string", "null"] }
            }
        },
        "render_template_example": "**Flag:** `{{ flag_name }}`  \n\n| env | enabled | since |\n|-----|:-------:|-------|\n{% for env, s in envs|items %}| {{ env }} | {{ \"✅\" if s.enabled else \"❌\" }} | {{ s.since or \"—\" }} |\n{% endfor %}\n_Last changed: {{ last_changed_commit or \"—\" }}_",
        "body_skeleton": "## Why this flag exists\n\n_Brief: what the flag controls, who owns it._\n\n## Rollout plan\n\n_Steps and gates._\n\n## History\n\n_Append dated session blocks: ### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the live state of feature flag `<NAME>` across environments. Pull current values from settings files via `gather_from: file`. When envs disagree with the deployed commit (`gather_from: git_log`), prefer the most recent commit-derived value and note the divergence in body history. Keep params strictly mechanical — narrative belongs in body."
    })
}

fn archetype_failure_table() -> Value {
    json!({
        "name": "failure_table",
        "when_to_use": "Numbered failure list (F-1..F-N) from a test/eval suite. Status flips often as fixes land. Examples: 'eval test suite tracker', 'chat-runtime quality audit'.",
        "params_shape_example": {
            "failures": [
                { "id": "F-1", "status": "fail", "owner": "@mareurs", "last_seen": "2026-04-29", "notes": "regression after temporal fix" },
                { "id": "F-2", "status": "pass", "owner": "@mareurs", "last_seen": "2026-04-30", "notes": "fixed in abc123" }
            ],
            "suite": "chat-eval-v3"
        },
        "params_schema_example": {
            "type": "object",
            "required": ["failures"],
            "properties": {
                "suite": { "type": "string" },
                "failures": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "status"],
                        "properties": {
                            "id":        { "type": "string", "pattern": "^F-\\d+$" },
                            "status":    { "type": "string", "enum": ["fail", "pass", "flaky", "wontfix"] },
                            "owner":     { "type": "string" },
                            "last_seen": { "type": "string" },
                            "notes":     { "type": "string" }
                        }
                    }
                }
            }
        },
        "render_template_example": "**Suite:** `{{ suite }}` — {{ failures|selectattr(\"status\",\"equalto\",\"fail\")|list|length }} failing / {{ failures|length }} total\n\n| id | status | owner | last seen | notes |\n|----|--------|-------|-----------|-------|\n{% for f in failures %}| {{ f.id }} | {{ f.status }} | {{ f.owner or \"—\" }} | {{ f.last_seen or \"—\" }} | {{ f.notes or \"\" }} |\n{% endfor %}",
        "body_skeleton": "## Suite methodology\n\n_What the suite tests, how it runs, where results live._\n\n## Per-failure detail\n\n_Optional deeper notes per F-N when warranted._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the F-N failure list. After each suite run (gather_from: file pointing at the latest junit/json report), update each failure's status, last_seen, and notes. Add new F-N entries for new failures (next free integer). Never delete an entry — mark fixed entries as pass with a notes line citing the commit. Body holds methodology and per-failure deep dives."
    })
}

fn archetype_metric_baseline() -> Value {
    json!({
        "name": "metric_baseline",
        "when_to_use": "Living benchmark log with a baseline + dated session deltas. Examples: 'retrieval-improvement', 'eval-implementation T0/T1 deltas'.",
        "params_shape_example": {
            "baseline": { "P@5": 0.145, "R@5": 0.724, "captured": "2026-04-24" },
            "current":  { "P@5": 0.193, "R@5": 0.781, "captured": "2026-05-01" },
            "sessions": [
                { "date": "2026-04-29", "label": "T-A prose-lane", "deltas": { "P@5": "+0.017" } },
                { "date": "2026-04-30", "label": "T-B docx-driven", "deltas": { "P@5": "+0.031" } }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["baseline", "current"],
            "properties": {
                "baseline": { "type": "object", "additionalProperties": true },
                "current":  { "type": "object", "additionalProperties": true },
                "sessions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["date", "label"],
                        "properties": {
                            "date":   { "type": "string" },
                            "label":  { "type": "string" },
                            "deltas": { "type": "object", "additionalProperties": true }
                        }
                    }
                }
            }
        },
        "render_template_example": "**Baseline** ({{ baseline.captured }}): {% for k, v in baseline|items %}{% if k != 'captured' %}{{ k }}={{ v }} {% endif %}{% endfor %}\n**Current**  ({{ current.captured }}):  {% for k, v in current|items %}{% if k != 'captured' %}{{ k }}={{ v }} {% endif %}{% endfor %}\n\n## Sessions\n{% for s in sessions %}- **{{ s.date }} — {{ s.label }}**: {% for k, v in s.deltas|items %}{{ k }}={{ v }} {% endfor %}\n{% endfor %}",
        "body_skeleton": "## What we're measuring\n\n_Metrics, dataset, harness._\n\n## Method log\n\n_Append per-session writeups: why we ran this trial, what we changed, what we learned._\n\n### YYYY-MM-DD — <session label>",
        "prompt_template": "Maintain baseline + current metrics + per-session deltas. After each benchmark run (gather_from: file pointing at metrics JSON), update `current` and append a session entry. Don't move `baseline` unless an explicit re-baselining is decided in body. Narrative (why we ran this, what we learned) lives in body, not params."
    })
}

fn archetype_audit_issues() -> Value {
    json!({
        "name": "audit_issues",
        "when_to_use": "Numbered audit output: issue table with severity, status, owner. Examples: 'chunking-pipeline audit', 'production-trace audit'.",
        "params_shape_example": {
            "issues": [
                { "n": 1, "title": "Long PDFs split mid-sentence", "severity": "high", "status": "fixed", "owner": "@mareurs" },
                { "n": 2, "title": "Headers lost in xlsx",        "severity": "med",  "status": "open",  "owner": "@mareurs" }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["issues"],
            "properties": {
                "issues": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["n", "title", "severity", "status"],
                        "properties": {
                            "n":        { "type": "integer", "minimum": 1 },
                            "title":    { "type": "string" },
                            "severity": { "type": "string", "enum": ["high", "med", "low"] },
                            "status":   { "type": "string", "enum": ["open", "in-progress", "fixed", "wontfix"] },
                            "owner":    { "type": "string" }
                        }
                    }
                }
            }
        },
        "render_template_example": "| # | Issue | Severity | Status | Owner |\n|--:|-------|:--------:|:------:|-------|\n{% for i in issues %}| {{ i.n }} | {{ i.title }} | {{ i.severity }} | {{ i.status }} | {{ i.owner or \"—\" }} |\n{% endfor %}",
        "body_skeleton": "## Audit scope and methodology\n\n_What was audited, when, by whom._\n\n## Per-issue detail\n\n_For each issue: Symptom / Root cause / Fix / Predicted impact._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the numbered issue table. Status flips drive updates: as issues are fixed, mark `fixed` with a body note. Don't renumber. New issues get the next integer. Body has per-issue Symptom/RootCause/Fix sections — update those when status changes."
    })
}

fn archetype_task_list() -> Value {
    json!({
        "name": "task_list",
        "when_to_use": "Followup queue or phase-based task list with done/in-progress/open status. Examples: this very tracker (`artifact-augmentation-followups`), 'knowledge-injection-future-improvements'.",
        "params_shape_example": {
            "phases": [
                { "n": 1, "title": "render_template + params_schema", "status": "code-complete" },
                { "n": 2, "title": "refresh_stale tool",              "status": "open" }
            ],
            "tasks": [
                { "id": "T-1", "task": "Merge feat branch",      "status": "done", "phase": 0 },
                { "id": "T-2", "task": "Schema v4 columns",      "status": "done", "phase": 1 },
                { "id": "T-9", "task": "refresh_stale design",   "status": "open", "phase": 2 }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["tasks"],
            "properties": {
                "phases": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["n", "title", "status"],
                        "properties": {
                            "n":      { "type": "integer", "minimum": 0 },
                            "title":  { "type": "string" },
                            "status": { "type": "string", "enum": ["open", "in-progress", "code-complete", "done", "blocked", "dropped"] }
                        }
                    }
                },
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "task", "status"],
                        "properties": {
                            "id":     { "type": "string", "pattern": "^T-\\d+$" },
                            "task":   { "type": "string" },
                            "status": { "type": "string", "enum": ["open", "in-progress", "done", "blocked", "dropped"] },
                            "phase":  { "type": "integer", "minimum": 0 },
                            "notes":  { "type": "string" }
                        }
                    }
                }
            }
        },
        "render_template_example": "## Phase status\n\n| Phase | Title | Status |\n|------:|-------|--------|\n{% for p in phases %}| {{ p.n }} | {{ p.title }} | {{ p.status }} |\n{% endfor %}\n## Tasks\n\n| ID | Task | Status | Phase |\n|---:|------|--------|------:|\n{% for t in tasks %}| {{ t.id }} | {{ t.task }} | {{ t.status }} | {{ t.phase if t.phase is defined else \"—\" }} |\n{% endfor %}",
        "body_skeleton": "## Why this initiative exists\n\n_Brief context._\n\n## Phase descriptions\n\n_For each phase: Why / Shape / Open questions / Acceptance._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the phase + task tables. Mark tasks done as commits land (gather_from: git_log filtered by relevant paths). Add new tasks under the right phase as scope expands. Don't delete completed tasks — they're part of the record. Phase descriptions live in body, individual task one-liners stay in params."
    })
}

fn archetype_reflective() -> Value {
    json!({
        "name": "reflective",
        "when_to_use": "Design brainstorm, decision log, options-being-weighed document. Content requires JUDGMENT, not gathering. Examples: 'plan-lifecycle-tracking', 'heuristic-code-analysis', 'agent-memory-research'. Keep params minimal or empty — the body IS the tracker.",
        "params_shape_example": {
            "status": "scoping",
            "started": "2026-04-21"
        },
        "params_schema_example": {
            "type": "object",
            "properties": {
                "status":  { "type": "string", "enum": ["scoping", "active", "deferred", "decided", "archived"] },
                "started": { "type": "string" }
            }
        },
        "render_template_example": "_**Status:** {{ status or \"scoping\" }}{% if started %} — **Started:** {{ started }}{% endif %}_",
        "body_skeleton": "## Why this exists\n\n_The problem we're scoping._\n\n## Options being weighed\n\n- **Option A** — ...\n- **Option B** — ...\n\n## Anti-goals\n\n_What we're explicitly NOT trying to solve._\n\n## Decision deferred / made\n\n_If decided: when, by whom, why. If deferred: under what conditions we'd revisit._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "This is a reflective tracker — prose-driven, not state-driven. On refresh, do NOT rewrite body sections; only update the lightweight status line in params if the user explicitly changes status. New options or decisions go in body via append. Augmentation refresh should be rare — most updates here are human edits."
    })
}

const SYSTEM_PROMPT: &str = r#"# How to design a tracker

A tracker is an artifact that mixes **live state** (params, refreshed often by gather sources) with **prose** (body, edited rarely by humans). The art of designing a good tracker is putting the right thing in the right place.

## Step 1 — Pick an archetype

Match the user's intent to one of the 6 archetypes. Use this decision sketch:

- **Will state change mechanically per commit/run/deploy?** → `deployment_state`, `failure_table`, `metric_baseline`, `audit_issues`, or `task_list`.
- **Is the content options/decisions/research that requires human judgment?** → `reflective`.
- **Is the structure a numbered table?** → `failure_table` (F-N), `audit_issues` (numbered), or `task_list` (T-N).
- **Is it metrics over time with sessions?** → `metric_baseline`.
- **Is it a feature flag or env state?** → `deployment_state`.
- **None fit cleanly?** Combine — e.g. start with `task_list`'s schema and add `metric_baseline`'s sessions array. Archetypes are starting points, not rules.

If you're unsure, ask the user which pattern fits before composing.

## Step 2 — Write the augmentation prompt

The `prompt` field is a standing instruction the augmentation refresh follows. Rules:

- **Imperative voice.** "Maintain the F-N table" not "this tracks failures".
- **Name the gather sources.** Be explicit which gather sources feed which fields. The synthesizer needs to know.
- **Conflict resolution.** When sources disagree, say which wins. Common: "newer commit beats older params", "params win if `last_seen` is within 24h".
- **Body vs params boundary.** State the rule: "narrative belongs in body, mechanical state in params".
- **Length budget hint.** "Body section under 200 lines, params under 50 entries."

The archetype's `prompt_template` is a starting point — customize for the user's domain.

## Step 3 — Design the params

- **Live state only.** No multi-paragraph strings, no rationale prose.
- **Stable keys.** Renaming a key breaks the template. Pick well, don't churn.
- **Flat-as-possible.** Templates iterate cleanly over flat arrays/dicts. Deep nesting hurts.
- **Use the archetype's `params_shape_example` as a literal starting point**, then trim/extend.
- **Don't put computed-from-other-fields data in params.** Compute in template.

## Step 4 — Decide the schema discipline

- **Early life:** loose schema with `additionalProperties: true`. Let the shape settle over 2-3 refreshes before locking down.
- **Mature:** add `required`, `enum`, `pattern` constraints. Schema lock prevents drift across refreshes.
- **Skip schema entirely** for `reflective` trackers — they don't have meaningful structured params.
- **Validation triggers** on `artifact_augment` (initial seed) and every `artifact_augment(merge=true)` merge. Violations leave params untouched and return a recoverable error.

## Step 5 — Compose the render_template

- MiniJinja syntax. Common patterns:
  - `{% for x in items %}...{% endfor %}` for lists
  - `{% for k, v in dict|items %}...{% endfor %}` for dicts
  - `{{ items|length }}` for counts
  - `{{ items|selectattr(\"status\",\"equalto\",\"fail\")|list|length }}` for filtered counts
  - `{{ value or \"—\" }}` for null fallback
- **Render output** is injected between the `[LIVE]` header and the body excerpt in `librarian_context`. Keep it scannable — tables and short status lines.
- **No template** is fine for `reflective` trackers — omit the field.

## Step 6 — Sketch the body skeleton

- Each archetype has a `body_skeleton`. Use it.
- Body sections are written by humans (or AI in `artifact_refresh` synthesis), edited rarely.
- Always include a **History** section for dated session blocks (`### YYYY-MM-DD — <event>`). This is the universal cross-project pattern.

## Step 7 — Check for collisions

The `existing_trackers` field in this response lists current trackers. Before creating:

- **Same concern already tracked?** Edit existing, don't fork.
- **Related tracker exists?** Use `artifact_link` to wire them after creation.
- **Naming collision?** Use a more specific title.

## Anti-patterns

- ❌ **Narrative in params.** Multi-sentence strings = use body.
- ❌ **Live state in body.** Flag values, F-N statuses, metric numbers = use params.
- ❌ **Premature schema lock-in.** First 2-3 refreshes will reveal shape changes.
- ❌ **Manual tracker file AND `kind: tracker` artifact for the same concern.** Pick one. Manual `docs/trackers/<name>.md` is for content where humans drive; `kind: tracker` augmented artifact is for content where gather+refresh drives.
- ❌ **Over-gathering.** Each gather source costs tokens at refresh time. Only pull what the prompt actually needs.
- ❌ **Empty render_template.** If you set it, make it useful. Don't ship a one-liner template that adds no value over the prompt blockquote.

## Final step

Call `artifact_create` with `kind=tracker`, `status=active`, and `augment={prompt,params}`:
- `path`: `docs/trackers/<slug>.md` (or project equivalent)
- `title`: human-readable
- `topic`: terse keyword for search
- `prompt`: the augmentation prompt you wrote in Step 2
- `params`: the initial params shape from Step 3 (matching schema if set)
- `params_schema`: optional, per Step 4
- `render_template`: optional, per Step 5
- `body`: from Step 6's skeleton, filled with initial content

The artifact + augmentation are created atomically.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{augmentation, Catalog};
    use crate::current_project::CurrentProject;
    use crate::workspace::WorkspaceConfig;
    use jsonschema::validator_for;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(CurrentProject {
                root: "x".into(),
                subdir: "y".into(),
                umbrella: None,
                ..Default::default()
            })),
        }
    }

    #[tokio::test]
    async fn returns_design_envelope() {
        let ctx = mk_ctx();
        let v = TrackerDesign.call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["design_version"], "1");
        assert!(v["system_prompt"].as_str().unwrap().len() > 1000);
        assert_eq!(v["archetypes"].as_array().unwrap().len(), 6);
        assert!(v["next_step"].as_str().unwrap().contains("artifact_create"));
    }

    #[tokio::test]
    async fn lists_existing_trackers_only() {
        let ctx = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            let now = chrono::Utc::now().timestamp_millis();
            for (id, kind) in [("t1", "tracker"), ("d1", "decision")] {
                artifact::upsert(
                    &cat,
                    &artifact::ArtifactRow {
                        id: id.to_string(),
                        repo: "r".into(),
                        rel_path: format!("{id}.md"),
                        kind: kind.into(),
                        status: "active".into(),
                        title: Some(format!("Title {id}")),
                        owners: vec![],
                        tags: vec![],
                        topic: None,
                        time_scope: None,
                        source: None,
                        created_at: now,
                        updated_at: now,
                        file_mtime: now,
                        file_sha256: "x".into(),
                        confidence: 1.0,
                    },
                )
                .unwrap();
                augmentation::upsert(
                    &cat,
                    &augmentation::AugmentationRow {
                        artifact_id: id.into(),
                        prompt: "p".into(),
                        params: "{}".into(),
                        last_refreshed_at: None,
                        refresh_count: 0,
                        created_at: "2026-01-01T00:00:00.000Z".into(),
                        updated_at: "2026-01-01T00:00:00.000Z".into(),
                        render_template: None,
                        params_schema: None,
                        append_mode: false,
                        history_cap: None,
                    },
                )
                .unwrap();
            }
        }

        let v = TrackerDesign.call(&ctx, json!({})).await.unwrap();
        let listed = v["existing_trackers"].as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["id"], "t1");
        assert_eq!(listed[0]["kind"], "tracker");
    }

    #[tokio::test]
    async fn overflow_hint_when_above_cap() {
        let ctx = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            let now = chrono::Utc::now().timestamp_millis();
            for i in 0..(EXISTING_TRACKERS_CAP + 5) {
                let id = format!("t{i}");
                artifact::upsert(
                    &cat,
                    &artifact::ArtifactRow {
                        id: id.clone(),
                        repo: "r".into(),
                        rel_path: format!("{id}.md"),
                        kind: "tracker".into(),
                        status: "active".into(),
                        title: None,
                        owners: vec![],
                        tags: vec![],
                        topic: None,
                        time_scope: None,
                        source: None,
                        created_at: now,
                        updated_at: now,
                        file_mtime: now,
                        file_sha256: "x".into(),
                        confidence: 1.0,
                    },
                )
                .unwrap();
                augmentation::upsert(
                    &cat,
                    &augmentation::AugmentationRow {
                        artifact_id: id,
                        prompt: "p".into(),
                        params: "{}".into(),
                        last_refreshed_at: None,
                        refresh_count: 0,
                        created_at: "2026-01-01T00:00:00.000Z".into(),
                        updated_at: "2026-01-01T00:00:00.000Z".into(),
                        render_template: None,
                        params_schema: None,
                        append_mode: false,
                        history_cap: None,
                    },
                )
                .unwrap();
            }
        }

        let v = TrackerDesign.call(&ctx, json!({})).await.unwrap();
        let listed = v["existing_trackers"].as_array().unwrap();
        assert_eq!(listed.len(), EXISTING_TRACKERS_CAP);
        assert_eq!(
            v["existing_trackers_total"].as_u64().unwrap() as usize,
            EXISTING_TRACKERS_CAP + 5
        );
        assert!(v["existing_trackers_overflow_hint"]
            .as_str()
            .unwrap()
            .contains("artifact_find"));
    }

    #[tokio::test]
    async fn each_archetype_self_consistent() {
        // The example params for each archetype must validate against that
        // archetype's example schema. This catches drift between the two as
        // we evolve archetypes.
        let v = archetypes();
        for arch in v.as_array().unwrap() {
            let name = arch["name"].as_str().unwrap();
            let schema = &arch["params_schema_example"];
            let example = &arch["params_shape_example"];
            let validator = validator_for(schema)
                .unwrap_or_else(|e| panic!("archetype '{name}' has invalid schema: {e}"));
            let errors: Vec<_> = validator.iter_errors(example).collect();
            assert!(
                errors.is_empty(),
                "archetype '{name}' params_shape_example does not validate against params_schema_example: {:?}",
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()
            );
        }
    }

    #[tokio::test]
    async fn each_archetype_template_renders_against_example_params() {
        // Every archetype that ships a render_template must render its
        // own example params without error — catches template/schema drift.
        let v = archetypes();
        for arch in v.as_array().unwrap() {
            let name = arch["name"].as_str().unwrap();
            let Some(tmpl) = arch["render_template_example"].as_str() else {
                continue;
            };
            let params = &arch["params_shape_example"];
            crate::tools::render::render_params(tmpl, params).unwrap_or_else(|e| {
                panic!("archetype '{name}' template fails on its example: {e}")
            });
        }
    }
}
