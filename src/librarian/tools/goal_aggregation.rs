//! Deterministic predicates for goal-tracker child status derivation.
//!
//! Implements amendment D1–D3 from
//! `docs/superpowers/specs/2026-05-17-goal-tracker-amendment.md`.
//!
//! `child_status_pure` is a **pure function of (archetype, child_params)**.
//! Archetypes whose status depends on parent context (`metric_baseline`)
//! return `ChildStatus::Unknown` here — those go through
//! `goal_aggregation::contextual::*` in a later phase.

use serde::{Deserialize, Serialize};
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

// =====================================================================
// Acceptance signals (D4) — structured kind discriminant for goal-tracker
// =====================================================================

/// Comparison operator for `MetricThreshold` signal evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThresholdOp {
    #[serde(rename = ">=")]
    Gte,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<=")]
    Lte,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "==")]
    Eq,
}

impl ThresholdOp {
    fn compare(self, lhs: f64, rhs: f64) -> bool {
        match self {
            ThresholdOp::Gte => lhs >= rhs,
            ThresholdOp::Gt => lhs > rhs,
            ThresholdOp::Lte => lhs <= rhs,
            ThresholdOp::Lt => lhs < rhs,
            ThresholdOp::Eq => (lhs - rhs).abs() < f64::EPSILON,
        }
    }
}

/// Structured per-kind acceptance signal specification (amendment D4).
///
/// Each variant carries the parameters needed to evaluate that kind of signal
/// against a referenced child artifact's params. `Freeform` is the backward-compat
/// default — when `kind` is absent in the input JSON, the signal is treated as
/// human-evaluated and its `met` field is trusted verbatim.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AcceptanceSignalSpec {
    /// Human-evaluated signal. `met` and `evidence` are written by the LLM
    /// during refresh. No Rust-side derivation.
    #[default]
    Freeform,

    /// Audit-issues child has at most `max_open` rows with `status == "open"`.
    /// `max_open` defaults to 0 (zero open = goal met).
    AuditIssuesOpenCount {
        evidence_child_id: String,
        #[serde(default)]
        max_open: u64,
    },

    /// Failure-table child has zero rows with `status ∈ {"fail","flaky"}`.
    /// `pass` and `wontfix` count as clean.
    FailureTableClean { evidence_child_id: String },

    /// Task-list child has all rows with `status == "done"` and the list is non-empty.
    TaskListComplete { evidence_child_id: String },

    /// Metric-baseline child's `current[metric_key]` satisfies `op threshold`.
    MetricThreshold {
        evidence_child_id: String,
        metric_key: String,
        op: ThresholdOp,
        threshold: f64,
    },

    /// Reflective child's `status` is `"decided"` or `"archived"`.
    ReflectiveDecided { evidence_child_id: String },

    /// Deployment-state child has the listed envs all enabled.
    /// If `envs` is None, all envs in the child must be enabled.
    DeploymentEnvsEnabled {
        evidence_child_id: String,
        #[serde(default)]
        envs: Option<Vec<String>>,
    },
}

/// Acceptance signal envelope: the LLM-readable description + status fields,
/// plus the structured spec that drives Rust-side evaluation (D4).
///
/// Legacy signals lacking `kind` deserialize as `Freeform` and bypass Rust
/// evaluation — their `met` field is trusted verbatim.
///
/// Custom `Deserialize` impl rather than derive: serde's `flatten + default`
/// combination doesn't gracefully handle a missing internally-tagged
/// discriminator (it raises `missing field 'kind'`). We probe for `kind` and
/// fall back to `Freeform` when absent.
#[derive(Debug, Clone)]
pub struct AcceptanceSignal {
    pub description: String,
    pub met: bool,
    pub evidence: String,
    pub spec: AcceptanceSignalSpec,
}

impl<'de> Deserialize<'de> for AcceptanceSignal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let met = value.get("met").and_then(|v| v.as_bool()).unwrap_or(false);
        let evidence = value
            .get("evidence")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let spec = if value.get("kind").is_none() {
            AcceptanceSignalSpec::Freeform
        } else {
            AcceptanceSignalSpec::deserialize(value.clone()).map_err(serde::de::Error::custom)?
        };
        Ok(AcceptanceSignal {
            description,
            met,
            evidence,
            spec,
        })
    }
}

/// Result of running `evaluate_signal` against a child's params.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    pub met: bool,
    pub evidence: String,
    /// `Some(msg)` when the signal could not be evaluated (e.g. unresolvable
    /// `evidence_child_id`, missing required field in child params). The LLM
    /// should surface this in the next refresh's progress_log note.
    pub error: Option<String>,
}

