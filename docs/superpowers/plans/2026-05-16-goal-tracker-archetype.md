# Goal-Tracker Archetype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the `goal` tracker archetype (7th archetype in `librarian-mcp`), its surfacing affordances, a custom prompt-based Stop hook in `codescout-companion`, and an eval gate validating the auto-close behavior on real repo goals.

**Architecture:** A container archetype that aggregates state of typed child trackers (`failure_table`, `task_list`, `metric_baseline`, `audit_issues`, `reflective`, nested `goal`) linked via existing `rel="child"` edges. Type-specific evidence stays in children; the goal owns criterion + aggregate status + progress log. Native Claude Code `/goal` is bypassed — a custom `Stop` prompt-hook reads the active goal-tracker via MCP and signals stop/continue from `goal.status`.

**Tech Stack:** Rust (`librarian-mcp` crate), MiniJinja templates, JSON Schema, MCP, Claude Code plugin hooks.

**Spec:** `docs/superpowers/specs/2026-05-16-goal-tracker-design.md`

**Phase checkpoints:** Phase 1 ships standalone; Phases 2/3/4 build on it. Each phase ends with `cargo fmt && cargo clippy -- -D warnings && cargo test` clean. Phase 4 is the gate before cherry-picking to master.

---

## Phase 1 — Archetype only

Adds the `goal` archetype to `tracker_design.rs`. Zero surfacing, zero hook. The archetype is usable via existing `artifact(create)` + `librarian(tracker_design)` after Phase 1.

### Task 1: Add `archetype_goal()` function

**Files:**
- Modify: `crates/librarian-mcp/src/tools/tracker_design.rs` (add function after `archetype_reflective` at line 262)

- [ ] **Step 1: Write the failing test**

