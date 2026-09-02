//! `tracker_design` — teaching tool. Returns a system_prompt + a library of
//! archetypes + the existing-tracker landscape so the agent can compose a
//! well-shaped tracker spec and call `artifact_create` with confidence.
//!
//! Archetype-driven (not intent-driven) for v1: server stays stateless, no
//! synthesis cost, transparent to the agent. Intent-driven tailoring is
//! deferred until archetype selection proves frustrating in practice.

use crate::librarian::catalog::{artifact, augmentation};
use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
struct Args {
    /// Free-form intent ("tracker for v7.1 flag rollout"). Currently
    /// echoed back; reserved for future intent-driven tailoring.
    #[serde(default)]
    intent: Option<String>,
    /// Fetch ONE archetype in full, by name. Omit for the teaching envelope
    /// plus the archetype menu.
    ///
    /// The two-call shape exists because this action is *guidance*: returning
    /// all eight full specs put the response at ~10,270 tokens — 4.1x the inline
    /// budget — so it overflowed on every call, and the teaching content the
    /// caller is meant to read *before acting* only ever arrived behind a buffer
    /// fetch. See docs/issues/archive/2026-08-16-tracker-design-guidance-always-overflows.md
    #[serde(default)]
    archetype: Option<String>,
}

const DESIGN_VERSION: &str = "1";

/// Cap for the inline existing-trackers sample. Above this, the agent should call
/// `artifact_find {kind:"tracker"}` directly.
///
/// Deliberately small, and tied to the inline budget rather than to usefulness:
/// at 30 this list alone was ~7 KB of a response that must fit in 10 KB — and it
/// was never the right collision check anyway, since a title scan cannot catch a
/// duplicate worded differently. Step 7 now sends the caller to a semantic
/// `doc(find)` for that.
const EXISTING_TRACKERS_CAP: usize = 5;

/// The archetype menu: `name` + `when_to_use` only.
///
/// This is what Step 1 of the system prompt selects on, and it is all Step 1
/// needs. The bulky fields (`params_shape_example`, `params_schema_example`,
/// `render_template_example`, `body_skeleton`, `prompt_template`) are the
/// overwhelming majority of the payload and are fetched one at a time, by name,
/// once a choice has been made.
fn archetype_menu() -> Value {
    let list: Vec<Value> = archetypes()
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| {
                    // Criterion only. Each `when_to_use` ends with an "Examples: …"
                    // clause that illustrates rather than discriminates; it stays in
                    // the full spec, which is one call away once a choice is made.
                    let full = x["when_to_use"].as_str().unwrap_or_default();
                    let criterion = full.split(" Examples:").next().unwrap_or(full).trim();
                    json!({ "name": x["name"], "when_to_use": criterion })
                })
                .collect()
        })
        .unwrap_or_default();
    json!(list)
}

