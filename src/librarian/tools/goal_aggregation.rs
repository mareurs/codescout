//! Deterministic predicates for goal-tracker child status derivation.
//!
//! Implements amendment D1–D3 from
//! `docs/superpowers/specs/2026-05-17-goal-tracker-amendment.md`.
//!
//! `child_status_pure` is a **pure function of (archetype, child_params)**.
//! Archetypes whose status depends on parent context (`metric_baseline`)
//! return `ChildStatus::Unknown` here — those go through
//! `goal_aggregation::contextual::*` in a later phase.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChildStatus {
    Pending,
    Active,
    InProgress,
    Done,
    Blocked,
    Orphan,
    Unknown,
}

impl ChildStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ChildStatus::Pending => "pending",
            ChildStatus::Active => "active",
            ChildStatus::InProgress => "in-progress",
            ChildStatus::Done => "done",
            ChildStatus::Blocked => "blocked",
            ChildStatus::Orphan => "orphan",
            ChildStatus::Unknown => "unknown",
        }
    }
}

/// Map a child's archetype + params to a normalized `ChildStatus`.
/// Returns `ChildStatus::Unknown` for archetypes that need parent context
/// (currently `metric_baseline`) or for unrecognized archetypes (H-5).
pub fn child_status_pure(archetype: &str, child_params: &Value) -> ChildStatus {
    match archetype {
        "failure_table" => failure_table_status(child_params),
        "task_list" => task_list_status(child_params),
        "audit_issues" => audit_issues_status(child_params),
        "reflective" => reflective_status(child_params),
        "goal" => nested_goal_status(child_params),
        "deployment_state" => deployment_state_status(child_params),
        // metric_baseline lives in `contextual` — needs parent signals.
        "metric_baseline" => ChildStatus::Unknown,
        _ => ChildStatus::Unknown, // H-5: unrecognized archetype
    }
}

// --- per-archetype predicates ---

fn failure_table_status(p: &Value) -> ChildStatus {
    match p.get("failures").and_then(|v| v.as_array()) {
        None => ChildStatus::Unknown,
        Some(f) if f.is_empty() => ChildStatus::Pending, // D1-style empty handling
        Some(f)
            if f.iter().all(|e| {
                matches!(
                    e.get("status").and_then(|s| s.as_str()),
                    Some("pass") | Some("wontfix")
                )
            }) =>
        {
            ChildStatus::Done
        } // D2: pass|wontfix → done
        Some(_) => ChildStatus::Active,                  // any fail or flaky blocks (D2)
    }
}

fn task_list_status(p: &Value) -> ChildStatus {
    match p.get("tasks").and_then(|v| v.as_array()) {
        None => ChildStatus::Unknown,
        Some(t) if t.is_empty() => ChildStatus::Pending, // D1
        Some(t)
            if t.iter()
                .all(|task| task.get("status").and_then(|s| s.as_str()) == Some("done")) =>
        {
            ChildStatus::Done
        }
        Some(_) => ChildStatus::InProgress,
    }
}

fn audit_issues_status(p: &Value) -> ChildStatus {
    match p.get("issues").and_then(|v| v.as_array()) {
        None => ChildStatus::Unknown,
        Some(i) if i.is_empty() => ChildStatus::Pending,
        Some(i)
            if i.iter()
                .all(|e| e.get("status").and_then(|s| s.as_str()) != Some("open")) =>
        {
            ChildStatus::Done
        }
        Some(_) => ChildStatus::Active,
    }
}

fn reflective_status(p: &Value) -> ChildStatus {
    match p.get("status").and_then(|v| v.as_str()) {
        Some("decided") | Some("archived") => ChildStatus::Done,
        Some("deferred") => ChildStatus::Blocked,
        Some(_) => ChildStatus::Active,
        None => ChildStatus::Active, // schema-permissive default
    }
}

fn nested_goal_status(p: &Value) -> ChildStatus {
    match p.get("status").and_then(|v| v.as_str()) {
        Some("done") => ChildStatus::Done,
        Some("blocked") | Some("abandoned") => ChildStatus::Blocked,
        Some("scoping") => ChildStatus::Pending,
        Some("pending-confirmation") => ChildStatus::InProgress,
        Some(_) | None => ChildStatus::Active,
    }
}