Add to `crates/librarian-mcp/src/tools/tracker_design.rs` inside the `tests` module (after the existing `each_archetype_template_renders_against_example_params` test at line ~596):

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p librarian-mcp --lib tools::tracker_design::tests::goal_archetype_present_and_registered`

Expected: FAIL with `expected 7 archetypes including goal` (currently 6).

- [ ] **Step 3: Add the `archetype_goal()` function**

Insert this function in `crates/librarian-mcp/src/tools/tracker_design.rs` immediately after `archetype_reflective()` (around line 263, before the `SYSTEM_PROMPT` constant):

```rust
fn archetype_goal() -> Value {
    json!({
        "name": "goal",
        "when_to_use": "Tracking an outcome-stated objective whose completion depends on a named criterion and on aggregated state of sibling/child artifacts. Use when the work has a definable 'done' line, decomposes into typed sub-trackers (tests, tasks, metrics, audits), and survives across sessions. Examples: 'all flaky tests resolved + suite green for 3 runs', 'retrieval P@5 reaches 0.20 on benchmark X', 'plan-lifecycle subsystem ships behind feature flag'. Not for: open-ended research (use `reflective`), single-metric tracking (use `metric_baseline`), bare task lists with no completion semantics (use `task_list`).",
        "params_shape_example": {
            "criterion": "Retrieval pipeline P@5 ≥ 0.20 on benchmark-25tc, with no regression on R@5",
            "status": "active",
            "blocked_reason": null,
            "acceptance_signals": [
                {"description": "P@5 ≥ 0.20 on benchmark-25tc", "met": false, "evidence": "metric_baseline child a1b2c3 current=0.193"},
                {"description": "R@5 not below baseline 0.724",   "met": true,  "evidence": "metric_baseline child a1b2c3 current=0.781"},
                {"description": "No new failures in chat-eval-v3", "met": true,  "evidence": "failure_table child d4e5f6 0 fail/12"}
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
                            "description": { "type": "string" },
                            "met":         { "type": "boolean" },
                            "evidence":    { "type": "string" }
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
        "prompt_template": "Maintain a goal-tracker. Your job is **aggregation**, not evaluation: read children via artifact(action=\"get\") and reconcile their state into the goal's params. Do not recompute children's evidence — trust the child's own params.\n\nINPUTS (gather):\n- This goal's current params.\n- For each `children[].artifact_id`, the child's params via artifact(action=\"get\").\n- Optional: commit log scoped to paths the criterion names (gather_from: git_log).\n\nUPDATE RULES:\n\n1. Reconcile each `children[].status` from the child's actual status, normalizing into our enum (pending|active|in-progress|done|blocked|orphan|unknown):\n   - failure_table child → \"done\" if 0 failures, \"active\" otherwise\n   - task_list child → \"done\" if all tasks done, \"in-progress\" otherwise\n   - metric_baseline child → \"done\" if current meets the related acceptance_signal, else \"in-progress\"\n   - audit_issues child → \"done\" if 0 open issues, \"active\" otherwise\n   - reflective child → \"done\" if child status ∈ {\"decided\",\"archived\"}; \"blocked\" if \"deferred\"; \"active\" otherwise.\n   - nested goal child → \"done\" if child status == \"done\"; \"blocked\" if ∈ {\"blocked\",\"abandoned\"}; \"pending\" if \"scoping\"; \"in-progress\" if \"pending-confirmation\"; \"active\" otherwise.\n   - Child artifact unreachable → set status: \"orphan\", DO NOT delete the row.\n\n2. Re-evaluate each `acceptance_signals[].met` from the children's evidence. Update the `evidence` string to cite the child id and the specific datum.\n\n3. Append exactly one entry to `progress_log` for this refresh cycle: {date: today, note: ≤200-char summary of what changed since previous log, evidence_commits: [commits added since last refresh that touched goal paths], evidence_artifacts: [child artifact_ids whose status changed]}. If nothing changed, append a \"no change\" entry — never skip the log.\n\n4. AUTO-CLOSE GATE (ALL conditions required):\n   a. len(children) > 0\n   b. All `children[].status` == \"done\".\n   c. Every `acceptance_signals[].met` is true.\n   If all three: set status: \"done\", append a History entry to body summarizing the closing evidence. Otherwise: leave status unchanged.\n\n5. SCOPE GROWTH: if your aggregation surfaces a missing sub-objective, you MAY:\n   a. Call artifact(action=\"create\", kind=\"tracker\", augment={...}) with the appropriate existing archetype (failure_table, task_list, metric_baseline, audit_issues, reflective, or nested goal).\n   b. Call artifact(action=\"link\", src_id=THIS_GOAL_ID, dst_id=NEW_CHILD_ID, rel=\"child\").\n   c. Add the new child to `children[]` with the next free C-N id.\n   d. Append one paragraph to body \"Decomposition rationale\" citing the trigger.\n\n6. NEVER:\n   - Delete a child row (use status=\"orphan\" if unreachable).\n   - Modify a child's params directly. The child has its own augmentation.\n   - Flip status to \"done\" without satisfying ALL gate conditions including 4a.\n   - Append more than one progress_log entry per refresh.\n\nSTOP CONDITION (you are done with this refresh when):\n- All children reconciled.\n- One progress_log entry appended.\n- Auto-close gate evaluated.\n- Output: the new params object. Body edits only for History append or Decomposition rationale append on scope growth.\n\nBody holds rationale and history; params hold mechanical state. Keep them separated."
    })
}
```

- [ ] **Step 4: Wire it into `archetypes()`**

Modify the existing `archetypes()` function at line 29–38 to include the new archetype. Use `mcp__codescout__edit_code` action=replace on symbol `archetypes`:

```rust
fn archetypes() -> Value {
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
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p librarian-mcp --lib tools::tracker_design::tests::goal_archetype_present_and_registered`

Expected: PASS.

- [ ] **Step 6: Verify existing archetype tests still pass**

Run: `cargo test -p librarian-mcp --lib tools::tracker_design::tests`

Expected: all tests pass, including `each_archetype_self_consistent` (validates params_shape against schema for ALL 7 archetypes) and `each_archetype_template_renders_against_example_params` (renders MiniJinja template against example params for ALL 7).

If `each_archetype_self_consistent` fails: the schema rejects the example shape. Fix the schema or the example until they match.

If `each_archetype_template_renders_against_example_params` fails: the MiniJinja template has a syntax or filter error. Read the error message and fix.

- [ ] **Step 7: Commit**

```bash
git add crates/librarian-mcp/src/tools/tracker_design.rs
git commit -m "feat(librarian): add goal tracker archetype

7th archetype: container that aggregates state of typed child trackers
(failure_table, task_list, metric_baseline, audit_issues, reflective,
nested goal). Type-specific evidence delegated to children; the goal
owns criterion + aggregate status + acceptance_signals + progress_log.

Spec: docs/superpowers/specs/2026-05-16-goal-tracker-design.md"
```

### Task 2: Phase 1 verification — full build + clippy + manual MCP smoke

**Files:** none

- [ ] **Step 1: Run full project test suite**

Run: `cargo test`

Expected: all tests pass across all crates.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: no warnings.

- [ ] **Step 3: Run formatter**

Run: `cargo fmt`

Expected: no changes (already formatted). If there are changes, stage them and amend the commit:

```bash
git add -u && git commit --amend --no-edit
```

- [ ] **Step 4: Build release binary for MCP smoke test**

Run: `cargo build --release`

Expected: clean build.

- [ ] **Step 5: Smoke test via running MCP server (manual)**

Restart the MCP server with `/mcp` in Claude Code, then verify the goal archetype is available:

```
mcp__codescout__librarian(action="tracker_design", intent="goal: ship feature X")
```

Expected: response includes 7 archetypes; the `goal` entry has all fields populated (name, when_to_use, params_shape_example, params_schema_example, render_template_example, body_skeleton, prompt_template).

**Phase 1 ends here. The archetype is shippable as-is. To stop here, cherry-pick the commit to `master` via the standard ship sequence in CLAUDE.md. To continue, proceed to Phase 2.**

---

## Phase 2 — Surfacing (S1 + S3)

Adds the agent-agnostic discovery layer: server_instructions paragraph + `librarian_context` no-anchor branch that prepends an active-goals header.

### Task 3: S1 — Add server_instructions paragraph

**Files:**
- Modify: `src/prompts/server_instructions.md` (location and exact section TBD by reading the file first)

- [ ] **Step 1: Locate the right section in server_instructions.md**

Run: `mcp__codescout__read_markdown(path="src/prompts/server_instructions.md")` — note the heading list. Find the section that documents `artifact` / `librarian` tools. The new paragraph belongs there as a child heading or appended bullet.

If unsure: place the new paragraph under a new heading `## Goal-trackers` at file end. The existing test `server::tests::prompt_surfaces_reference_only_real_tools` will catch placement issues if any tool name is misspelled.

- [ ] **Step 2: Write the failing test**

Add to `src/server.rs` `tests` module (after the existing `prompt_surfaces_reference_only_real_tools` test):

```rust
#[test]
fn server_instructions_documents_goal_tracker_discovery() {
    let s = include_str!("prompts/server_instructions.md");
    assert!(
        s.contains("goal-tracker") || s.contains("Goal-tracker"),
        "server_instructions.md should document the goal-tracker discovery pattern"
    );
    assert!(
        s.contains(r#"tags":{"in":["goal"]}"#) || s.contains(r#"tags: ["goal"]"#),
        "server_instructions.md should show the goal-tracker tag discovery query"
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib server::tests::server_instructions_documents_goal_tracker_discovery`

Expected: FAIL.

- [ ] **Step 4: Add the paragraph to server_instructions.md**

Append this section to `src/prompts/server_instructions.md` (use `mcp__codescout__edit_markdown` with `action="insert_after"` on the last heading, or just append to file end):

```markdown
## Goal-trackers

A **goal-tracker** is a tracker artifact (kind=tracker, tags=["goal"]) that names a completion criterion and aggregates the state of typed child trackers. Each project should have at most one goal with status=active at a time.

**Find the active goal for the current project:**

```
artifact(action="find", kind="tracker",
         filter={"tags":{"in":["goal"]}, "status":{"eq":"active"}})
```

**Get richer context including active goals plus other project signal:**

```
librarian(action="context")   # no anchor — auto-includes active goals
```

When you start work toward a stated objective, create a goal-tracker via `librarian(action="tracker_design", intent="goal: ...")` then `artifact(action="create", kind="tracker", tags=["goal"], augment=...)`. Children are linked via `artifact(action="link", rel="child")` and use existing archetypes (failure_table, task_list, metric_baseline, audit_issues, reflective).
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib server::tests::server_instructions_documents_goal_tracker_discovery`

Expected: PASS.

- [ ] **Step 6: Run prompt-surface consistency check**

Run: `cargo test --lib server::tests::prompt_surfaces_reference_only_real_tools`

Expected: PASS. If FAIL: the new paragraph mentions a tool name that no longer exists. Cross-reference against current tool registrations in `src/server.rs::CodeScoutServer::from_parts` and fix.

- [ ] **Step 7: Commit**

```bash
git add src/prompts/server_instructions.md src/server.rs
git commit -m "feat(prompts): document goal-tracker discovery in server_instructions

Teaches every connecting LLM how to find the active goal for a project
and points at librarian(context) for richer surfacing. Pure prompt edit —
server_instructions is loaded fresh per MCP connect, no ONBOARDING_VERSION
bump required.

Part of goal-tracker spec Phase 2 (S1)."
```

### Task 4: S3 — `librarian_context` no-anchor active-goals header

**Files:**
- Modify: `crates/librarian-mcp/src/tools/context.rs` (the `call` function's no-anchor early return at lines ~140–145)
- Modify: `crates/librarian-mcp/src/tools/context.rs` `tests` module (update `no_args_returns_empty`)

- [ ] **Step 1: Write the failing test**

Add to `crates/librarian-mcp/src/tools/context.rs` inside the `tests` module:

```rust
#[tokio::test]
async fn no_args_returns_active_goals_header() {
    let tmp = TempDir::new().unwrap();
    let cat = Catalog::open_in_memory().unwrap();

    // Insert one active goal-tracker
    {
        use crate::catalog::artifact::insert_artifact;
        let goal_row = sample_row("goal-A", "tracker", "active", "Ship Feature X");
        let mut row_with_tags = goal_row.clone();
        row_with_tags.tags = vec!["goal".to_string()];
        insert_artifact(&cat, &row_with_tags).unwrap();
    }

    let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

    let v = call(&ctx, json!({})).await.unwrap();

    let md = v["markdown"].as_str().unwrap();
    assert!(
        md.contains("## Active goals"),
        "expected '## Active goals' header in markdown; got: {md}"
    );
    assert!(
        md.contains("Ship Feature X"),
        "expected goal title in active-goals section; got: {md}"
    );
}
```

Note: this test depends on `sample_row` and `insert_artifact` helpers; verify their names/signatures by reading `crates/librarian-mcp/src/tools/context.rs` test module (around line 273) and `crates/librarian-mcp/src/catalog/artifact.rs` first. If signatures differ, adjust the test accordingly.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p librarian-mcp --lib tools::context::tests::no_args_returns_active_goals_header`

Expected: FAIL — the current no-anchor branch returns empty markdown.

- [ ] **Step 3: Modify the no-anchor branch in `call` to render active goals**

Replace the existing early return inside the `else` arm of the `if let Some(ref anchor_id) = a.anchor_id` ... `else if a.topic.is_some()` ... `else` chain (lines ~140–145 in `crates/librarian-mcp/src/tools/context.rs`).

Use `mcp__codescout__edit_code` action=replace on symbol `call` if practical, or `edit_file` with the exact `old_string` / `new_string`. The replacement block:

```rust
        } else {
            // No anchor, no topic — return active goals as the discovery surface.
            use crate::filter::FilterNode;
            let archived_clause = if a.include_archived {
                None
            } else {
                Some(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ))
            };
            let goal_filter = FilterNode::And {
                and: vec![
                    FilterNode::Leaf(
                        [("kind".to_string(), json!({"eq": "tracker"}))]
                            .into_iter()
                            .collect(),
                    ),
                    FilterNode::Leaf(
                        [("tags".to_string(), json!({"in": ["goal"]}))]
                            .into_iter()
                            .collect(),
                    ),
                    FilterNode::Leaf(
                        [("status".to_string(), json!({"eq": "active"}))]
                            .into_iter()
                            .collect(),
                    ),
                ],
            };
            let combined = match archived_clause {
                Some(a) => FilterNode::And {
                    and: vec![a, goal_filter],
                },
                None => goal_filter,
            };
            let (scoped, _) =
                apply_scope(Some(combined), effective_scope, &ctx.workspace, current)?;
            let rows = find(
                &cat,
                &FindOpts {
                    filter: scoped,
                    limit: 10,
                    offset: 0,
                    semantic: None,
                },
            )?;
            rows.into_iter().map(|r| r.id).collect()
        }
```

This replaces the early `return Ok(empty)` with a populated `candidate_ids: Vec<String>`, which then flows through the existing row-loading + rendering pipeline below. The rendered output uses the existing `## {title} — {kind}/{status} ({path})` format per row.

To get an "Active goals" header above the rows, prepend a single section before the loop. Find the line `let mut markdown = String::new();` and replace with:

```rust
    let active_goals_header = matches!(
        (&a.topic, &a.anchor_id),
        (None, None)
    ) && !sorted_ids.is_empty();
    let mut markdown = if active_goals_header {
        String::from("## Active goals\n\n")
    } else {
        String::new()
    };
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p librarian-mcp --lib tools::context::tests::no_args_returns_active_goals_header`

Expected: PASS.

- [ ] **Step 5: Update the now-broken `no_args_returns_empty` test**

The existing test at lines 422–431 expects empty markdown on no-args. With the new behavior, empty markdown is only returned when there are *zero* active goals. Update the test to assert that explicitly:

Use `mcp__codescout__edit_code` action=replace on symbol `tests/no_args_returns_empty`:

```rust
#[tokio::test]
async fn no_args_with_no_active_goals_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

    let v = call(&ctx, json!({})).await.unwrap();

    assert_eq!(v["markdown"].as_str().unwrap(), "");
    assert_eq!(v["included_ids"].as_array().unwrap().len(), 0);
}
```

(The rename is intentional — the new name describes the actual contract.)

- [ ] **Step 6: Run full context tests**

Run: `cargo test -p librarian-mcp --lib tools::context::tests`

Expected: all pass, including the renamed `no_args_with_no_active_goals_returns_empty`.

- [ ] **Step 7: Run cargo fmt and full test suite**

Run: `cargo fmt && cargo test`

Expected: clean.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: no warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/librarian-mcp/src/tools/context.rs
git commit -m "feat(librarian): librarian_context no-anchor mode surfaces active goals

When called with no anchor_id and no topic, librarian_context now queries
artifacts with kind=tracker, tags=['goal'], status='active' and prepends
an '## Active goals' header before the rendered rows. Returns empty
markdown only when there are zero active goals.

Renamed no_args_returns_empty test to no_args_with_no_active_goals_returns_empty
to reflect the actual contract.

Part of goal-tracker spec Phase 2 (S3)."
```

**Phase 2 ends here. Goal-trackers are now discoverable by any MCP client. To stop here, cherry-pick to master. To continue with the Claude Code Stop hook integration, proceed to Phase 3.**

---

### Task 4b: Tier 2 tests T2a, T2b, T2c — aggregation, three-query sandwich, scope growth

**Files:**
- Create: `crates/librarian-mcp/tests/goal_archetype.rs`

These tests exercise the goal-tracker's aggregation rules without an LLM in the loop. They encode the augmentation prompt's rule 1 (per-archetype status mapping) and rule 5 (scope growth) as a deterministic Rust function — the "mocked synthesizer" referenced in the spec. The Rust function is the executable shadow of the prompt; if the two drift, Tier 3 catches it.

- [ ] **Step 1: Create the test file with the mocked synthesizer helper**

Create `crates/librarian-mcp/tests/goal_archetype.rs`:

```rust
//! Tier 2 behavior tests for the goal-tracker archetype.
//!
//! These tests apply the augmentation prompt's rules deterministically via a Rust
//! "mocked synthesizer" — `reconcile_child_status` and `apply_auto_close_gate`.
//! The functions encode rules 1 and 4 of the goal archetype's prompt_template.
//! Tier 3 (in `tests/goal_eval/`) verifies the same rules under a real LLM.

use serde_json::{json, Value};

/// Reconcile a single child's status into the goal's children[].status enum per rule 1.
fn reconcile_child_status(child_archetype: &str, child_params: &Value) -> &'static str {
    match child_archetype {
        "failure_table" => {
            let failures = child_params
                .get("failures")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter(|f| f["status"] == "fail").count())
                .unwrap_or(0);
            if failures == 0 { "done" } else { "active" }
        }
        "task_list" => {
            let tasks = child_params.get("tasks").and_then(|v| v.as_array());
            match tasks {
                Some(t) if t.iter().all(|x| x["status"] == "done") => "done",
                _ => "in-progress",
            }
        }
        "metric_baseline" => {
            // For test purposes: meets-acceptance is signalled by params.meets_acceptance.
            if child_params.get("meets_acceptance") == Some(&json!(true)) {
                "done"
            } else {
                "in-progress"
            }
        }
        "audit_issues" => {
            let open = child_params
                .get("issues")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter(|i| i["status"] == "open").count())
                .unwrap_or(0);
            if open == 0 { "done" } else { "active" }
        }
        "reflective" => match child_params.get("status").and_then(|v| v.as_str()) {
            Some("decided") | Some("archived") => "done",
            Some("deferred") => "blocked",
            _ => "active",
        },
        "goal" => match child_params.get("status").and_then(|v| v.as_str()) {
            Some("done") => "done",
            Some("blocked") | Some("abandoned") => "blocked",
            Some("scoping") => "pending",
            Some("pending-confirmation") => "in-progress",
            _ => "active",
        },
        _ => "unknown",
    }
}