/// Every archetype name — used to resolve a requested one, to name the valid
/// options when resolution fails, and to populate the `archetype` enum in
/// `Librarian::input_schema`. That last caller is why this is `pub(super)`
/// rather than private: the schema must be *derived* from this list, never
/// hand-copied, or the two drift and the tool advertises names it refuses.
pub(super) fn archetype_names() -> Vec<String> {
    archetypes()
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn archetypes() -> Value {
    json!([
        archetype_deployment_state(),
        archetype_failure_table(),
        archetype_metric_baseline(),
        archetype_audit_issues(),
        archetype_task_list(),
        archetype_reflective(),
        archetype_goal(),
        archetype_constitution(),
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
        "prompt_template": "This tracker is the source of truth for feature flag `<NAME>`'s rollout — read the env table above before relying on the flag. To act: if your target env shows `enabled: false`, the flag is not live there; `since` shows how long the current state has held. To maintain (refresh only): pull current values from settings files via `gather_from: file`; when envs disagree with the deployed commit (`gather_from: git_log`), prefer the most recent commit-derived value and note the divergence in body history; keep params strictly mechanical — narrative belongs in body."
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
        "body_skeleton": "## Suite methodology\n\n_What the suite tests, how it runs, where results live._\n\n## F-N — <title>\n\n_One section per failure, and NOT optional: this heading is the only thing that makes `F-N` citable — link_scan binds a token to `## <ID> — <title>` and a params row or a rendered table defines nothing. The render_template table is a reading surface in librarian(context), never a definition._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the F-N failure list. After each suite run (gather_from: file pointing at the latest junit/json report), update each failure's status, last_seen, and notes. Add new F-N entries for new failures (next free integer). Never delete an entry — mark fixed entries as pass with a notes line citing the commit. Body holds methodology and per-failure deep dives.",
        "entry_collection": "failures"
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
                { "n": 1, "title": "Long PDFs split mid-sentence", "severity": "high", "status": "fixed", "owner": "@mareurs",
                  "severity_reason": "splits within a paragraph break sentence boundaries; downstream retrieval scores 0.05 lower P@5",
                  "ref_kind": "code_symbol", "md_file": "docs/chunking-audit.md", "md_line": 42,
                  "raw_ref": "src/chunking/splitter.rs::Splitter::next_chunk",
                  "first_seen_commit": "abc1234", "first_seen_at": "2026-04-12T09:30:00Z",
                  "last_verified_at": "2026-05-15T14:20:00Z" },
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
                            "n":                 { "type": "integer", "minimum": 1 },
                            "title":             { "type": "string" },
                            "severity":          { "type": "string", "enum": ["high", "med", "low"] },
                            "status":            { "type": "string", "enum": ["open", "in-progress", "fixed", "wontfix"] },
                            "owner":             { "type": "string" },
                            "severity_reason":   { "type": "string", "description": "Optional rationale for the severity rating." },
                            "ref_kind":          { "type": "string", "description": "Optional: nature of the target ref (e.g. code_symbol, file_path, url, line)." },
                            "md_file":           { "type": "string", "description": "Optional: path of the markdown document where this finding originated." },
                            "md_line":           { "type": "integer", "minimum": 1, "description": "Optional: line number within md_file." },
                            "raw_ref":           { "type": "string", "description": "Optional: the exact ref string as it appeared in the source." },
                            "first_seen_commit": { "type": "string", "description": "Optional: git commit SHA where the issue was first observed." },
                            "first_seen_at":     { "type": "string", "format": "date-time", "description": "Optional: ISO-8601 timestamp of first observation." },
                            "last_verified_at":  { "type": "string", "format": "date-time", "description": "Optional: ISO-8601 timestamp of the most recent check that re-confirmed the issue." }
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
        "body_skeleton": "## Why this initiative exists\n\n_Brief context._\n\n## Phase descriptions\n\n_For each phase: Why / Shape / Open questions / Acceptance._\n\n## T-N — <title>\n\n_One section per task. This heading is the only thing that makes `T-N` citable — link_scan binds a token to `## <ID> — <title>`, and neither a params row nor the rendered table above defines one. A task list whose entries live only in rows has entries nothing can ever reference._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "Maintain the phase + task tables. Mark tasks done as commits land (gather_from: git_log filtered by relevant paths). Add new tasks under the right phase as scope expands. Don't delete completed tasks — they're part of the record. Phase descriptions live in body, individual task one-liners stay in params — but every task also gets its own `## T-N — <title>` section, which is what makes it citable.",
        "entry_collection": "tasks"
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

// Phase 1 — per-archetype reconciliation clauses. These are the strings the LLM
// reads from rule 1 of the augmentation prompt. After Phase 1 lands they are
// also the strings `goal_aggregation::child_status_pure` is unit-tested against,
// keeping prompt and code in sync.
//
// Edit here, NOT inline in `archetype_goal()`'s prompt JSON.

fn archetype_goal() -> Value {
    json!({
        "name": "goal",
        "when_to_use": "Tracking an outcome-stated objective whose completion depends on a named criterion and on aggregated state of sibling/child artifacts. Use when the work has a definable 'done' line, decomposes into typed sub-trackers (tests, tasks, metrics, audits), and survives across sessions. Examples: 'all flaky tests resolved + suite green for 3 runs', 'retrieval P@5 reaches 0.20 on benchmark X', 'plan-lifecycle subsystem ships behind feature flag'. Not for: open-ended research (use `reflective`), single-metric tracking (use `metric_baseline`), bare task lists with no completion semantics (use `task_list`), goals with fewer than 2 child sub-trackers (the container archetype's job is aggregation — without 2+ children to aggregate, use the underlying archetype directly).",
        "params_shape_example": {
            "criterion": "Retrieval pipeline P@5 ≥ 0.20 on benchmark-25tc, with no regression on R@5",
            "status": "active",
            "blocked_reason": null,
            "acceptance_signals": [
                {"description": "P@5 ≥ 0.20 on benchmark-25tc", "met": false, "evidence": "metric_baseline C-1: current.P@5=0.193 >= 0.20", "kind": "metric_threshold", "evidence_child_id": "C-1", "metric_key": "P@5", "op": ">=", "threshold": 0.20},
                {"description": "No new failures in chat-eval-v3", "met": true,  "evidence": "failure_table C-2: 0/12 fail|flaky", "kind": "failure_table_clean", "evidence_child_id": "C-2"},
                {"description": "All reranker-tuning tasks done", "met": false, "evidence": "task_list C-3: 4/7 done", "kind": "task_list_complete", "evidence_child_id": "C-3"},
                {"description": "Out-of-band human review complete", "met": false, "evidence": "pending stakeholder sign-off", "kind": "freeform"}
            ],
            "children": [
                {"id": "C-1", "artifact_id": "a1b2c3d4", "title": "Retrieval Benchmark",   "archetype": "metric_baseline", "status": "in-progress"},
                {"id": "C-2", "artifact_id": "d4e5f6a7", "title": "chat-eval-v3 failures",  "archetype": "failure_table",   "status": "active"},
                {"id": "C-3", "artifact_id": "b9c8d7e6", "title": "Reranker tuning tasks",  "archetype": "task_list",        "status": "done"}
            ],
            "progress_log": [
                {"date": "2026-05-12", "note": "Reranker tuning landed. P@5 0.145 → 0.193.", "evidence_commits": ["abc1234"], "evidence_artifacts": ["a1b2c3d4"]},
                {"date": "2026-05-14", "note": "chat-eval-v3 stable. Need final 7pt P@5.",  "evidence_commits": [],          "evidence_artifacts": ["d4e5f6a7"]}
            ],
            "gather_from": [
                {
                    "source": "git_log",
                    "since": "last_refreshed_at",
                    "limit": 30,
                    "grep": "<path or component pattern named in your criterion>"
                }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["criterion", "status", "children"],
            "properties": {
                "criterion":      { "type": "string" },
                "status":         { "type": "string", "enum": ["scoping","active","pending-confirmation","done","blocked","abandoned"] },
                "blocked_reason": { "type": ["string","null"] },
                "acceptance_signals": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["description","met"],
                        "properties": {
                            "description":       { "type": "string" },
                            "met":               { "type": "boolean" },
                            "evidence":          { "type": "string" },
                            "kind":              { "type": "string", "enum": ["freeform","audit_issues_open_count","failure_table_clean","task_list_complete","metric_threshold","reflective_decided","deployment_envs_enabled"], "description": "Optional discriminator for Rust-side evaluation (amendment D4). Default freeform — human-evaluated. Other kinds drive deterministic .met derivation from the cited child's params." },
                            "evidence_child_id": { "type": "string", "pattern": "^C-\\d+$", "description": "Required for non-freeform kinds — names the child whose params satisfy the signal." },
                            "max_open":          { "type": "integer", "minimum": 0, "description": "audit_issues_open_count only — max allowed status=open count." },
                            "metric_key":        { "type": "string", "description": "metric_threshold only — key path under child's params.current to read." },
                            "op":                { "type": "string", "enum": [">=",">","<=","<","=="], "description": "metric_threshold only — comparison operator." },
                            "threshold":         { "type": "number", "description": "metric_threshold only — RHS of the comparison." },
                            "envs":              { "type": ["array","null"], "items": {"type": "string"}, "description": "deployment_envs_enabled only — subset of envs required enabled. null = all envs must be enabled." }
                        }
                    }
                },
                "children": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id","artifact_id","title","archetype","status"],
                        "properties": {
                            "id":          { "type": "string", "pattern": "^C-\\d+$" },
                            "artifact_id": { "type": "string" },
                            "title":       { "type": "string" },
                            "archetype":   { "type": "string" },
                            "status":      { "type": "string", "enum": ["pending","active","in-progress","done","blocked","orphan","unknown"] }
                        }
                    }
                },
                "progress_log": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["date","note"],
                        "properties": {
                            "date":               { "type": "string" },
                            "note":               { "type": "string" },
                            "evidence_commits":   { "type": "array", "items": { "type": "string" } },
                            "evidence_artifacts": { "type": "array", "items": { "type": "string" } }
                        }
                    }
                }
            }
        },
        "render_template_example": "**Goal:** {{ criterion }}\n**Status:** {{ status }}{% if blocked_reason %} — _blocked: {{ blocked_reason }}_{% endif %}\n\n{% if acceptance_signals %}**Acceptance signals** — {{ acceptance_signals|selectattr(\"met\")|list|length }}/{{ acceptance_signals|length }} met\n\n| signal | met | evidence |\n|--------|:---:|----------|\n{% for s in acceptance_signals %}| {{ s.description }} | {{ \"✅\" if s.met else \"❌\" }} | {{ s.evidence or \"—\" }} |\n{% endfor %}{% endif %}\n\n**Children** — {{ children|selectattr(\"status\",\"equalto\",\"done\")|list|length }}/{{ children|length }} done\n\n| id | title | archetype | status |\n|---:|-------|-----------|--------|\n{% for c in children %}| {{ c.id }} | {{ c.title }} | {{ c.archetype }} | {{ c.status }} |\n{% endfor %}\n\n{% if progress_log %}**Recent progress** _(last 3 of {{ progress_log|length }})_\n\n{% for p in progress_log|reverse|slice(3)|first %}- **{{ p.date }}**: {{ p.note }}\n{% endfor %}{% endif %}",
        "body_skeleton": "## Why this goal exists\n\n_Briefly: the business / engineering driver. Two to four sentences._\n\n## Acceptance criteria (prose)\n\n_Long-form acceptance criteria. Mirrors `acceptance_signals` in params but with rationale, counterexamples, and what's explicitly out of scope._\n\n## Decomposition rationale\n\n_Why these children, in this archetype mix. When new children are spawned mid-refresh, the synthesizer appends a one-paragraph rationale here citing the trigger._\n\n## History\n\n_### YYYY-MM-DD — <event>_\n",
        "prompt_template": "Maintain a goal-tracker. Your job is **aggregation**, not evaluation: reconcile each child's state into the goal's params using ground truth supplied by the refresh pipeline. Do not recompute children's evidence — trust the child's own params.\n\nINPUTS (gather):\n- This goal's current params.\n- `context.deterministic_child_statuses` — an array, one entry per linked child, of `{child_id, artifact_id, archetype, status, basis}`. `basis` is `\"deterministic\"` for archetypes the Rust kernel resolved, `\"needs parent context\"` for `metric_baseline` (you evaluate it via rule 1b), `\"unknown archetype\"` for any archetype the kernel doesn't know, `\"no augmentation\"` if the child has no augmentation row, or `\"child unreachable\"` if the artifact_id has no row in the catalog.\n- Optional: commit log scoped to paths the criterion names (gather_from: git_log).\n\nUPDATE RULES:\n\n1. Reconcile each `children[].status` from the child's actual status:\n\n   a. **First, copy ground truth from `context.deterministic_child_statuses`.** For every entry whose `basis == \"deterministic\"`, copy its `status` into `children[id].status` verbatim. Do not reinterpret — these archetypes are handled by the Rust kernel and its verdict is authoritative.\n\n   b. **For entries whose `basis != \"deterministic\"`** — e.g. `basis == \"needs parent context\"` (a `metric_baseline` child with no citing `acceptance_signals[kind=metric_threshold]`), `basis == \"unknown archetype\"` (archetype not yet in the Rust kernel), or `basis == \"no augmentation\"` (child has no augmentation row) — leave `children[id].status` at the value the entry carries (the gather produced a best-guess `\"active\"`) and append a progress_log note flagging the gap so a future refresh can close it.\n\n   c. **For entries with `basis == \"child unreachable\"`** → set `children[id].status = \"orphan\"`. Do NOT delete the row.\n\n2. For each `acceptance_signals[i]`, set `.met` by looking up the cited child's params and copying the relevant value forward. Do not re-derive the underlying metric — read it from the child's params verbatim and compare against the signal's description. Update the `evidence` string to cite the child id and the specific datum.\n\n3. `refresh_meta` is **read-only** for you — Rust computes it. Copy `context.refresh_meta` verbatim into `params.refresh_meta` (every field, byte-identical). Then optionally append at most one entry to `progress_log` IFF `refresh_meta.children_status_delta` is non-empty OR `refresh_meta.commit_count_since_last > 0`. Skip the append on a no-change refresh — Rust increments `refresh_meta.unchanged_refreshes`. **When you DO append, populate every field of the new entry — none are optional in practice:**\n   - `date`: today (UTC, `YYYY-MM-DD`).\n   - `note`: one-line summary — cite the children that flipped status and the commits that landed.\n   - `evidence_commits`: short `hash` strings selected from `context.git_log`, restricted to commits whose `subject` indicates work on a path or component named in this goal's `criterion`. The gather is already date-windowed (the augmentation's `gather_from: git_log` should set `since: \"last_refreshed_at\"` so `context.git_log` only carries commits after `refresh_meta.last_refresh_at`). `refresh_meta.commit_count_since_last` equals `len(context.git_log)`; if zero, leave `[]`.\n   - `evidence_artifacts`: child `artifact_id`s for each entry in `refresh_meta.children_status_delta` — look up the delta's `child_id` in `params.children` and copy its `artifact_id`. If the delta is empty, leave `[]`.\n\n4. AUTO-CLOSE GATE — **Rust enforces this; you may propose `status: \"done\"` but the merge will be rejected unless ALL conditions hold:**\n   a. `len(children) >= 2` (amendment D9 — single-child or empty goals should use the underlying archetype directly)\n   b. Every `children[].status == \"done\"`\n   c. Every `acceptance_signals[].met` is true\n   If you flip status to \"done\" with any condition unmet, the augment call returns a recoverable error citing the failing condition. Leave status unchanged in that case; the prior body's History entry stays as-is.\n\n5. SCOPE GROWTH: if your aggregation surfaces a missing sub-objective, you MAY add **at most one** new child per refresh (amendment D10 — Rust rejects merges that introduce more than one new `children[].id`). Defer the rest to a follow-up refresh.\n   a. Call doc(action=\"create\", kind=\"tracker\", augment={}) with the appropriate existing archetype (failure_table, task_list, metric_baseline, audit_issues, reflective, deployment_state, or nested goal).\n   b. Call doc(action=\"link\", src_id=THIS_GOAL_ID, dst_id=NEW_CHILD_ID, rel=\"child\").\n   c. Add the new child to `children[]` with the next free C-N id.\n   d. Append one paragraph to body \"Decomposition rationale\" citing the trigger.\n\nSTOP CONDITION (you are done with this refresh when):\n- All children reconciled.\n- One progress_log entry appended.\n- Auto-close gate evaluated.\n- Output: the new params object. Body edits only for History append or Decomposition rationale append on scope growth.\n\nBody holds rationale and history; params hold mechanical state. Keep them separated."
    })
}

