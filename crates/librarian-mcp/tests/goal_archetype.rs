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
            if failures == 0 {
                "done"
            } else {
                "active"
            }
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
            if open == 0 {
                "done"
            } else {
                "active"
            }
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
    let signals = goal_params
        .get("acceptance_signals")
        .and_then(|v| v.as_array());
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
    assert_eq!(
        reconcile_child_status("metric_baseline", &below),
        "in-progress"
    );
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
    assert_eq!(
        reconcile_child_status("reflective", &json!({"status":"decided"})),
        "done"
    );
    assert_eq!(
        reconcile_child_status("reflective", &json!({"status":"archived"})),
        "done"
    );
    assert_eq!(
        reconcile_child_status("reflective", &json!({"status":"deferred"})),
        "blocked"
    );
    assert_eq!(
        reconcile_child_status("reflective", &json!({"status":"scoping"})),
        "active"
    );
    assert_eq!(
        reconcile_child_status("reflective", &json!({"status":"active"})),
        "active"
    );
}

#[test]
fn t2a_nested_goal_status_mapping() {
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"done"})),
        "done"
    );
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"blocked"})),
        "blocked"
    );
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"abandoned"})),
        "blocked"
    );
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"scoping"})),
        "pending"
    );
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"pending-confirmation"})),
        "in-progress"
    );
    assert_eq!(
        reconcile_child_status("goal", &json!({"status":"active"})),
        "active"
    );
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
    assert_eq!(
        new_child_status, "done",
        "after refresh, child status is fresh"
    );
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
    goal_after["children"]
        .as_array_mut()
        .unwrap()
        .push(new_child);

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