fn deployment_state_status(p: &Value) -> ChildStatus {
    match p.get("envs").and_then(|v| v.as_object()) {
        None => ChildStatus::Unknown,
        Some(e) if e.is_empty() => ChildStatus::Pending, // D3
        Some(e)
            if e.values()
                .all(|env| env.get("enabled").and_then(|b| b.as_bool()) == Some(true)) =>
        {
            ChildStatus::Done
        }
        Some(_) => ChildStatus::InProgress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- failure_table D2 ---

    #[test]
    fn failure_table_done_when_all_pass() {
        let p = json!({"failures":[
            {"id":"F-1","status":"pass"},
            {"id":"F-2","status":"pass"}
        ]});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Done);
    }

    #[test]
    fn failure_table_done_when_pass_plus_wontfix() {
        let p = json!({"failures":[
            {"id":"F-1","status":"pass"},
            {"id":"F-2","status":"wontfix"}
        ]});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Done);
    }

    #[test]
    fn failure_table_active_when_any_flaky() {
        let p = json!({"failures":[
            {"id":"F-1","status":"pass"},
            {"id":"F-2","status":"flaky"}
        ]});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Active);
    }

    #[test]
    fn failure_table_active_when_any_fail() {
        let p = json!({"failures":[
            {"id":"F-1","status":"fail"}
        ]});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Active);
    }

    #[test]
    fn failure_table_pending_when_empty() {
        let p = json!({"failures":[]});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Pending);
    }

    #[test]
    fn failure_table_unknown_when_no_failures_key() {
        let p = json!({});
        assert_eq!(child_status_pure("failure_table", &p), ChildStatus::Unknown);
    }

    // --- task_list D1 ---

    #[test]
    fn task_list_pending_when_empty() {
        let p = json!({"tasks":[]});
        assert_eq!(child_status_pure("task_list", &p), ChildStatus::Pending);
    }

    #[test]
    fn task_list_done_when_all_done() {
        let p = json!({"tasks":[{"id":"T-1","status":"done"}]});
        assert_eq!(child_status_pure("task_list", &p), ChildStatus::Done);
    }

    #[test]
    fn task_list_in_progress_when_mixed() {
        let p = json!({"tasks":[
            {"id":"T-1","status":"done"},
            {"id":"T-2","status":"pending"}
        ]});
        assert_eq!(child_status_pure("task_list", &p), ChildStatus::InProgress);
    }

    // --- audit_issues ---

    #[test]
    fn audit_issues_done_when_no_open() {
        let p = json!({"issues":[
            {"n":1,"status":"fixed"},
            {"n":2,"status":"wontfix"}
        ]});
        assert_eq!(child_status_pure("audit_issues", &p), ChildStatus::Done);
    }

    #[test]
    fn audit_issues_active_when_any_open() {
        let p = json!({"issues":[
            {"n":1,"status":"open"}
        ]});
        assert_eq!(child_status_pure("audit_issues", &p), ChildStatus::Active);
    }

    // --- reflective ---

    #[test]
    fn reflective_done_when_decided_or_archived() {
        assert_eq!(
            child_status_pure("reflective", &json!({"status":"decided"})),
            ChildStatus::Done
        );
        assert_eq!(
            child_status_pure("reflective", &json!({"status":"archived"})),
            ChildStatus::Done
        );
    }

    #[test]
    fn reflective_blocked_when_deferred() {
        assert_eq!(
            child_status_pure("reflective", &json!({"status":"deferred"})),
            ChildStatus::Blocked
        );
    }

    // --- nested goal ---

    #[test]
    fn nested_goal_status_round_trip() {
        for (s, expected) in [
            ("done", ChildStatus::Done),
            ("blocked", ChildStatus::Blocked),
            ("abandoned", ChildStatus::Blocked),
            ("scoping", ChildStatus::Pending),
            ("pending-confirmation", ChildStatus::InProgress),
            ("active", ChildStatus::Active),
        ] {
            assert_eq!(
                child_status_pure("goal", &json!({"status": s})),
                expected,
                "nested goal status={s}"
            );
        }
    }

    // --- deployment_state D3 ---

    #[test]
    fn deployment_state_done_all_enabled() {
        let p = json!({"envs":{
            "dev":  {"enabled": true},
            "prod": {"enabled": true}
        }});
        assert_eq!(child_status_pure("deployment_state", &p), ChildStatus::Done);
    }

    #[test]
    fn deployment_state_in_progress_partial() {
        let p = json!({"envs":{
            "dev":  {"enabled": true},
            "prod": {"enabled": false}
        }});
        assert_eq!(
            child_status_pure("deployment_state", &p),
            ChildStatus::InProgress
        );
    }

    #[test]
    fn deployment_state_pending_when_empty_envs() {
        let p = json!({"envs":{}});
        assert_eq!(
            child_status_pure("deployment_state", &p),
            ChildStatus::Pending
        );
    }

    // --- metric_baseline → Unknown (Phase 2 handles in context) ---

    #[test]
    fn metric_baseline_returns_unknown() {
        let p = json!({"baseline":{"P@5":0.18},"current":{"P@5":0.19}});
        assert_eq!(
            child_status_pure("metric_baseline", &p),
            ChildStatus::Unknown
        );
    }

    // --- unknown archetype (H-5) ---

    #[test]
    fn unknown_archetype_returns_unknown() {
        let p = json!({"anything":42});
        assert_eq!(
            child_status_pure("link_rot_crawler", &p),
            ChildStatus::Unknown
        );
    }

    // --- idempotency property ---

    #[test]
    fn idempotent_pure_function() {
        let cases = [
            (
                "failure_table",
                json!({"failures":[{"id":"F-1","status":"pass"}]}),
            ),
            ("task_list", json!({"tasks":[{"id":"T-1","status":"done"}]})),
            ("audit_issues", json!({"issues":[]})),
            ("reflective", json!({"status":"decided"})),
            ("goal", json!({"status":"done"})),
            (
                "deployment_state",
                json!({"envs":{"prod":{"enabled":true}}}),
            ),
        ];
        for (arch, params) in &cases {
            let r1 = child_status_pure(arch, params);
            let r2 = child_status_pure(arch, params);
            assert_eq!(r1, r2, "non-idempotent: {arch}");
        }
    }
}