/// Evaluate one `AcceptanceSignal` against a slice of `(child_id, archetype, params)` tuples.
/// `Freeform` passes through the signal's existing `met`/`evidence`. Every other
/// variant looks up `evidence_child_id`, applies the archetype-appropriate
/// predicate, and returns a fresh `EvalResult`.
pub fn evaluate_signal(
    signal: &AcceptanceSignal,
    children: &[(String, String, Value)],
) -> EvalResult {
    match &signal.spec {
        AcceptanceSignalSpec::Freeform => EvalResult {
            met: signal.met,
            evidence: signal.evidence.clone(),
            error: None,
        },
        AcceptanceSignalSpec::AuditIssuesOpenCount {
            evidence_child_id,
            max_open,
        } => match find_child(children, evidence_child_id) {
            None => unresolved(evidence_child_id),
            Some((_, params)) => {
                let open_count = params
                    .get("issues")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|i| i.get("status").and_then(|s| s.as_str()) == Some("open"))
                            .count()
                    })
                    .unwrap_or(0) as u64;
                EvalResult {
                    met: open_count <= *max_open,
                    evidence: format!(
                        "audit_issues {evidence_child_id}: {open_count} open (max {max_open})"
                    ),
                    error: None,
                }
            }
        },
        AcceptanceSignalSpec::FailureTableClean { evidence_child_id } => {
            match find_child(children, evidence_child_id) {
                None => unresolved(evidence_child_id),
                Some((_, params)) => {
                    let failures = params.get("failures").and_then(|v| v.as_array());
                    let (total, bad) = match failures {
                        None => (0, 0),
                        Some(arr) => {
                            let bad = arr
                                .iter()
                                .filter(|f| {
                                    matches!(
                                        f.get("status").and_then(|s| s.as_str()),
                                        Some("fail") | Some("flaky")
                                    )
                                })
                                .count();
                            (arr.len(), bad)
                        }
                    };
                    EvalResult {
                        met: bad == 0,
                        evidence: format!(
                            "failure_table {evidence_child_id}: {bad}/{total} fail|flaky"
                        ),
                        error: None,
                    }
                }
            }
        }
        AcceptanceSignalSpec::TaskListComplete { evidence_child_id } => {
            match find_child(children, evidence_child_id) {
                None => unresolved(evidence_child_id),
                Some((_, params)) => {
                    let tasks = params.get("tasks").and_then(|v| v.as_array());
                    let (total, done) = match tasks {
                        None => (0, 0),
                        Some(arr) => {
                            let done = arr
                                .iter()
                                .filter(|t| {
                                    t.get("status").and_then(|s| s.as_str()) == Some("done")
                                })
                                .count();
                            (arr.len(), done)
                        }
                    };
                    EvalResult {
                        met: total > 0 && done == total,
                        evidence: format!("task_list {evidence_child_id}: {done}/{total} done"),
                        error: None,
                    }
                }
            }
        }
        AcceptanceSignalSpec::MetricThreshold {
            evidence_child_id,
            metric_key,
            op,
            threshold,
        } => match find_child(children, evidence_child_id) {
            None => unresolved(evidence_child_id),
            Some((_, params)) => {
                let current_value = params
                    .get("current")
                    .and_then(|c| c.get(metric_key))
                    .and_then(|v| v.as_f64());
                match current_value {
                    None => EvalResult {
                        met: false,
                        evidence: format!(
                            "metric_baseline {evidence_child_id}: current.{metric_key} missing or not a number"
                        ),
                        error: Some(format!(
                            "missing metric {metric_key} in child {evidence_child_id} params.current"
                        )),
                    },
                    Some(v) => EvalResult {
                        met: op.compare(v, *threshold),
                        evidence: format!(
                            "metric_baseline {evidence_child_id}: current.{metric_key}={v} {op_str} {threshold}",
                            op_str = match op {
                                ThresholdOp::Gte => ">=",
                                ThresholdOp::Gt => ">",
                                ThresholdOp::Lte => "<=",
                                ThresholdOp::Lt => "<",
                                ThresholdOp::Eq => "==",
                            }
                        ),
                        error: None,
                    },
                }
            }
        },
        AcceptanceSignalSpec::ReflectiveDecided { evidence_child_id } => {
            match find_child(children, evidence_child_id) {
                None => unresolved(evidence_child_id),
                Some((_, params)) => {
                    let status = params.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    let decided = matches!(status, "decided" | "archived");
                    EvalResult {
                        met: decided,
                        evidence: format!("reflective {evidence_child_id}: status={status}"),
                        error: None,
                    }
                }
            }
        }
        AcceptanceSignalSpec::DeploymentEnvsEnabled {
            evidence_child_id,
            envs,
        } => match find_child(children, evidence_child_id) {
            None => unresolved(evidence_child_id),
            Some((_, params)) => {
                let envs_obj = params.get("envs").and_then(|e| e.as_object());
                let (all_required, total) = match (envs_obj, envs) {
                    (None, _) => (false, 0),
                    (Some(o), None) => {
                        let total = o.len();
                        let enabled = o
                            .values()
                            .filter(|v| v.get("enabled").and_then(|b| b.as_bool()) == Some(true))
                            .count();
                        (total > 0 && enabled == total, total)
                    }
                    (Some(o), Some(required)) => {
                        let enabled = required
                            .iter()
                            .filter(|env| {
                                o.get(env.as_str())
                                    .and_then(|e| e.get("enabled"))
                                    .and_then(|b| b.as_bool())
                                    == Some(true)
                            })
                            .count();
                        (enabled == required.len(), required.len())
                    }
                };
                EvalResult {
                    met: all_required,
                    evidence: format!(
                        "deployment_state {evidence_child_id}: {total} envs evaluated, all_enabled={all_required}"
                    ),
                    error: None,
                }
            }
        },
    }
}