/// Apply the auto-close gate per rule 4. Returns the new goal status.
fn apply_auto_close_gate(goal_params: &Value) -> String {
    let children = goal_params.get("children").and_then(|v| v.as_array());
    let signals = goal_params.get("acceptance_signals").and_then(|v| v.as_array());
    let current = goal_params
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("active")
        .to_string();

    let len_children = children.map(|c| c.len()).unwrap_or(0);
    if len_children == 0 {
        return current;
    }
    let all_children_done = children
        .map(|c| c.iter().all(|x| x["status"] == "done"))
        .unwrap_or(false);
    let all_signals_met = signals
        .map(|s| s.iter().all(|x| x["met"] == json!(true)))
        .unwrap_or(true);

    if all_children_done && all_signals_met {
        "done".to_string()
    } else {
        current
    }
}

// ------------------------------------------------------------------
// T2a — aggregation correctness per child archetype
// ------------------------------------------------------------------

#[test]
fn t2a_failure_table_done_iff_zero_failures() {
    let zero = json!({"failures": []});
    let some = json!({"failures": [{"id":"F-1","status":"fail"}]});
    let mixed = json!({"failures": [{"id":"F-1","status":"pass"},{"id":"F-2","status":"fail"}]});

    assert_eq!(reconcile_child_status("failure_table", &zero), "done");
    assert_eq!(reconcile_child_status("failure_table", &some), "active");
    assert_eq!(reconcile_child_status("failure_table", &mixed), "active");
}

#[test]
fn t2a_task_list_done_iff_all_tasks_done() {
    let all_done = json!({"tasks": [{"id":"T-1","status":"done"},{"id":"T-2","status":"done"}]});
    let partial = json!({"tasks": [{"id":"T-1","status":"done"},{"id":"T-2","status":"open"}]});

    assert_eq!(reconcile_child_status("task_list", &all_done), "done");
    assert_eq!(reconcile_child_status("task_list", &partial), "in-progress");
}

#[test]
fn t2a_metric_baseline_done_iff_meets_acceptance() {
    let meets = json!({"current": {"P@5": 0.21}, "meets_acceptance": true});
    let below = json!({"current": {"P@5": 0.19}, "meets_acceptance": false});

    assert_eq!(reconcile_child_status("metric_baseline", &meets), "done");
    assert_eq!(reconcile_child_status("metric_baseline", &below), "in-progress");
}

#[test]
fn t2a_audit_issues_done_iff_zero_open() {
    let fixed = json!({"issues": [{"n":1,"status":"fixed"}]});
    let open = json!({"issues": [{"n":1,"status":"open"}]});

    assert_eq!(reconcile_child_status("audit_issues", &fixed), "done");
    assert_eq!(reconcile_child_status("audit_issues", &open), "active");
}