fn archetype_constitution() -> Value {
    json!({
        "name": "constitution",
        "when_to_use": "Rules the agent MUST follow no matter what, enforced mechanically rather than by prose trust — not a place for advisory/'should' guidance (use a regular tracker or memory for that). Tag the tracker artifact's `tags` with `\"constitution\"` so codescout-companion's enforcement hooks can find it — the archetype shape alone does not enable enforcement. Examples: 'solver invariants (path-scoped)', 'never commit secrets (global, no paths)'.",
        "params_shape_example": {
            "rules": [
                {
                    "id": "C-1",
                    "paths": ["**/solver/**", "**/*Constraint*.kt"],
                    "title": "Never disable a constraint via weight 0",
                    "rule": "A constraint_profiles weight of 0 or 1 is a sentinel, not disabled — read the lambda before touching it.",
                    "status": "active"
                },
                {
                    "id": "C-2",
                    "title": "Never commit secrets",
                    "rule": "Never stage .env, credentials.json, or any file matching *_key*/*_secret* without explicit user confirmation.",
                    "status": "active"
                }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["rules"],
            "properties": {
                "rules": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "title", "rule", "status"],
                        "properties": {
                            "id":     { "type": "string", "pattern": "^C-\\d+$" },
                            "paths":  { "type": "array", "items": { "type": "string" } },
                            "title":  { "type": "string" },
                            "rule":   { "type": "string" },
                            "status": { "type": "string", "enum": ["active", "superseded"] }
                        }
                    }
                }
            }
        },
        "render_template_example": "**Active rules:** {{ rules|selectattr(\"status\",\"equalto\",\"active\")|list|length }} / {{ rules|length }}\n\n| id | scope | title |\n|----|-------|-------|\n{% for r in rules %}| {{ r.id }} | {{ \"path-scoped\" if r.paths else \"global\" }} | {{ r.title }} |\n{% endfor %}",
        "body_skeleton": "## Why this constitution exists\n\n_What domain these rules guard, and why prose alone wasn't enough._\n\n## Per-rule detail\n\n_One `## C-N — <title>` section per rule: why / how to apply / evidence. The dash-and-title is load-bearing — `## C-N` alone defines no citable token._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "This tracker holds rules the agent must follow no matter what — single-tier, mechanically enforced (path-scoped rules via a PreToolUse deny, global rules via a UserPromptSubmit injection), never prose-trust alone. To act: if a tool call was denied citing a C-N rule, read that rule's body section before retrying. To maintain: add new C-N entries via the `append_entry` primitive (never hand-pick the next integer — see docs/superpowers/specs/2026-07-06-librarian-atomic-index-allocation-design.md); never delete an entry — supersede a wrong one with status=superseded plus a pointer to its replacement. This artifact's `tags` must include `\"constitution\"` for the enforcement hooks to find it.",
        "entry_collection": "rules"
    })
}

