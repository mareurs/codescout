//! `tracker_design` — teaching tool. Returns a system_prompt + a library of
//! archetypes + the existing-tracker landscape so the agent can compose a
//! well-shaped tracker spec and call `artifact_create` with confidence.
//!
//! Archetype-driven (not intent-driven) for v1: server stays stateless, no
//! synthesis cost, transparent to the agent. Intent-driven tailoring is
//! deferred until archetype selection proves frustrating in practice.

use crate::librarian::catalog::{artifact, augmentation};
use crate::librarian::tools::ToolContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

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

pub fn archetypes() -> Value {
    json!([
        archetype_deployment_state(),
        archetype_failure_table(),
        archetype_metric_baseline(),
        archetype_audit_issues(),
        archetype_task_list(),
        archetype_reflective(),
        archetype_goal(),
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
        "prompt_template": "Maintain a goal-tracker. Your job is **aggregation**, not evaluation: reconcile each child's state into the goal's params using ground truth supplied by the refresh pipeline. Do not recompute children's evidence — trust the child's own params.\n\nINPUTS (gather):\n- This goal's current params.\n- `context.deterministic_child_statuses` — an array, one entry per linked child, of `{child_id, artifact_id, archetype, status, basis}`. `basis` is `\"deterministic\"` for archetypes the Rust kernel resolved, `\"needs parent context\"` for `metric_baseline` (you evaluate it via rule 1b), `\"unknown archetype\"` for any archetype the kernel doesn't know, `\"no augmentation\"` if the child has no augmentation row, or `\"child unreachable\"` if the artifact_id has no row in the catalog.\n- Optional: commit log scoped to paths the criterion names (gather_from: git_log).\n\nUPDATE RULES:\n\n1. Reconcile each `children[].status` from the child's actual status:\n\n   a. **First, copy ground truth from `context.deterministic_child_statuses`.** For every entry whose `basis == \"deterministic\"`, copy its `status` into `children[id].status` verbatim. Do not reinterpret — these archetypes are handled by the Rust kernel and its verdict is authoritative.\n\n   b. **For entries whose `basis != \"deterministic\"`** (currently `metric_baseline` only, plus any future archetype not yet in the Rust kernel), evaluate manually:\n      - metric_baseline child → \"done\" if `current` meets the related acceptance_signal's threshold; \"in-progress\" otherwise. See rule 2 for the threshold cross-reference.\n\n   c. **For entries with `basis == \"child unreachable\"`** → set `children[id].status = \"orphan\"`. Do NOT delete the row.\n\n2. For each `acceptance_signals[i]`, set `.met` by looking up the cited child's params and copying the relevant value forward. Do not re-derive the underlying metric — read it from the child's params verbatim and compare against the signal's description. Update the `evidence` string to cite the child id and the specific datum.\n\n3. Append exactly one entry to `progress_log` for this refresh cycle: {date: today, note: ≤200-char summary of what changed since previous log, evidence_commits: [commits added since last refresh that touched goal paths], evidence_artifacts: [child artifact_ids whose status changed]}. If nothing changed, append a \"no change\" entry — never skip the log.\n\n4. AUTO-CLOSE GATE (ALL conditions required):\n   a. len(children) >= 2 (per amendment D9 — single-child or empty goals should use the underlying archetype directly)\n   b. All `children[].status` == \"done\".\n   c. Every `acceptance_signals[].met` is true.\n   If all three: set status: \"done\", append a History entry to body summarizing the closing evidence. Otherwise: leave status unchanged.\n\n5. SCOPE GROWTH: if your aggregation surfaces a missing sub-objective, you MAY:\n   a. Call artifact(action=\"create\", kind=\"tracker\", augment={}) with the appropriate existing archetype (failure_table, task_list, metric_baseline, audit_issues, reflective, deployment_state, or nested goal).\n   b. Call artifact(action=\"link\", src_id=THIS_GOAL_ID, dst_id=NEW_CHILD_ID, rel=\"child\").\n   c. Add the new child to `children[]` with the next free C-N id.\n   d. Append one paragraph to body \"Decomposition rationale\" citing the trigger.\n\n6. NEVER:\n   - Delete a child row (use status=\"orphan\" if unreachable).\n   - Modify a child's params directly. The child has its own augmentation.\n   - Flip status to \"done\" without satisfying ALL gate conditions including 4a.\n   - Append more than one progress_log entry per refresh.\n\nSTOP CONDITION (you are done with this refresh when):\n- All children reconciled.\n- One progress_log entry appended.\n- Auto-close gate evaluated.\n- Output: the new params object. Body edits only for History append or Decomposition rationale append on scope growth.\n\nBody holds rationale and history; params hold mechanical state. Keep them separated."
    })
}

const SYSTEM_PROMPT: &str = r#"# How to design a tracker

A tracker is an artifact that mixes **live state** (params, refreshed often by gather sources) with **prose** (body, edited rarely by humans). The art of designing a good tracker is putting the right thing in the right place.

## Step 1 — Pick an archetype

Match the user's intent to one of the 7 archetypes. Use this decision sketch:

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
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).unwrap_or_default();
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
        let aug = augmentation::get(&cat, id)?;
        existing.push(json!({
            "id": id,
            "title": art.title,
            "kind": art.kind,
            "abs_path": art.abs_path.display().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{augmentation, Catalog};
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::workspace::WorkspaceConfig;
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
                abs_path: std::path::PathBuf::from("/test/x/y"),
                git_root: std::path::PathBuf::from("/test/x"),
                umbrella: None,
            })),
        }
    }

    #[tokio::test]
    async fn returns_design_envelope() {
        let ctx = mk_ctx();
        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["design_version"], "1");
        assert!(v["system_prompt"].as_str().unwrap().len() > 1000);
        assert_eq!(v["archetypes"].as_array().unwrap().len(), 7);
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
    #[tokio::test]
    async fn goal_archetype_present_and_registered() {
        let v = archetypes();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 7, "expected 7 archetypes including goal");
        let names: Vec<&str> = arr.iter().map(|a| a["name"].as_str().unwrap()).collect();
        assert!(
            names.contains(&"goal"),
            "goal archetype missing from archetypes() — got {names:?}"
        );
    }

    /// Drift tripwire — enumerates archetypes that `child_status_pure` returns
    /// non-`Unknown` for and asserts the prompt's rule 1 correctly delegates
    /// to Rust for exactly those. If a future edit adds an archetype to the
    /// kernel but forgets to collapse its prompt clause (or removes one from
    /// the kernel while leaving the "copy verbatim" framing), this test fails.
    ///
    /// Closes Phase 1 of the I1 refactor: rule 1 lives in Rust, prompt is the
    /// consumer.
    #[test]
    fn prompt_rule_1_matches_rust_kernel_coverage() {
        use crate::librarian::tools::goal_aggregation::{child_status_pure, ChildStatus};
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
            (
                "metric_baseline",
                json!({"baseline":{"P@5":0.18},"current":{"P@5":0.20}}),
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
    }
}