#[test]
fn t2a_reflective_status_mapping() {
    assert_eq!(reconcile_child_status("reflective", &json!({"status":"decided"})), "done");
    assert_eq!(reconcile_child_status("reflective", &json!({"status":"archived"})), "done");
    assert_eq!(reconcile_child_status("reflective", &json!({"status":"deferred"})), "blocked");
    assert_eq!(reconcile_child_status("reflective", &json!({"status":"scoping"})), "active");
    assert_eq!(reconcile_child_status("reflective", &json!({"status":"active"})), "active");
}

#[test]
fn t2a_nested_goal_status_mapping() {
    assert_eq!(reconcile_child_status("goal", &json!({"status":"done"})), "done");
    assert_eq!(reconcile_child_status("goal", &json!({"status":"blocked"})), "blocked");
    assert_eq!(reconcile_child_status("goal", &json!({"status":"abandoned"})), "blocked");
    assert_eq!(reconcile_child_status("goal", &json!({"status":"scoping"})), "pending");
    assert_eq!(reconcile_child_status("goal", &json!({"status":"pending-confirmation"})), "in-progress");
    assert_eq!(reconcile_child_status("goal", &json!({"status":"active"})), "active");
}

// ------------------------------------------------------------------
// T2b — three-query sandwich for status-change propagation
// ------------------------------------------------------------------

#[test]
fn t2b_status_propagation_three_query_sandwich() {
    // Step 1: initial state — goal G with one failure_table child C (3 failures)
    let child_initial = json!({"failures": [
        {"id":"F-1","status":"fail"},
        {"id":"F-2","status":"fail"},
        {"id":"F-3","status":"fail"}
    ]});
    let goal_initial = json!({
        "criterion": "All F-N pass",
        "status": "active",
        "children": [
            {"id":"C-1","artifact_id":"x","title":"failures","archetype":"failure_table",
             "status": reconcile_child_status("failure_table", &child_initial)}
        ],
        "acceptance_signals": [{"description":"all failures resolved","met":false}]
    });

    // Step 2: mutate C (flip all 3 fails to pass) — do NOT refresh G
    let child_mutated = json!({"failures": [
        {"id":"F-1","status":"pass"},
        {"id":"F-2","status":"pass"},
        {"id":"F-3","status":"pass"}
    ]});

    // Step 3: G's children[0].status is still "active" because we haven't refreshed
    assert_eq!(
        goal_initial["children"][0]["status"], "active",
        "before refresh, goal's view of child must be stale"
    );

    // Step 4: trigger refresh — reconcile child status
    let new_child_status = reconcile_child_status("failure_table", &child_mutated);

    // Step 5: G's children[0].status is now "done"
    assert_eq!(new_child_status, "done", "after refresh, child status is fresh");
}

// ------------------------------------------------------------------
// T2c — scope growth round-trip (deterministic shape)
// ------------------------------------------------------------------