const SYSTEM_PROMPT: &str = r#"# How to design a tracker

A tracker is an artifact that mixes **live state** (params, refreshed often by gather sources) with **prose** (body, edited rarely by humans). The art of designing a good tracker is putting the right thing in the right place.

## Step 1 — Pick an archetype

Match the user's intent to one of the 8 archetypes. Use this decision sketch:

- **Will state change mechanically per commit/run/deploy?** → `deployment_state`, `failure_table`, `metric_baseline`, `audit_issues`, or `task_list`.
- **Is the content options/decisions/research that requires human judgment?** → `reflective`.
- **Does it hold rules the agent must follow no matter what, mechanically enforced rather than just documented?** → `constitution`. Remember to tag the artifact `"constitution"` — the archetype shape alone doesn't enable enforcement.
- **Is the structure a numbered table?** → `failure_table` (F-N), `audit_issues` (numbered), or `task_list` (T-N).
- **Is it metrics over time with sessions?** → `metric_baseline`.
- **Is it a feature flag or env state?** → `deployment_state`.
- **None fit cleanly?** Combine — e.g. start with `task_list`'s schema and add `metric_baseline`'s sessions array. Archetypes are starting points, not rules.

If you're unsure, ask the user which pattern fits before composing.

## Step 2 — Write the augmentation prompt

The `prompt` has **two readers**, and most authors brief only the first: the **refresh synthesizer**
that maintains the tracker, and **every agent that meets it cold** via the `[LIVE]` block in
`librarian(context)` — which sees this prompt verbatim as a standing instruction.

Write **reader-first**: lead with what a consumer should *do with* this tracker, then how to
maintain it. A tracker is a skill for its own state.

- **Open with the read/act contract.** "To act: take the highest-severity `open` row, read its body
  section, verify against current code before fixing."