fn find_child<'a>(
    children: &'a [(String, String, Value)],
    target_id: &str,
) -> Option<(&'a str, &'a Value)> {
    children
        .iter()
        .find(|(id, _, _)| id == target_id)
        .map(|(_, archetype, params)| (archetype.as_str(), params))
}

fn unresolved(child_id: &str) -> EvalResult {
    EvalResult {
        met: false,
        evidence: format!("evidence_child_id={child_id} not found among linked children"),
        error: Some(format!("unresolvable evidence_child_id: {child_id}")),
    }
}

/// Like `child_status_pure`, but consults the parent goal's
/// `acceptance_signals` when the archetype is `metric_baseline` (D8).
///
/// For `metric_baseline`, this folds across every parent signal whose
/// `MetricThreshold` cites `child_id`:
/// - all citing signals met → `ChildStatus::Done`
/// - at least one met (mixed) → `ChildStatus::InProgress`
/// - none met → `ChildStatus::Active`
/// - no citing signals at all → `ChildStatus::Active` (the child exists,
///   but no parent signal pins a threshold to it — needs prompt-level
///   evaluation per legacy behavior)
///
/// All other archetypes fall through to `child_status_pure`.
pub fn child_status_in_context(
    archetype: &str,
    child_id: &str,
    child_params: &Value,
    parent_signals: &[AcceptanceSignal],
    children: &[(String, String, Value)],
) -> ChildStatus {
    if archetype != "metric_baseline" {
        return child_status_pure(archetype, child_params);
    }

    let citing: Vec<&AcceptanceSignal> = parent_signals
        .iter()
        .filter(|s| {
            matches!(
                &s.spec,
                AcceptanceSignalSpec::MetricThreshold { evidence_child_id, .. }
                    if evidence_child_id == child_id
            )
        })
        .collect();

    if citing.is_empty() {
        return ChildStatus::Active;
    }

    let evaluated: Vec<bool> = citing
        .iter()
        .map(|s| evaluate_signal(s, children).met)
        .collect();

    if evaluated.iter().all(|&m| m) {
        ChildStatus::Done
    } else if evaluated.iter().any(|&m| m) {
        ChildStatus::InProgress
    } else {
        ChildStatus::Active
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

    // =================================================================
    // D4 — acceptance signal evaluation tests
    // =================================================================

    fn child(id: &str, archetype: &str, params: Value) -> (String, String, Value) {
        (id.to_string(), archetype.to_string(), params)
    }

    #[test]
    fn freeform_passes_through_existing_met_and_evidence() {
        let signal = AcceptanceSignal {
            description: "manual gate".into(),
            met: true,
            evidence: "human note".into(),
            spec: AcceptanceSignalSpec::Freeform,
        };
        let result = evaluate_signal(&signal, &[]);
        assert_eq!(result.met, true);
        assert_eq!(result.evidence, "human note");
        assert!(result.error.is_none());
    }

    #[test]
    fn legacy_signal_without_kind_field_deserializes_as_freeform() {
        let s: AcceptanceSignal =
            serde_json::from_str(r#"{"description":"x","met":true,"evidence":"e"}"#).unwrap();
        assert_eq!(s.spec, AcceptanceSignalSpec::Freeform);
        assert!(s.met);
    }

    #[test]
    fn explicit_kind_freeform_deserializes_as_freeform() {
        let s: AcceptanceSignal = serde_json::from_str(
            r#"{"description":"x","met":false,"evidence":"","kind":"freeform"}"#,
        )
        .unwrap();
        assert_eq!(s.spec, AcceptanceSignalSpec::Freeform);
    }

    #[test]
    fn audit_issues_open_count_met_when_zero_open() {
        let signal = AcceptanceSignal {
            description: "no open audits".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::AuditIssuesOpenCount {
                evidence_child_id: "C-1".into(),
                max_open: 0,
            },
        };
        let children = vec![child(
            "C-1",
            "audit_issues",
            json!({"issues":[{"n":1,"status":"fixed"}]}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
        assert!(result.evidence.contains("0 open"));
    }

    #[test]
    fn audit_issues_open_count_unmet_when_above_max() {
        let signal = AcceptanceSignal {
            description: "≤1 open".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::AuditIssuesOpenCount {
                evidence_child_id: "C-1".into(),
                max_open: 1,
            },
        };
        let children = vec![child(
            "C-1",
            "audit_issues",
            json!({"issues":[
                {"n":1,"status":"open"},
                {"n":2,"status":"open"},
                {"n":3,"status":"fixed"}
            ]}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(!result.met);
        assert!(result.evidence.contains("2 open"));
    }

    #[test]
    fn failure_table_clean_met_when_no_bad() {
        let signal = AcceptanceSignal {
            description: "all pass".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::FailureTableClean {
                evidence_child_id: "C-2".into(),
            },
        };
        let children = vec![child(
            "C-2",
            "failure_table",
            json!({"failures":[
                {"id":"F-1","status":"pass"},
                {"id":"F-2","status":"wontfix"}
            ]}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
    }

    #[test]
    fn failure_table_clean_unmet_when_any_flaky() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::FailureTableClean {
                evidence_child_id: "C-2".into(),
            },
        };
        let children = vec![child(
            "C-2",
            "failure_table",
            json!({"failures":[{"id":"F-1","status":"flaky"}]}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(!result.met);
        assert!(result.evidence.contains("1/1 fail|flaky"));
    }

    #[test]
    fn task_list_complete_met_when_all_done() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::TaskListComplete {
                evidence_child_id: "C-3".into(),
            },
        };
        let children = vec![child(
            "C-3",
            "task_list",
            json!({"tasks":[
                {"id":"T-1","status":"done"},
                {"id":"T-2","status":"done"}
            ]}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
    }

    #[test]
    fn task_list_complete_unmet_when_empty() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::TaskListComplete {
                evidence_child_id: "C-3".into(),
            },
        };
        let children = vec![child("C-3", "task_list", json!({"tasks":[]}))];
        let result = evaluate_signal(&signal, &children);
        assert!(!result.met);
    }

    #[test]
    fn metric_threshold_gte_met() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "P@5".into(),
                op: ThresholdOp::Gte,
                threshold: 0.20,
            },
        };
        let children = vec![child(
            "C-M",
            "metric_baseline",
            json!({"baseline":{"P@5":0.18},"current":{"P@5":0.21}}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
        assert!(result.evidence.contains("0.21"));
    }

    #[test]
    fn metric_threshold_lt_unmet() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "P@5".into(),
                op: ThresholdOp::Gte,
                threshold: 0.25,
            },
        };
        let children = vec![child(
            "C-M",
            "metric_baseline",
            json!({"current":{"P@5":0.19}}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(!result.met);
    }

    #[test]
    fn metric_threshold_missing_key_returns_error() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "R@10".into(),
                op: ThresholdOp::Gte,
                threshold: 0.5,
            },
        };
        let children = vec![child(
            "C-M",
            "metric_baseline",
            json!({"current":{"P@5":0.21}}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(!result.met);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("R@10"));
    }

    #[test]
    fn reflective_decided_met() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::ReflectiveDecided {
                evidence_child_id: "C-R".into(),
            },
        };
        let children = vec![child("C-R", "reflective", json!({"status":"decided"}))];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
    }

    #[test]
    fn deployment_envs_enabled_all_when_no_list() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::DeploymentEnvsEnabled {
                evidence_child_id: "C-D".into(),
                envs: None,
            },
        };
        let children = vec![child(
            "C-D",
            "deployment_state",
            json!({"envs":{"dev":{"enabled":true},"prod":{"enabled":true}}}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
    }

    #[test]
    fn deployment_envs_enabled_subset_required() {
        let signal = AcceptanceSignal {
            description: "prod only".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::DeploymentEnvsEnabled {
                evidence_child_id: "C-D".into(),
                envs: Some(vec!["prod".into()]),
            },
        };
        // dev disabled, prod enabled — required subset met
        let children = vec![child(
            "C-D",
            "deployment_state",
            json!({"envs":{"dev":{"enabled":false},"prod":{"enabled":true}}}),
        )];
        let result = evaluate_signal(&signal, &children);
        assert!(result.met);
    }

    #[test]
    fn unresolvable_evidence_child_id_returns_error() {
        let signal = AcceptanceSignal {
            description: "".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::ReflectiveDecided {
                evidence_child_id: "C-MISSING".into(),
            },
        };
        let result = evaluate_signal(&signal, &[]);
        assert!(!result.met);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("C-MISSING"));
    }
}