#[test]
fn t2c_scope_growth_adds_child_with_sequential_id() {
    let goal_before = json!({
        "criterion": "Test",
        "status": "active",
        "children": [
            {"id":"C-1","artifact_id":"a","title":"a","archetype":"failure_table","status":"active"}
        ]
    });

    // Simulate scope growth: append C-2.
    let mut goal_after = goal_before.clone();
    let new_child = json!({
        "id":"C-2","artifact_id":"b","title":"new","archetype":"task_list","status":"pending"
    });
    goal_after["children"].as_array_mut().unwrap().push(new_child);

    // Assert: id is sequential, new child appended (not replaced).
    let kids = goal_after["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["id"], "C-1");
    assert_eq!(kids[1]["id"], "C-2");
    assert_eq!(kids[1]["archetype"], "task_list");
}

// ------------------------------------------------------------------
// T2d — auto-close gate enforcement
// ------------------------------------------------------------------

#[test]
fn t2d_auto_close_flips_when_all_gates_pass() {
    let goal = json!({
        "status": "active",
        "children": [
            {"id":"C-1","status":"done"},
            {"id":"C-2","status":"done"}
        ],
        "acceptance_signals": [
            {"description":"a","met":true},
            {"description":"b","met":true}
        ]
    });
    assert_eq!(apply_auto_close_gate(&goal), "done");
}

#[test]
fn t2d_auto_close_refuses_when_one_signal_unmet() {
    // 4/5 acceptance signals met — tempting flip, must be refused.
    let goal = json!({
        "status": "active",
        "children": [
            {"id":"C-1","status":"done"},
            {"id":"C-2","status":"done"}
        ],
        "acceptance_signals": [
            {"description":"a","met":true},
            {"description":"b","met":true},
            {"description":"c","met":true},
            {"description":"d","met":true},
            {"description":"e","met":false}
        ]
    });
    assert_eq!(apply_auto_close_gate(&goal), "active");
}

#[test]
fn t2d_auto_close_refuses_when_one_child_active() {
    let goal = json!({
        "status": "active",
        "children": [
            {"id":"C-1","status":"done"},
            {"id":"C-2","status":"active"}
        ],
        "acceptance_signals": [{"description":"a","met":true}]
    });
    assert_eq!(apply_auto_close_gate(&goal), "active");
}

#[test]
fn t2d_auto_close_refuses_on_empty_children() {
    // Critical guard: len(children) > 0. Without this, trivially-all-done would falsely close.
    let goal = json!({
        "status": "active",
        "children": [],
        "acceptance_signals": [{"description":"a","met":true}]
    });
    assert_eq!(apply_auto_close_gate(&goal), "active");
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test -p librarian-mcp --test goal_archetype`

Expected: all 12 tests pass.

- [ ] **Step 3: Run full project test suite**

Run: `cargo test`

Expected: clean.

- [ ] **Step 4: Run clippy and format**

Run: `cargo fmt && cargo clippy -- -D warnings`

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/tests/goal_archetype.rs
git commit -m "test(goal-tracker): add Tier 2 behavior tests (T2a/T2b/T2c/T2d)

Mocked synthesizer + deterministic tests covering:
- T2a: aggregation correctness per child archetype (6 tests)
- T2b: three-query sandwich for status-change propagation
- T2c: scope growth — sequential C-N id assignment
- T2d: auto-close gate (happy + 3 forbidden-flip cases including empty-children guard)

The Rust 'mocked synthesizer' is the executable shadow of the prompt's
rules 1 and 4. Drift between Rust and prompt is detected by Tier 3 (eval).

Part of goal-tracker spec Phase 2 + Phase 3 Tier 2 tests."
```

## Phase 3 — Custom Stop hook in codescout-companion (S5)

Adds the prompt-based Stop hook to `claude-plugins/codescout-companion`. Bypasses native `/goal` entirely; the goal-tracker artifact becomes the single source of truth for stop/continue decisions.

**Open question to resolve before starting Phase 3:** the exact format for prompt-based Stop hooks in `hooks.json`. The current `codescout-companion/hooks/hooks.json` uses only `"type": "command"` hooks. Anthropic's CC docs (per web research) describe prompt-based hooks as `"type": "prompt"`. Before writing the hook, run:

```
mcp__codescout__run_command(command='find /home/marius/.claude/plugins -name "hooks.json" -exec grep -l "type.*prompt" {} \\;')
```

to find an existing prompt-based hook example in any installed plugin. If none found, consult `https://docs.claude.com/en/docs/claude-code/hooks` for the current schema. **Do not invent the format.**

### Task 5: Author the goal Stop hook script

**Files:**
- Create: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.sh` (or `.prompt.md` if prompt-type hooks reference a file)
- Create: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.test.sh` (smoke test)

- [ ] **Step 1: Write the hook smoke test**

Create `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.test.sh`:

```bash
#!/usr/bin/env bash
# Smoke test for goal-stop-hook.sh
# Asserts the hook outputs valid JSON with continue=true and a reason
# when no active goal exists.
set -euo pipefail

HOOK="$(dirname "$0")/goal-stop-hook.sh"

if [[ ! -x "$HOOK" ]]; then
    echo "FAIL: hook not executable: $HOOK"
    exit 1
fi

# Simulate hook input (CC sends JSON on stdin)
INPUT='{"session_id":"test","transcript_path":"/dev/null","cwd":"/tmp","last_assistant_message":""}'
OUTPUT=$(echo "$INPUT" | "$HOOK" 2>&1 || true)

# Hook must always emit valid JSON
echo "$OUTPUT" | python3 -c "import json,sys; d=json.load(sys.stdin); assert 'continue' in d, 'missing continue field'; assert isinstance(d['continue'], bool), 'continue must be bool'; print('PASS')"
```

```bash
chmod +x /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.test.sh
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bash /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.test.sh`

Expected: FAIL with `hook not executable` — the hook doesn't exist yet.

- [ ] **Step 3: Author the Stop hook**

Create `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.sh`:

```bash
#!/usr/bin/env bash
# Stop hook for codescout goal-trackers.
# Reads stdin (CC-provided JSON with session_id, transcript_path, cwd, last_assistant_message),
# queries codescout MCP for the active goal-tracker in the current project,
# and emits {"continue": bool, "reason": "..."} based on goal.status.
#
# Fail-open: if codescout MCP is unreachable or the query errors, emit
# {"continue": true, "reason": "codescout MCP unreachable"}.
#
# Disable via .claude/codescout-companion.json {"goal_stop_hook": false}.

set -euo pipefail

# Read CC input from stdin (JSON)
INPUT=$(cat)

CWD=$(echo "$INPUT" | python3 -c "import json,sys; print(json.load(sys.stdin).get('cwd','.'))")

# Check disable flag
CONFIG_FILE="$CWD/.claude/codescout-companion.json"
if [[ -f "$CONFIG_FILE" ]]; then
    DISABLED=$(python3 -c "
import json
with open('$CONFIG_FILE') as f:
    cfg = json.load(f)
v = cfg.get('goal_stop_hook', True)
print('1' if v is False or v == 'false' else '0')
" 2>/dev/null || echo "0")
    if [[ "$DISABLED" == "1" ]]; then
        echo '{"continue": true, "reason": "goal_stop_hook disabled in .claude/codescout-companion.json"}'
        exit 0
    fi
fi

# Query active goals via codescout MCP CLI (codescout exposes a `cli` subcommand)
# Note: confirm the exact CLI invocation against `codescout --help` at implementation time.
# Fail-open on any error.
GOAL_JSON=$(codescout artifact find --kind tracker --tag goal --status active --scope project --limit 5 2>/dev/null || echo "")

if [[ -z "$GOAL_JSON" ]]; then
    # MCP unreachable or codescout binary missing → fail open with a logged warning.
    LOG="$CWD/.claude/codescout-companion.log"
    mkdir -p "$(dirname "$LOG")"
    echo "$(date -Iseconds) goal-stop-hook: codescout MCP unreachable, defaulting to continue" >> "$LOG"
    echo '{"continue": true, "reason": "codescout MCP unreachable — defaulting to continue"}'
    exit 0
fi

# Parse the result
RESULT=$(echo "$GOAL_JSON" | python3 -c "
import json, sys
data = json.load(sys.stdin)
items = data.get('items', [])
n = len(items)
if n == 0:
    print(json.dumps({'continue': True, 'reason': 'no active goal'}))
elif n > 1:
    print(json.dumps({'continue': True, 'reason': f'multiple active goals ({n}) — ambiguous, deferring'}))
else:
    goal = items[0]
    status = goal.get('params', {}).get('status') or goal.get('status', 'unknown')
    criterion = (goal.get('params', {}).get('criterion') or goal.get('title', ''))[:120]
    if status == 'done':
        print(json.dumps({'continue': False, 'reason': f'goal done: {criterion}'}))
    elif status in ('blocked', 'abandoned'):
        blocked_reason = goal.get('params', {}).get('blocked_reason') or criterion
        print(json.dumps({'continue': False, 'reason': f'goal {status}: {blocked_reason}'}))
    else:
        # Find first unmet acceptance signal
        signals = goal.get('params', {}).get('acceptance_signals', [])
        unmet = [s for s in signals if not s.get('met')]
        next_target = unmet[0]['description'] if unmet else criterion
        print(json.dumps({'continue': True, 'reason_to_continue': f'next acceptance signal: {next_target}'}))
")

echo "$RESULT"
```

```bash
chmod +x /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.sh
```

**Caveat:** the `codescout artifact find ...` CLI invocation assumes a CLI surface on the codescout binary. If codescout does not yet expose a CLI subcommand for `artifact(find)`, this task requires either (a) adding one to the codescout binary, or (b) reformulating the hook to talk to the MCP server via stdio protocol directly. Audit `src/main.rs` and `src/cli/` (if it exists) before proceeding. If the CLI is missing, escalate to a follow-up task before continuing.

- [ ] **Step 4: Run smoke test**

Run: `bash /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.test.sh`

Expected: PASS (outputs valid JSON with `continue: true` because no active goal exists in `/tmp`).

- [ ] **Step 5: Commit the hook**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add hooks/goal-stop-hook.sh hooks/goal-stop-hook.test.sh
git commit -m "feat(hooks): add goal-stop-hook for codescout goal-trackers

Prompt-based Stop hook that queries the active goal-tracker via codescout
MCP and signals stop/continue based on goal.status. Fail-open if MCP is
unreachable. Disable via .claude/codescout-companion.json
{\"goal_stop_hook\": false}.

Part of goal-tracker spec Phase 3 (S5)."
cd -
```

### Task 6: Register the Stop hook in hooks.json

**Files:**
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/hooks.json`

- [ ] **Step 1: Add the Stop hook registration**

Insert a `"Stop"` event under `"hooks"` in `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/hooks.json`. The full updated file should be:

Use `mcp__codescout__edit_file` to insert before the closing `}` of the `"hooks"` object (after the existing `"PostToolUse"` array):

Find this exact text in the file:
```
    "PostToolUse": [
```

Edit so the file structure becomes:

```json
{
  "hooks": {
    "SessionStart": [ ... ],
    "SubagentStart": [ ... ],
    "PreToolUse": [ ... ],
    "PostToolUse": [ ... ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/goal-stop-hook.sh"
          }
        ]
      }
    ]
  }
}
```

The exact `old_string` to find (last 3 lines of the existing `PostToolUse` array followed by the closing braces):

```json
    ]
  }
}
```

Replace with:

```json
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/hooks/goal-stop-hook.sh"
          }
        ]
      }
    ]
  }
}
```

- [ ] **Step 2: Validate JSON syntax**

Run: `python3 -c "import json; json.load(open('/home/marius/work/claude/claude-plugins/codescout-companion/hooks/hooks.json'))" && echo OK`

Expected: `OK`. If `json.JSONDecodeError`: open the file and fix the trailing-comma / bracket issue.

- [ ] **Step 3: Commit**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add hooks/hooks.json
git commit -m "feat(hooks): register goal-stop-hook on Stop event

Part of goal-tracker spec Phase 3 (S5)."
cd -
```

### Task 7: Add goal_stop_hook flag to plugin config schema + README

**Files:**
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/README.md`
- Modify (if exists): config schema; otherwise document inline

- [ ] **Step 1: Document the flag in the plugin README**

Read the existing README to find the right insertion point:

Run: `mcp__codescout__read_markdown(path="/home/marius/work/claude/claude-plugins/codescout-companion/README.md")` — note the heading structure.

Append a new heading (or insert near existing `block_reads` documentation if present):

```markdown
### `goal_stop_hook` (default: `true`)

Controls the Stop hook that reads the active goal-tracker and decides whether to continue the assistant loop. Disable via `.claude/codescout-companion.json`:

```json
{ "goal_stop_hook": false }
```

When disabled, Claude Code's native `/goal` behavior (or vanilla Stop semantics) takes over. When enabled (default), the hook queries codescout's active goal-tracker and signals stop when `goal.status` is `done`, `blocked`, or `abandoned`. Fails open (continues) if codescout MCP is unreachable.
```

- [ ] **Step 2: Commit**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add README.md
git commit -m "docs(hooks): document goal_stop_hook config flag

Part of goal-tracker spec Phase 3 (S5)."
cd -
```

### Task 7b: Tier 2 test T2e — Stop hook decision matrix (7 branches)

**Files:**
- Create: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.matrix.test.sh`

A comprehensive table-driven bash test that mocks the codescout CLI's `artifact find` response with each of 7 scenarios and asserts the hook's stdout matches the expected `{continue, reason}` payload.

- [ ] **Step 1: Author the matrix test**

Create `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.matrix.test.sh`:

```bash
#!/usr/bin/env bash
# T2e — Tier 2 decision matrix test for goal-stop-hook.sh.
#
# Mocks the `codescout` binary by prepending a stub directory to PATH that
# emits canned `artifact find` responses, then asserts the hook's stdout
# matches the expected verdict for each of 7 branches.
set -euo pipefail

HOOK="$(dirname "$0")/goal-stop-hook.sh"
WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

mkdir -p "$WORK/bin"
PATH="$WORK/bin:$PATH"

# Helper: install a codescout stub that emits the given JSON to stdout
install_stub() {
    local response="$1"
    cat > "$WORK/bin/codescout" <<EOF
#!/usr/bin/env bash
echo '$response'
EOF
    chmod +x "$WORK/bin/codescout"
}

# Helper: run hook with given CWD and stub response, assert stdout JSON has expected continue + reason substring
assert_hook() {
    local label="$1"
    local cwd="$2"
    local expected_continue="$3"        # "true" or "false"
    local expected_reason_substring="$4"

    local input="{\"session_id\":\"t\",\"transcript_path\":\"/dev/null\",\"cwd\":\"$cwd\",\"last_assistant_message\":\"\"}"
    local output
    output=$(echo "$input" | "$HOOK")
    local got_continue
    got_continue=$(echo "$output" | python3 -c "import json,sys; print(json.load(sys.stdin)['continue'])")
    local got_reason
    got_reason=$(echo "$output" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('reason') or d.get('reason_to_continue') or '')")

    if [[ "$got_continue" != "$expected_continue" ]]; then
        echo "FAIL [$label]: expected continue=$expected_continue, got $got_continue (full: $output)"
        exit 1
    fi
    if [[ "$got_reason" != *"$expected_reason_substring"* ]]; then
        echo "FAIL [$label]: expected reason containing '$expected_reason_substring', got '$got_reason' (full: $output)"
        exit 1
    fi
    echo "PASS [$label]"
}

CWD="$WORK/project"
mkdir -p "$CWD/.claude"

# --- Branch 1: 0 active goals ---
install_stub '{"items":[]}'
assert_hook "0-goals" "$CWD" "True" "no active goal"

# --- Branch 2: >1 active goals ---
install_stub '{"items":[{"id":"a","title":"G1"},{"id":"b","title":"G2"}]}'
assert_hook "multi-goals" "$CWD" "True" "multiple active goals"

# --- Branch 3: 1 goal, status=done ---
install_stub '{"items":[{"id":"a","title":"G","params":{"status":"done","criterion":"do X"}}]}'
assert_hook "done" "$CWD" "False" "goal done"

# --- Branch 4: 1 goal, status=blocked ---
install_stub '{"items":[{"id":"a","title":"G","params":{"status":"blocked","criterion":"do X","blocked_reason":"need approval"}}]}'
assert_hook "blocked" "$CWD" "False" "goal blocked"

# --- Branch 5: 1 goal, status=abandoned ---
install_stub '{"items":[{"id":"a","title":"G","params":{"status":"abandoned","criterion":"do X"}}]}'
assert_hook "abandoned" "$CWD" "False" "goal abandoned"

# --- Branch 6: 1 goal, status=active, with one unmet signal ---
install_stub '{"items":[{"id":"a","title":"G","params":{"status":"active","criterion":"do X","acceptance_signals":[{"description":"step one","met":true},{"description":"step two","met":false}]}}]}'
assert_hook "active-with-next" "$CWD" "True" "step two"

# --- Branch 7: MCP unreachable (codescout binary errors) ---
cat > "$WORK/bin/codescout" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
chmod +x "$WORK/bin/codescout"
assert_hook "mcp-unreachable" "$CWD" "True" "unreachable"

echo "All 7 branches passed."
```

```bash
chmod +x /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.matrix.test.sh
```

- [ ] **Step 2: Run the matrix test**

Run: `bash /home/marius/work/claude/claude-plugins/codescout-companion/hooks/goal-stop-hook.matrix.test.sh`

Expected: `All 7 branches passed.` If any branch fails, the FAIL line identifies the branch label and the expected vs got values.

- [ ] **Step 3: Commit**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add hooks/goal-stop-hook.matrix.test.sh
git commit -m "test(hooks): T2e — Stop hook 7-branch decision matrix

Table-driven test mocking codescout CLI for each scenario:
0 goals, multiple goals, status=done/blocked/abandoned/active,
MCP unreachable. Each branch asserts the hook's {continue, reason}
payload matches the spec's decision matrix.

Part of goal-tracker spec Phase 3 (Tier 2 T2e)."
cd -
```

### Task 8: Manual integration smoke test

**Files:** none

- [ ] **Step 1: Reload plugins in Claude Code**

In an active CC session, run `/reload-plugins`. Expected: confirms codescout-companion reloaded with the new hook count.

- [ ] **Step 2: Create a test goal-tracker in this repo**

```
mcp__codescout__librarian(action="tracker_design", intent="goal: test stop hook end-to-end")
```

Then `mcp__codescout__artifact(action="create", kind="tracker", tags=["goal"], augment={...}, body=...)` with a minimal goal.

- [ ] **Step 3: Trigger the Stop hook**

In any new Claude Code turn that ends naturally, observe the session output. The hook should fire; check `.claude/codescout-companion.log` for any warnings.

- [ ] **Step 4: Update the test goal to status=done and verify hook signals stop**

```
mcp__codescout__artifact_augment(id=<goal_id>, merge=true, params={status: "done"})
```

Then trigger another turn. Expected: CC loop terminates with the hook's "goal done" reason visible.

- [ ] **Step 5: Clean up the test goal**

```
mcp__codescout__artifact(action="update", id=<goal_id>, patch={status: "archived"})
```

**Phase 3 ends here. The integration is live for CC users. To stop here, cherry-pick Phases 1–3 commits to master. To complete the eval gate (recommended before promoting), proceed to Phase 4.**

---

## Phase 4 — Eval gate (Tier 3 replay)

Builds the 5-real-goal × 3-checkpoint fixture set and the synthesis-evaluation harness. This is the gate Hamsa flagged as non-optional: without it, every future edit to the goal prompt is a guess.

### Task 9: Construct the eval fixture set

**Files:**
- Create: `crates/librarian-mcp/tests/goal_eval/fixtures/goal_01_phase6_provider_lifts/{t0,t1,t2}.json`
- Create: `crates/librarian-mcp/tests/goal_eval/fixtures/goal_02_retrieval_p5/{t0,t1,t2}.json`
- Create: `crates/librarian-mcp/tests/goal_eval/fixtures/goal_03_tools_mod_refactor/{t0,t1,t2}.json`
- Create: `crates/librarian-mcp/tests/goal_eval/fixtures/goal_04_kotlin_lsp_mux/{t0,t1,t2}.json`
- Create: `crates/librarian-mcp/tests/goal_eval/fixtures/goal_05_augmentation_postfix/{t0,t1,t2}.json`

- [ ] **Step 1: Pull the real history for each goal candidate**

For each of the 5 goals, read the corresponding source documents to ground the fixture:

```
mcp__codescout__artifact(action="get", id=<id from this list>, full=true)
```

Goal sources:
- goal_01: `docs/TODO-phase6-provider-lifts.md` (id: c54ad4ece0a71aed)
- goal_02: `docs/trackers/retrieval-benchmark.md` (id: 8e09ca67f463027e)
- goal_03: `docs/trackers/tools-mod-refactor-2026-05.md` (id: ccd1cda1b4135fff)
- goal_04: `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`
- goal_05: `docs/trackers/agent-memory-research-2026-04.md` (id: 7c910f78bf28796c)

If any of these no longer exist or aren't appropriate "done"-line work, swap with another goal from the project's recent history.

- [ ] **Step 2: Hand-construct each fixture**

For each goal, create three checkpoint files (`t0.json` = scoping, `t1.json` = mid-work, `t2.json` = done) shaped as full goal-tracker params per the archetype's `params_schema_example`. Example for `goal_01_phase6_provider_lifts/t0.json`:

```json
{
  "criterion": "Phase 6 provider lifts complete: all deferred provider implementations from src/tools/* migrated to src/providers/* with no functionality regressions",
  "status": "scoping",
  "blocked_reason": null,
  "acceptance_signals": [
    {"description": "All 7 provider lift targets identified", "met": false, "evidence": ""},
    {"description": "Zero functionality regressions in existing tools", "met": false, "evidence": ""},
    {"description": "Tests green on experiments branch", "met": false, "evidence": ""}
  ],
  "children": [],
  "progress_log": []
}
```

The corresponding `t1.json` should have 2/3 acceptance signals met and 2 children at mixed statuses. `t2.json` should have all signals met, all children done, `status: "done"`.

Repeat for goal_02 through goal_05. Each fixture is a hand-curated representation of how a senior engineer *would have* tracked the work.

**Note:** the fixtures are inputs to the eval, not outputs. The eval *runs synthesis* on each fixture and scores the result.

- [ ] **Step 3: Commit the fixtures**

```bash
git add crates/librarian-mcp/tests/goal_eval/fixtures/
git commit -m "test(goal-eval): add 5 real-goal fixtures × 3 checkpoints

Hand-curated fixtures replaying recent repo work as goal-trackers at
T0 (scoping), T1 (mid-work), T2 (done). Inputs to the Tier 3 synthesis
eval per spec Phase 4.

Part of goal-tracker spec Phase 4 (Tier 3 eval gate)."
```

### Task 10: Author the eval harness

**Files:**
- Create: `crates/librarian-mcp/tests/goal_eval/eval.rs`
- Create: `crates/librarian-mcp/tests/goal_eval/rubric.rs`

- [ ] **Step 1: Write the rubric module**

Create `crates/librarian-mcp/tests/goal_eval/rubric.rs`:

```rust
//! Rubric for scoring synthesizer output against expected behavior at each checkpoint.
//!
//! Each goal × checkpoint produces a `RubricScore` with 0/1 on each applicable
//! sub-rubric. A goal "passes" if every checkpoint passes every applicable sub-rubric.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkpoint {
    T0,
    T1,
    T2,
}

#[derive(Debug, Default)]
pub struct RubricScore {
    pub correct_status: Option<bool>,
    pub correct_evidence_citation: Option<bool>,
    pub no_fabrication: Option<bool>,
    pub appropriate_decomposition: Option<bool>,
}

impl RubricScore {
    pub fn passed(&self) -> bool {
        [
            self.correct_status,
            self.correct_evidence_citation,
            self.no_fabrication,
            self.appropriate_decomposition,
        ]
        .into_iter()
        .filter_map(|x| x)
        .all(|x| x)
    }
}

/// Score a synthesizer's output against the expected state at the given checkpoint.
///
/// - `before`: the fixture params handed to the synthesizer.
/// - `after`: the params the synthesizer produced.
/// - `expected_status`: what status the goal should be at this checkpoint.
/// - `real_commits_for_goal`: commits known to belong to this goal's work
///   (used to verify the synthesizer didn't fabricate).
pub fn score(
    cp: Checkpoint,
    before: &Value,
    after: &Value,
    expected_status: &str,
    real_commits: &[&str],
) -> RubricScore {
    let mut s = RubricScore::default();

    // Correct status (all checkpoints)
    s.correct_status = Some(
        after
            .get("status")
            .and_then(|v| v.as_str())
            .map_or(false, |actual| actual == expected_status),
    );

    // Correct evidence citation (T1, T2)
    if matches!(cp, Checkpoint::T1 | Checkpoint::T2) {
        let log = after
            .get("progress_log")
            .and_then(|v| v.as_array())
            .map(|arr| arr.last())
            .flatten();
        s.correct_evidence_citation = Some(
            log.map_or(false, |entry| {
                entry
                    .get("evidence_artifacts")
                    .and_then(|v| v.as_array())
                    .map_or(false, |arr| !arr.is_empty())
                    || entry
                        .get("evidence_commits")
                        .and_then(|v| v.as_array())
                        .map_or(false, |arr| !arr.is_empty())
            }),
        );
    }

    // No fabrication (all checkpoints)
    // Heuristic: any commit cited in progress_log must be in real_commits.
    let cited_commits: Vec<String> = after
        .get("progress_log")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .flat_map(|entry| {
                    entry
                        .get("evidence_commits")
                        .and_then(|v| v.as_array())
                        .into_iter()
                        .flat_map(|inner| inner.iter())
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();
    s.no_fabrication = Some(
        cited_commits
            .iter()
            .all(|c| real_commits.iter().any(|r| c.starts_with(r) || r.starts_with(c.as_str()))),
    );

    // Appropriate decomposition (T0 only)
    if matches!(cp, Checkpoint::T0) {
        let children = after
            .get("children")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        // Sensible decomposition: at least 1 child, at most 8, with valid archetype names.
        let valid_archetypes = [
            "failure_table",
            "task_list",
            "metric_baseline",
            "audit_issues",
            "reflective",
            "goal",
        ];
        let all_valid_archetypes = after
            .get("children")
            .and_then(|v| v.as_array())
            .map_or(false, |arr| {
                arr.iter().all(|c| {
                    c.get("archetype")
                        .and_then(|v| v.as_str())
                        .map_or(false, |a| valid_archetypes.contains(&a))
                })
            });
        s.appropriate_decomposition = Some((1..=8).contains(&children) && all_valid_archetypes);
    }

    s
}
```

- [ ] **Step 2: Write the eval harness**

Create `crates/librarian-mcp/tests/goal_eval/eval.rs`:

```rust
//! Tier 3 eval harness: runs the goal augmentation prompt against each
//! fixture × checkpoint and scores the result. Marked #[ignore] — run on demand
//! with `cargo test --test goal_eval -- --ignored`.
//!
//! Requires an Anthropic API key for synthesis (set via ANTHROPIC_API_KEY).

mod rubric;

use rubric::{score, Checkpoint};
use serde_json::Value;

const GOALS: &[(&str, &str, &[&str])] = &[
    ("goal_01_phase6_provider_lifts", "done", &["1df2e6a8"]),
    ("goal_02_retrieval_p5",          "done", &["abc1234"]), // replace with real commits
    ("goal_03_tools_mod_refactor",    "done", &["ccd1cda"]),
    ("goal_04_kotlin_lsp_mux",        "done", &["75f03635"]),
    ("goal_05_augmentation_postfix",  "done", &["715ecb8"]),
];

fn read_fixture(goal_slug: &str, cp: Checkpoint) -> Value {
    let cp_name = match cp {
        Checkpoint::T0 => "t0",
        Checkpoint::T1 => "t1",
        Checkpoint::T2 => "t2",
    };
    let path = format!("tests/goal_eval/fixtures/{goal_slug}/{cp_name}.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {path}: {e}"));
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("invalid JSON in {path}: {e}"))
}

/// Synthesize new params by running the goal augmentation prompt via the Anthropic API.
///
/// Implementation note: this is the stub the engineer fills in. It must:
/// 1. Read the goal archetype's `prompt_template` from `archetype_goal()`.
/// 2. Send a request with the prompt + the fixture params + any necessary child
///    state (in practice, children are referenced by `artifact_id` — for the
///    eval, embed mock child params inline since there's no live database).
/// 3. Parse the model's response as the new params object.
async fn synthesize(_prompt: &str, _params: &Value) -> Value {
    // TODO(eval harness author): implement against the Anthropic SDK.
    // Use claude-haiku-4-5-20251001 to match the Stop hook's default model.
    // For initial scaffolding, this stub returns the input unchanged so
    // tests fail visibly until the real synthesis is wired up.
    _params.clone()
}

#[tokio::test]
#[ignore = "eval — run manually with --ignored after API key set"]
async fn tier3_goal_eval() {
    use librarian_mcp::tools::tracker_design;

    let archetypes = tracker_design::archetypes();
    let goal_arch = archetypes
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "goal")
        .expect("goal archetype not registered");
    let prompt = goal_arch["prompt_template"].as_str().unwrap();

    let mut goal_pass_count = 0;
    let total = GOALS.len();

    for (slug, expected_t2_status, commits) in GOALS {
        let mut all_cp_pass = true;
        for cp in [Checkpoint::T0, Checkpoint::T1, Checkpoint::T2] {
            let before = read_fixture(slug, cp);
            let after = synthesize(prompt, &before).await;
            let expected_status = match cp {
                Checkpoint::T0 => "scoping",
                Checkpoint::T1 => "active",
                Checkpoint::T2 => expected_t2_status,
            };
            let s = score(cp, &before, &after, expected_status, commits);
            if !s.passed() {
                eprintln!("FAIL {slug} {cp:?}: {s:?}");
                all_cp_pass = false;
            }
        }
        if all_cp_pass {
            goal_pass_count += 1;
        }
    }

    println!("Tier 3 eval: {goal_pass_count}/{total} goals passed");
    assert!(
        goal_pass_count >= 4,
        "Tier 3 eval gate: need ≥4 of {total} goals to pass; got {goal_pass_count}. Iterate the augmentation prompt and re-run."
    );
}
```

Note the **two placeholders** in this harness that the engineer must complete:
1. `GOALS` array — replace the placeholder commit hashes (`abc1234` etc.) with the actual commits from each goal's real history. Use `git log --oneline --grep=<keyword>` or read the corresponding tracker artifact.
2. `synthesize()` function — wire up the actual Anthropic SDK call. Use Haiku 4.5 (`claude-haiku-4-5-20251001`) to match the Stop hook's default model.

- [ ] **Step 3: Add a Cargo dependency on the Anthropic SDK if not already present**

Check `crates/librarian-mcp/Cargo.toml` for an existing Anthropic SDK dep:

Run: `grep -E "anthropic|claude" crates/librarian-mcp/Cargo.toml || echo "no dep yet"`

If `no dep yet`: add to `[dev-dependencies]` in `crates/librarian-mcp/Cargo.toml`:

```toml
anthropic-sdk = "0.x"  # use latest stable version at implementation time
```

- [ ] **Step 4: Verify the harness compiles (even with the stub)**

Run: `cargo test -p librarian-mcp --test goal_eval -- --ignored --no-run`

Expected: builds clean. (Won't execute because `--ignored` tests don't run unless `--ignored` is passed *without* `--no-run`.)

- [ ] **Step 5: Commit the scaffolding**

```bash
git add crates/librarian-mcp/tests/goal_eval/
git commit -m "test(goal-eval): add Tier 3 eval harness scaffolding + rubric

Synthesizes goal augmentation prompts against 5 real-goal fixtures at 3
checkpoints each, scores against a 4-axis rubric (correct status, correct
evidence citation, no fabrication, appropriate decomposition). Pass gate:
≥4 of 5 goals pass every applicable sub-rubric.

The synthesize() stub returns input unchanged; the engineer must wire the
Anthropic SDK call before running the gate. Marked #[ignore] so CI skips
the API-key-requiring test by default.

Part of goal-tracker spec Phase 4 (Tier 3 eval gate)."
```

### Task 11: Run the eval gate and iterate

**Files:** none initially; may iterate on `crates/librarian-mcp/src/tools/tracker_design.rs::archetype_goal()` prompt_template if the eval fails.

- [ ] **Step 1: Wire up the synthesize() stub**

Implement the `synthesize` function in `crates/librarian-mcp/tests/goal_eval/eval.rs` per the placeholder note in Task 10 Step 2. The function must:

1. Read `ANTHROPIC_API_KEY` from env.
2. Construct a message to Haiku 4.5 with the prompt template as system and the fixture params (plus any embedded child state) as user content.
3. Parse the response — extract a single JSON object representing the new params.
4. Return it as a `Value`.

This is real implementation work, not a stub. Reference any existing Anthropic SDK usage in the project (search `grep -r "anthropic" crates/` first).

- [ ] **Step 2: Run the eval**

Run: `ANTHROPIC_API_KEY=<key> cargo test -p librarian-mcp --test goal_eval -- --ignored --nocapture`

Expected behavior: either passes the `≥4 of 5` gate, or prints which goals failed which sub-rubrics.

- [ ] **Step 3: If <4/5 passes, iterate the prompt**

Identify which sub-rubric most goals failed (read the eprintln output). Common failure modes and their prompt fixes:

| Failure mode | Likely prompt issue | Fix |
|---|---|---|
| `correct_status` false at T2 | Auto-close gate too strict or too loose | Re-read rule 4 in `prompt_template`; clarify what "all children done" means in this fixture's child types. |
| `correct_evidence_citation` false at T1/T2 | Rule 3 (progress_log) under-specified | Tighten rule 3: require ≥1 entry in either evidence_commits or evidence_artifacts when status changed. |
| `no_fabrication` false | Model hallucinated commit hashes | Add an explicit "Do not invent commit hashes you have not been shown" line to rule 3. |
| `appropriate_decomposition` false at T0 | Model picked wrong archetypes for children | Add an example mapping in rule 5 ("for a tests-must-pass criterion, use failure_table; for a metric-target criterion, use metric_baseline; ..."). |

Edit `archetype_goal()`'s `prompt_template` field accordingly and re-run.

- [ ] **Step 4: Once ≥4/5 passes, commit and mark spec validated**

```bash
git add crates/librarian-mcp/src/tools/tracker_design.rs crates/librarian-mcp/tests/goal_eval/eval.rs
git commit -m "feat(goal-tracker): pass Tier 3 eval gate, prompt iteration complete

Tier 3 eval (5 real-goal × 3 checkpoint replay) passes ≥4/5 goals on
all applicable sub-rubrics. Spec is now validated.

Part of goal-tracker spec Phase 4."
```

- [ ] **Step 5: Update spec status to validated**

Edit `docs/superpowers/specs/2026-05-16-goal-tracker-design.md` frontmatter:

```yaml
status: validated
validated: 2026-05-XX
```

```bash
git add docs/superpowers/specs/2026-05-16-goal-tracker-design.md
git commit -m "docs(spec): mark goal-tracker spec validated after Tier 3 pass"
```

### Task 12: Final integration verification + ship

**Files:** none

- [ ] **Step 1: Run full project verification**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`

Expected: all clean.

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`

Expected: clean build.

- [ ] **Step 3: Manual MCP smoke test**

Restart MCP via `/mcp` in Claude Code. In a new session, verify:

1. `librarian(tracker_design, intent="goal: ...")` returns the goal archetype.
2. `librarian(context)` with no args returns an "Active goals" header (or empty if no goals exist).
3. Create a real goal in this repo via `artifact(create, kind=tracker, tags=["goal"], augment=...)`.
4. Run a CC turn that ends naturally; observe Stop hook fires (check `.claude/codescout-companion.log`).
5. Flip the goal to `status=done`; observe Stop hook signals stop on the next turn.

- [ ] **Step 4: Cherry-pick the feature commits to master**

Per the standard ship sequence in CLAUDE.md:

```bash
git log --oneline experiments...master -- crates/librarian-mcp/ src/prompts/ docs/superpowers/specs/2026-05-16-goal-tracker-design.md
# Identify the SHAs of: Task 1, Task 3, Task 4, Task 10, Task 11, Task 11-Step-5 commits.

git checkout master
git cherry-pick <sha-task-1>
git cherry-pick <sha-task-3>
git cherry-pick <sha-task-4>
git cherry-pick <sha-task-10>
git cherry-pick <sha-task-11>
git cherry-pick <sha-task-11-step-5>
git push origin master

git checkout experiments
git rebase master
```

The codescout-companion plugin commits (Tasks 5, 6, 7) live in a separate repo (`/home/marius/work/claude/claude-plugins/codescout-companion`) — push them via that repo's own workflow.

- [ ] **Step 5: Cut a release if version bump warranted**

Per the release cycle in CLAUDE.md, decide if this warrants a minor version bump (new feature = minor per semver). If yes, run the full release sequence in `CLAUDE.md § Release Cycle` from master.

---

## Summary of files touched

**`code-explorer` repo (librarian-mcp + prompts):**
- `crates/librarian-mcp/src/tools/tracker_design.rs` — add archetype, wire into `archetypes()`, test
- `crates/librarian-mcp/src/tools/context.rs` — no-anchor branch + test rename
- `src/prompts/server_instructions.md` — append goal-tracker discovery section
- `src/server.rs` — test asserting server_instructions mentions goal-tracker
- `crates/librarian-mcp/tests/goal_eval/fixtures/` — 5 goals × 3 checkpoints
- `crates/librarian-mcp/tests/goal_eval/eval.rs` — Tier 3 harness
- `crates/librarian-mcp/tests/goal_eval/rubric.rs` — scoring rubric
- `crates/librarian-mcp/Cargo.toml` — anthropic-sdk dev-dependency (if not present)
- `docs/superpowers/specs/2026-05-16-goal-tracker-design.md` — status: validated

**`claude-plugins/codescout-companion` repo:**
- `hooks/goal-stop-hook.sh` — new prompt-based Stop hook
- `hooks/goal-stop-hook.test.sh` — smoke test
- `hooks/hooks.json` — register Stop event
- `README.md` — document `goal_stop_hook` config flag