- **Then the maintenance rules** (for the synthesizer): imperative voice ("Maintain the F-N table",
  not "this tracks failures"); name which gather source feeds which field; say which source wins on
  conflict ("newer commit beats older params"); state the body-vs-params boundary; give a length
  budget ("body under 200 lines, params under 50 entries").
- **Escape hatch** when consumers outnumber maintainers: "Readers: the gather/refresh rules below
  are not yours."

The archetype's `prompt_template` already leads reader-first — keep that order.

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

- MiniJinja: `{% for x in items %}`, `{% for k, v in dict|items %}`, `{{ items|length }}`,
  `{{ items|selectattr(\"status\",\"equalto\",\"fail\")|list|length }}`, `{{ value or \"—\" }}`.
- **Render output** is injected between the `[LIVE]` header and the body excerpt in `librarian_context`. Keep it scannable — tables and short status lines.
- **No template** is fine for `reflective` trackers — omit the field.

## Step 5b — Make entries filterable (optional)

Set `entry_collection` to the params key holding your array of entry objects (e.g. `"failures"`). That enables `doc(get, id=..., entry_filter={field:{op:value}})` — same filter syntax as `doc(find)`. Only archetypes keeping entries in params (`failure_table`, `task_list`) support it; `reflective` keeps entries in prose — retrofit first (`docs/conventions/retrofitting-trackers-for-filtering.md`).

## Step 6 — Sketch the body skeleton

- Each archetype has a `body_skeleton`. Use it.
- **A `PREFIX-N` namespace means every entry needs its own `## <ID> — <title>` section** — the only shape `link_scan` reads as a definition. A params row, a `## T-4` missing the dash-and-title, and the `render_template` table (which renders into `librarian(context)`, never the file) all define nothing, so entries written that way can never be cited. `doctor` reports the gap; `get_guide("tracker-conventions")` has the measurements.
- Body sections are written by humans (or AI in `artifact_refresh` synthesis), edited rarely.
- Always include a **History** section for dated session blocks (`### YYYY-MM-DD — <event>`). This is the universal cross-project pattern.

## Step 7 — Check for collisions

`existing_trackers` in this response is a **sample**, not the list. For a real check run
`doc(find, kind="tracker", semantic="<your concern>")` — scanning titles will not catch a
duplicate that is worded differently, which is the collision that actually happens.

- **Same concern already tracked?** Edit existing, don't fork.
- **Related tracker exists?** Use `artifact_link` to wire them after creation.

## Anti-patterns

- ❌ **Narrative in params.** Multi-sentence strings = use body.
- ❌ **Live state in body.** Flag values, F-N statuses, metric numbers = use params.
- ❌ **Premature schema lock-in.** First 2-3 refreshes will reveal shape changes.
- ❌ **Manual tracker file AND `kind: tracker` artifact for one concern.** Pick one: manual for human-driven content, augmented artifact for gather+refresh-driven.
- ❌ **Over-gathering.** Each gather source costs tokens per refresh. Pull only what the prompt needs.
- ❌ **Empty render_template.** If you set it, make it earn its place.

## Final step

One call. `artifact_create` — `kind=tracker`, `status=active`, `rel_path`
(`docs/trackers/<slug>.md`), `title`, `topic`, `body` (Step 6), and `augment={...}`.

`augment` takes the **whole** augmentation, so there is no follow-up call to forget:
`prompt` (required), `params` (Step 3), `render_template` (Step 5), `params_schema` (Step 4),
`entry_collection` (Step 5b), `append_mode`, `history_cap`. An unknown key is **rejected, not
ignored** — a typo fails loudly instead of vanishing.

To change an augmentation afterwards use `artifact_augment(id=..., merge=true, ...)`.
**`merge=true` matters:** `merge=false` overwrites all seven fields, resetting everything you omit.
"#;
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // `unwrap_or_default()` here until 2026-09-01, which made ANY malformed argument
    // discard EVERY argument and still return 200-with-the-menu: `archetype=[]` silently
    // became `archetype=None`, and so did a perfectly good `intent` sent alongside it. The
    // caller's only evidence was a successful response containing the menu they did not ask
    // for. `IC-15`'s remedy is to refuse rather than to drop — refusing is loud, immediate,
    // and tells the caller the truth. Found by
    // `Librarian::every_action_labelled_schema_key_is_honored_by_that_action`, which flagged
    // both fields as unhonored because a type error and an absent key were indistinguishable
    // downstream.
    let a: Args = serde_json::from_value(args).map_err(|e| {
        RecoverableError::new(format!(
            "tracker_design: could not read arguments — {e}. Valid params are `intent` \
             (string) and `archetype` (string, one of: {}).",
            archetype_names().join(", ")
        ))
    })?;

    // Named fetch: one archetype in full. Returns early — a caller who has already
    // chosen needs neither the teaching prompt again nor the catalog walk.
    if let Some(want) = a.archetype.as_deref() {
        let found = archetypes()
            .as_array()
            .and_then(|list| list.iter().find(|x| x["name"] == want).cloned());
        let Some(found) = found else {
            return Err(RecoverableError::with_hint(
                format!("unknown archetype '{want}'"),
                format!("Valid archetypes: {}.", archetype_names().join(", ")),
            ));
        };
        return Ok(json!({
            "design_version": DESIGN_VERSION,
            "archetype": found,
        }));
    }

    let cat = ctx.catalog.lock();

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
        // Only what Step 7 reads. `last_refreshed_at` / `refresh_count` were carried
        // here but answer no question the collision check asks — and dropping them
        // also drops an `augmentation::get` per tracker.
        existing.push(json!({
            "id": id,
            "title": art.title,
            "kind": art.kind,
        }));
    }

    let mut response = json!({
        "design_version": DESIGN_VERSION,
        "system_prompt": SYSTEM_PROMPT,
        // Menu only — name + when_to_use, which is what Step 1 selects on. The
        // examples are ~95% of the payload and are fetched one at a time.
        "archetypes": archetype_menu(),
        "archetype_detail": "Once you have picked one, call librarian(action=\"tracker_design\", archetype=\"<name>\") for its params_shape_example, params_schema_example, render_template_example, body_skeleton and prompt_template.",
        "existing_trackers": existing,
        "existing_trackers_total": total_trackers,
        "intent": a.intent,
        "next_step": "Pick an archetype, then call librarian(tracker_design, archetype=\"<name>\") for its examples. Compose the spec and call artifact_create with kind=tracker, status=active, body, and augment={prompt, params, render_template, params_schema, entry_collection} — augment takes the full augmentation in one call, and rejects unknown keys.",
    });

    if total_trackers > EXISTING_TRACKERS_CAP {
        response["existing_trackers_overflow_hint"] = json!(format!(
            "Showing {EXISTING_TRACKERS_CAP} of {total_trackers}. For full list use artifact_find {{\"kind\":\"tracker\"}}."
        ));
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{augmentation, Catalog};
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;
    use jsonschema::validator_for;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(CurrentProject {
                abs_path: std::path::PathBuf::from("/test/x/y"),
                git_root: std::path::PathBuf::from("/test/x"),
                main_root: None,
                umbrella: None,
            }))
            .build()
    }

    #[tokio::test]
    async fn returns_design_envelope() {
        let ctx = mk_ctx();
        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["design_version"], "1");
        assert!(v["system_prompt"].as_str().unwrap().len() > 1000);
        assert_eq!(v["archetypes"].as_array().unwrap().len(), 8);
        assert!(v["next_step"].as_str().unwrap().contains("artifact_create"));
    }

    /// BL-5. This action exists to *teach the caller before they act* — CLAUDE.md
    /// and `get_guide("librarian")` both route here ahead of `doc(create)`.
    /// Measured 2026-08-16: it had overflowed on 6 of 6 calls in the corpus at a
    /// near-constant ~10,270 tokens, 4.1x the inline budget, so the guidance has
    /// **never once arrived inline** and every caller had to fetch it back out of
    /// a buffer first — a step that measurably fails elsewhere.
    ///
    /// Mutation caught: folding the full archetype specs back into the default
    /// response restores the permanent overflow.
    #[tokio::test]
    async fn default_response_fits_inline() {
        let ctx = mk_ctx();
        // Seed a FULL catalog. An empty one understates the payload by several
        // KB, because `existing_trackers` is populated in production and not in
        // a bare fixture — a test that passes at 9.9KB against zero trackers
        // would still overflow on every real call. Titles and paths are sized
        // from this repo's actual trackers, which run ~90 chars.
        {
            let cat = ctx.catalog.lock();
            let now = chrono::Utc::now().timestamp_millis();
            for i in 0..EXISTING_TRACKERS_CAP {
                let id = format!("seed{i:04}");
                artifact::upsert(
                    &cat,
                    &artifact::ArtifactRow {
                        id: id.clone(),
                        abs_path: std::path::PathBuf::from(format!(
                            "/repo/docs/trackers/a-fairly-long-tracker-slug-{i}.md"
                        )),
                        kind: "tracker".into(),
                        status: "active".into(),
                        title: Some(format!(
                            "Tracker {i} — a representative title of the length this repo actually uses (N-N)"
                        )),
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
                        entry_collection: None,
                        refreshed_at_commit: None,
                    },
                )
                .unwrap();
            }
        }

        let v = call(&ctx, json!({})).await.unwrap();
        let bytes = serde_json::to_string(&v).unwrap().len();
        // TOOL_OUTPUT_BUFFER_THRESHOLD is the actual buffering trigger
        // (`exceeds_inline_limit`); INLINE_BYTE_BUDGET's 9 KB is the slice budget
        // for content that has ALREADY overflowed, so it is not the bar here.
        //
        // Measured 2026-08-16: 9,358 bytes with a full catalog — ~640 bytes of
        // headroom, down from ~41,000 pre-split. That margin is thin: growing the
        // system prompt or raising EXISTING_TRACKERS_CAP will re-break this, and
        // this assertion is what will say so.
        //
        // It said so, on 2026-08-18. Adding one Step 6 bullet about entry-defining
        // headings took the response to 10,096 and broke this test; the bullet was cut
        // to its essential rule (the measurements moved to
        // `get_guide("tracker-conventions")`, which is where overflow belongs) and it
        // now measures **9,898 bytes — about 102 bytes of headroom.**
        //
        // Treat 102 bytes as zero. The next addition of any size to SYSTEM_PROMPT has
        // to pay for itself by cutting elsewhere, and the cut has to be argued rather
        // than eyeballed. Re-measure by flipping this assertion to `bytes < 1` and
        // reading the panic — the number is not printed on success.
        assert!(
            bytes < crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "tracker_design must arrive inline with a full catalog: {bytes} bytes \
                 against a {} byte threshold",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
        );
    }

    /// The split must not cost Step 1. Picking an archetype is the first thing the
    /// system prompt asks for, so every archetype stays listed by name and
    /// `when_to_use` — only the bulky examples move behind a second call.
    #[tokio::test]
    async fn default_response_still_lists_every_archetype_to_choose_from() {
        let ctx = mk_ctx();
        let v = call(&ctx, json!({})).await.unwrap();
        let list = v["archetypes"].as_array().expect("archetypes array");
        assert_eq!(list.len(), 8, "all 8 must remain choosable inline");
        for a in list {
            assert!(a["name"].is_string(), "menu entry needs a name: {a}");
            assert!(
                a["when_to_use"].is_string(),
                "menu entry needs when_to_use — that is what Step 1 selects on: {a}"
            );
            assert!(
                a["params_shape_example"].is_null(),
                "the heavy fields are what blew the budget; they must NOT be inline: {a}"
            );
        }
    }

    /// The other half of the split: one archetype, in full, on request.
    #[tokio::test]
    async fn named_archetype_returns_the_full_spec() {
        let ctx = mk_ctx();
        let v = call(&ctx, json!({ "archetype": "task_list" }))
            .await
            .unwrap();
        let a = &v["archetype"];
        assert_eq!(a["name"], "task_list");
        assert!(
            a["params_shape_example"].is_object(),
            "a named fetch must carry the examples the menu omits, got: {a}"
        );
        assert!(a["body_skeleton"].is_string());
        assert!(a["prompt_template"].is_string());
    }

    /// An unknown name must say what the valid ones are rather than returning an
    /// empty envelope the caller has to diagnose.
    #[tokio::test]
    async fn unknown_archetype_names_the_valid_ones() {
        let ctx = mk_ctx();
        let err = call(&ctx, json!({ "archetype": "nope" }))
            .await
            .expect_err("an unknown archetype must error");
        let msg = err.to_string();
        assert!(
            msg.contains("task_list"),
            "error must list valid names: {msg}"
        );
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
                        abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
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
                        entry_collection: None,
                        refreshed_at_commit: None,
                    },
                )
                .unwrap();
            }
        }

        let v = call(&ctx, json!({})).await.unwrap();
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
                        abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
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
                        entry_collection: None,
                        refreshed_at_commit: None,
                    },
                )
                .unwrap();
            }
        }

        let v = call(&ctx, json!({})).await.unwrap();
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
            crate::librarian::tools::render::render_params(tmpl, params).unwrap_or_else(|e| {
                panic!("archetype '{name}' template fails on its example: {e}")
            });
        }
    }

    /// docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
    ///
    /// An archetype that keeps id-bearing entries in `params` is teaching a LEDGER,
    /// and a ledger's entries are citable only through `## <ID> — <title>`:
    /// `link_scan` derives a definition from that heading shape and from nothing
    /// else. A `body_skeleton` offering only prose sections therefore teaches a
    /// tracker whose every entry is unreachable — and the `render_template` table
    /// does not rescue it, because that output goes to `librarian(context)` and
    /// never to the file.
    ///
    /// Not hypothetical. Measured 2026-08-18 across the catalog: 13 ledgers in five
    /// repos had entries their body never defines, the largest at 64 of 68, and the
    /// archetype library is where their shape came from.
    ///
    /// Keyed on each archetype's OWN `entry_collection` declaration rather than a
    /// hardcoded name list, so a new archetype that declares one is covered without
    /// anyone remembering to edit this test.
    #[test]
    fn every_archetype_with_an_entry_collection_teaches_where_the_defining_heading_goes() {
        // The dash-and-title is the load-bearing half: `## C-N` alone defines
        // nothing, which link_scan's own `heading_without_dash_separator_does_not_define`
        // pins. So the skeleton has to show the full shape, not just the token.
        let re = regex::Regex::new(r"#{2,6}\s+[A-Z]{1,3}-N\s+—\s+<title>").unwrap();
        for arch in archetypes().as_array().unwrap() {
            let Some(collection) = arch["entry_collection"].as_str() else {
                continue;
            };
            let name = arch["name"].as_str().unwrap();
            let skeleton = arch["body_skeleton"].as_str().unwrap_or("");
            assert!(
                re.is_match(skeleton),
                "archetype '{name}' keeps entries in params (`{collection}`) but its \
                     body_skeleton never shows a `## <PREFIX>-N — <title>` heading, so an author \
                     following it writes a ledger whose entries nothing can cite. Skeleton was: \
                     {skeleton}"
            );
        }
    }

    #[test]
    fn failure_table_archetype_has_entry_collection_field() {
        let v = archetype_failure_table();
        assert_eq!(
            v["entry_collection"].as_str(),
            Some("failures"),
            "failure_table archetype must advertise entry_collection = \"failures\""
        );
    }

    #[test]
    fn constitution_archetype_has_entry_collection_field() {
        let v = archetype_constitution();
        assert_eq!(
            v["entry_collection"].as_str(),
            Some("rules"),
            "constitution archetype must advertise entry_collection = \"rules\""
        );
    }

    #[tokio::test]
    async fn constitution_archetype_present_and_registered() {
        let v = archetypes();
        let arr = v.as_array().unwrap();
        let names: Vec<&str> = arr.iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert!(
            names.contains(&"constitution"),
            "constitution archetype missing from archetypes() — got {names:?}"
        );
    }

    #[tokio::test]
    async fn goal_archetype_present_and_registered() {
        let v = archetypes();
        let arr = v.as_array().unwrap();
        assert_eq!(
            arr.len(),
            8,
            "expected 8 archetypes including goal and constitution"
        );
        let names: Vec<&str> = arr.iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert!(
            names.contains(&"goal"),
            "goal archetype missing from archetypes() — got {names:?}"
        );
    }

    /// Drift tripwire — enumerates archetypes that the goal-aggregation kernel
    /// resolves to non-`Unknown` and asserts the prompt's rule 1 correctly
    /// delegates to Rust for exactly those. If a future edit adds an archetype
    /// to the kernel but forgets to collapse its prompt clause (or removes
    /// one from the kernel while leaving the "copy verbatim" framing), this
    /// test fails.
    ///
    /// `metric_baseline` is probed via `child_status_in_context` with a
    /// canonical citing `MetricThreshold` signal — the kernel resolves it
    /// deterministically (D8) when a signal cites it, so it should NOT
    /// appear as a special LLM-fallback bullet in rule 1b.
    ///
    /// Closes Phase 1 of the I1 refactor: rule 1 lives in Rust, prompt is
    /// the consumer. Extended in T-8 for D8 coverage.
    #[test]
    fn prompt_rule_1_matches_rust_kernel_coverage() {
        use crate::librarian::tools::goal_aggregation::{
            child_status_in_context, child_status_pure, AcceptanceSignal, AcceptanceSignalSpec,
            ChildStatus, ThresholdOp,
        };
        use serde_json::json;

        // Each archetype × canonical params known to elicit a definite verdict.
        let probes = [
            (
                "failure_table",
                json!({"failures":[{"id":"F-1","status":"pass"}]}),
            ),
            ("task_list", json!({"tasks":[{"id":"T-1","status":"done"}]})),
            ("audit_issues", json!({"issues":[{"n":1,"status":"fixed"}]})),
            ("reflective", json!({"status":"decided"})),
            ("goal", json!({"status":"done"})),
            (
                "deployment_state",
                json!({"envs":{"prod":{"enabled":true}}}),
            ),
        ];

        let v = super::archetype_goal();
        let prompt = v.get("prompt_template").and_then(|p| p.as_str()).unwrap();
        // Everything before rule 2 — covers rules 1a/1b/1c and the gather framing.
        let rule_1_section = prompt
            .split("2. For each `acceptance_signals[i]")
            .next()
            .unwrap();

        for (arch, params) in &probes {
            let status = child_status_pure(arch, params);
            if status == ChildStatus::Unknown {
                // Archetype is NOT Rust-handled in this phase. Prompt MUST
                // mention it explicitly under rule 1b's LLM-fallback list.
                assert!(
                    rule_1_section.contains(arch),
                    "{arch} returns Unknown in kernel — prompt rule 1b must mention it \
                     for the LLM fallback path"
                );
            } else {
                // Archetype IS Rust-handled. Rule 1a's copy-verbatim clause
                // governs it. The prompt must reference the gather context key
                // so the LLM knows where to read ground truth.
                assert!(
                    rule_1_section.contains("deterministic_child_statuses"),
                    "{arch} resolved in kernel — prompt rule 1 must reference \
                     deterministic_child_statuses as the source of truth"
                );
            }
        }

        // D8 — metric_baseline must be Rust-handled when a metric_threshold
        // signal cites it. Probe via child_status_in_context with a canonical
        // signal; assert the verdict is non-Unknown AND the prompt does not
        // single it out as an LLM-only fallback case.
        let citing = vec![AcceptanceSignal {
            description: "metric_baseline contract".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "P@5".into(),
                op: ThresholdOp::Gte,
                threshold: 0.20,
            },
        }];
        let child_lookup = vec![(
            "C-M".to_string(),
            "metric_baseline".to_string(),
            json!({"current":{"P@5":0.21}}),
        )];
        let mb_status = child_status_in_context(
            "metric_baseline",
            "C-M",
            &json!({"current":{"P@5":0.21}}),
            &citing,
            &child_lookup,
        );
        assert_ne!(
            mb_status,
            ChildStatus::Unknown,
            "metric_baseline with citing signal must resolve deterministically (D8)"
        );
        assert!(
            !rule_1_section.contains("metric_baseline child →"),
            "metric_baseline is now Rust-handled when cited — prompt must not \
             carry the legacy per-archetype LLM-evaluation bullet for it"
        );
    }

    /// H-8 follow-through — goal-tracker prompt rule 3 must explicitly tell
    /// the LLM how to populate `progress_log[].evidence_commits` and
    /// `evidence_artifacts`. The Rust kernel provides `context.git_log` +
    /// `refresh_meta.last_refresh_at` + `refresh_meta.children_status_delta`;
    /// the prompt has to anchor the LLM to those names. Without this
    /// instruction the LLM leaves evidence_commits empty (the failure mode
    /// H-8 surfaced in the 2026-05-17 audit).
    #[test]
    fn goal_prompt_rule_3_anchors_evidence_fields_to_gather_context() {
        let v = super::archetype_goal();
        let prompt = v.get("prompt_template").and_then(|p| p.as_str()).unwrap();

        // Carve out rule 3 — between "3. `refresh_meta`" and the rule 4 header.
        let rule_3 = prompt
            .split("3. `refresh_meta`")
            .nth(1)
            .and_then(|s| s.split("4. AUTO-CLOSE GATE").next())
            .expect("rule 3 must be present and precede rule 4");

        // Evidence-commits anchoring — both the source (`context.git_log`) and
        // the date cutoff (`refresh_meta.last_refresh_at`) must be named so
        // the LLM knows where to look and how far back to scan.
        assert!(
            rule_3.contains("evidence_commits") && rule_3.contains("context.git_log"),
            "rule 3 must instruct the LLM to populate evidence_commits from context.git_log"
        );
        assert!(
            rule_3.contains("refresh_meta.last_refresh_at"),
            "rule 3 must anchor evidence_commits selection to refresh_meta.last_refresh_at"
        );

        // Evidence-artifacts anchoring — the source (`children_status_delta`)
        // and the lookup path (`params.children`) must be named so the LLM
        // can map child_id → artifact_id.
        assert!(
            rule_3.contains("evidence_artifacts")
                && rule_3.contains("children_status_delta"),
            "rule 3 must instruct the LLM to populate evidence_artifacts from children_status_delta"
        );

        // Regression guard — the old misleading claim about field ownership
        // must not return. `date`, `note`, `evidence_commits`, and
        // `evidence_artifacts` are ALL LLM-owned. Only `refresh_meta` is
        // Rust-owned.
        assert!(
            !rule_3.contains("`note` is the only LLM-owned field"),
            "rule 3 must not claim note is the only LLM-owned field — date/evidence_* are also LLM-owned"
        );
    }

    /// Regression for the `unwrap_or_default()` at the top of `call`, which made a type error
    /// on ONE argument silently discard EVERY argument and return success.
    ///
    /// The assertion that matters is the second one. Asserting only that the bad call errors
    /// would pass for a version that rejects everything; asserting only that the good call
    /// succeeds would pass for the buggy version, which succeeded at all times. **The pair is
    /// what discriminates**, and the buggy code fails the first half while passing the second
    /// — so this test dies with the old line and lives with the new one.
    #[tokio::test]
    async fn a_malformed_argument_is_refused_rather_than_silently_dropped() {
        let ctx = mk_ctx();

        let bad = call(&ctx, json!({"action": "tracker_design", "archetype": []})).await;
        let err = bad.expect_err(
            "an array where a string is declared must be refused — under the old \
             `unwrap_or_default()` this returned the archetype MENU with a 200, so a caller \
             who mistyped `archetype` was told they had asked for the menu",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("tracker_design"),
            "the refusal must name the tool it came from, got: {msg}"
        );

        // Negative control: a well-formed call still works, so the fix refuses type errors
        // rather than arguments in general. `action` is present here exactly as the
        // dispatcher passes it through, and `Args` carries no `deny_unknown_fields`, so the
        // extra key must remain harmless.
        let good = call(
            &ctx,
            json!({"action": "tracker_design", "archetype": "task_list"}),
        )
        .await;
        assert!(
            good.is_ok(),
            "a valid archetype must still resolve: {:?}",
            good.err()
        );
    }
}
