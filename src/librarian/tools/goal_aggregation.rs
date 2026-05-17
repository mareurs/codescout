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

// =====================================================================
// Refresh metadata (D5) — Rust-owned bookkeeping projected into params
// =====================================================================

/// One child's status before/after a refresh — populated when the verdict
/// changed since the prior refresh.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusDelta {
    pub child_id: String,
    pub from: String,
    pub to: String,
}

/// Rust-owned bookkeeping for a goal-tracker refresh cycle (amendment D5).
/// The LLM copies this verbatim from `context.refresh_meta` into
/// `params.refresh_meta` and does not modify any field — Rust computes
/// every value deterministically.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RefreshMeta {
    pub last_refresh_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh_commit: Option<String>,
    #[serde(default)]
    pub children_status_delta: Vec<StatusDelta>,
    #[serde(default)]
    pub commit_count_since_last: u64,
    #[serde(default)]
    pub unchanged_refreshes: u64,
    #[serde(default)]
    pub degraded: bool,
    #[serde(default)]
    pub orphan_children: Vec<String>,
}

/// Compute a fresh `RefreshMeta` from prior state + the current refresh's
/// observations.
///
/// `prior_refresh_meta` — last refresh's RefreshMeta (None on first refresh).
/// `prior_child_statuses` — what the goal's `params.children[].status` said
///   BEFORE this refresh. Used to compute deltas.
/// `fresh_child_statuses` — what the kernel produced THIS refresh.
/// `orphan_children` — child_ids whose `status == "orphan"` this refresh.
/// `now` — clock-injectable timestamp (tests pass a fixed value).
/// `head_commit` — git HEAD short hash, if available.
/// `commits_since_last` — count of commits since the prior refresh,
///   typically derived from `context.git_log.len()` when that gather
///   source is configured.
pub fn compute_refresh_meta(
    prior_refresh_meta: Option<&RefreshMeta>,
    prior_child_statuses: &[(String, ChildStatus)],
    fresh_child_statuses: &[(String, ChildStatus)],
    orphan_children: Vec<String>,
    now: chrono::DateTime<chrono::Utc>,
    head_commit: Option<String>,
    commits_since_last: u64,
) -> RefreshMeta {
    let mut deltas: Vec<StatusDelta> = Vec::new();
    for (child_id, fresh_status) in fresh_child_statuses {
        let prior_status = prior_child_statuses
            .iter()
            .find(|(id, _)| id == child_id)
            .map(|(_, s)| *s);
        match prior_status {
            None => {
                // Newly-added child between refreshes.
                deltas.push(StatusDelta {
                    child_id: child_id.clone(),
                    from: "(new)".to_string(),
                    to: fresh_status.as_str().to_string(),
                });
            }
            Some(prior) if prior != *fresh_status => {
                deltas.push(StatusDelta {
                    child_id: child_id.clone(),
                    from: prior.as_str().to_string(),
                    to: fresh_status.as_str().to_string(),
                });
            }
            Some(_) => {}
        }
    }

    let unchanged_now = deltas.is_empty() && commits_since_last == 0;
    let prior_unchanged = prior_refresh_meta
        .map(|m| m.unchanged_refreshes)
        .unwrap_or(0);
    let unchanged_refreshes = if unchanged_now {
        prior_unchanged + 1
    } else {
        0
    };

    RefreshMeta {
        last_refresh_at: now.to_rfc3339(),
        last_refresh_commit: head_commit,
        children_status_delta: deltas,
        commit_count_since_last: commits_since_last,
        unchanged_refreshes,
        degraded: false,
        orphan_children,
    }
}

/// Parse a `ChildStatus` from the kebab-case string we serialize as.
/// Returns `Unknown` for unrecognized inputs.
pub fn child_status_from_str(s: &str) -> ChildStatus {
    match s {
        "pending" => ChildStatus::Pending,
        "active" => ChildStatus::Active,
        "in-progress" => ChildStatus::InProgress,
        "done" => ChildStatus::Done,
        "blocked" => ChildStatus::Blocked,
        "orphan" => ChildStatus::Orphan,
        _ => ChildStatus::Unknown,
    }
}

// =====================================================================
// Auto-close gate (D6) — Rust enforces rule 4 / rule 6 NEVER conditions
// =====================================================================

/// Result of `evaluate_gate` over goal-tracker params.
#[derive(Debug, Clone, PartialEq)]
pub enum GateOutcome {
    /// All conditions satisfied — the goal may flip status to "done".
    AutoClose,
    /// One condition unmet — the caller should refuse the status flip.
    Block(GateBlockReason),
}

/// Why the auto-close gate refused to flip status to "done".
#[derive(Debug, Clone, PartialEq)]
pub enum GateBlockReason {
    /// `children.len() < 2` (amendment D9).
    TooFewChildren { count: usize },
    /// One or more children have `status != "done"`.
    ChildrenIncomplete { incomplete: Vec<String> },
    /// One or more `acceptance_signals[].met` is false.
    SignalsUnmet { unmet: Vec<String> },
}

impl std::fmt::Display for GateBlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateBlockReason::TooFewChildren { count } => write!(
                f,
                "goal has {count} children; auto-close requires \u{2265}2 (amendment D9)"
            ),
            GateBlockReason::ChildrenIncomplete { incomplete } => {
                write!(f, "children not all done: {}", incomplete.join(", "))
            }
            GateBlockReason::SignalsUnmet { unmet } => {
                write!(f, "acceptance_signals unmet: {}", unmet.join("; "))
            }
        }
    }
}

/// Evaluate the goal-tracker auto-close gate against the supplied params.
///
/// Returns `GateOutcome::AutoClose` when ALL of:
///   - `params.children.len() >= 2` (D9)
///   - every `children[].status == "done"`
///   - every `acceptance_signals[].met` is true
///
/// Otherwise returns `GateOutcome::Block` with the first failing condition.
/// This is the Rust enforcement of what was formerly prompt rule 6's NEVER
/// list and rule 4's gate (D6).
pub fn evaluate_gate(params: &Value) -> GateOutcome {
    let children: &[Value] = params
        .get("children")
        .and_then(|c| c.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if children.len() < 2 {
        return GateOutcome::Block(GateBlockReason::TooFewChildren {
            count: children.len(),
        });
    }
    let incomplete: Vec<String> = children
        .iter()
        .filter(|c| c.get("status").and_then(|s| s.as_str()) != Some("done"))
        .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(String::from))
        .collect();
    if !incomplete.is_empty() {
        return GateOutcome::Block(GateBlockReason::ChildrenIncomplete { incomplete });
    }
    let unmet: Vec<String> = params
        .get("acceptance_signals")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|s| s.get("met").and_then(|m| m.as_bool()) != Some(true))
                .filter_map(|s| {
                    s.get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    if !unmet.is_empty() {
        return GateOutcome::Block(GateBlockReason::SignalsUnmet { unmet });
    }
    GateOutcome::AutoClose
}

// =====================================================================
// Scope-growth cap (D10) — refuse more than 1 new child per refresh
// =====================================================================

/// Verdict from a passing `validate_scope_growth` call.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeGrowthVerdict {
    pub added_count: usize,
    pub added_ids: Vec<String>,
}

/// Why scope-growth was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeGrowthError {
    /// More than 1 new `children[].id` introduced in this refresh.
    TooManyNewChildren { count: usize, ids: Vec<String> },
}

impl std::fmt::Display for ScopeGrowthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeGrowthError::TooManyNewChildren { count, ids } => write!(
                f,
                "scope-growth cap: {count} new children in one refresh ({}); D10 allows at most 1. Defer the rest to a follow-up refresh.",
                ids.join(", ")
            ),
        }
    }
}

/// Enforce D10: at most 1 new `children[].id` may appear between the prior
/// children list and the submitted-post-merge children list.
///
/// Initial seed (`prior_children` empty) is exempt — the cap targets growth
/// during refreshes, not creation. Same-id, status-only changes are not
/// counted as growth.
pub fn validate_scope_growth(
    prior_children: &[Value],
    submitted_children: &[Value],
) -> Result<ScopeGrowthVerdict, ScopeGrowthError> {
    if prior_children.is_empty() {
        // Initial seed — not a growth event.
        return Ok(ScopeGrowthVerdict {
            added_count: submitted_children.len(),
            added_ids: submitted_children
                .iter()
                .filter_map(|c| c.get("id").and_then(|i| i.as_str()).map(String::from))
                .collect(),
        });
    }
    let prior_ids: std::collections::HashSet<&str> = prior_children
        .iter()
        .filter_map(|c| c.get("id").and_then(|i| i.as_str()))
        .collect();
    let new_ids: Vec<String> = submitted_children
        .iter()
        .filter_map(|c| c.get("id").and_then(|i| i.as_str()))
        .filter(|id| !prior_ids.contains(id))
        .map(String::from)
        .collect();
    if new_ids.len() > 1 {
        return Err(ScopeGrowthError::TooManyNewChildren {
            count: new_ids.len(),
            ids: new_ids,
        });
    }
    Ok(ScopeGrowthVerdict {
        added_count: new_ids.len(),
        added_ids: new_ids,
    })
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
        assert!(result.met);
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

    // =================================================================
    // D5 — refresh_meta compute tests
    // =================================================================

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        use chrono::TimeZone;
        chrono::Utc.with_ymd_and_hms(2026, 5, 17, 12, 0, 0).unwrap()
    }

    #[test]
    fn refresh_meta_no_prior_no_changes() {
        let fresh = vec![
            ("C-1".to_string(), ChildStatus::Active),
            ("C-2".to_string(), ChildStatus::Done),
        ];
        let meta = compute_refresh_meta(None, &[], &fresh, vec![], fixed_now(), None, 0);
        assert_eq!(meta.children_status_delta.len(), 2);
        assert_eq!(meta.children_status_delta[0].from, "(new)");
        assert_eq!(meta.commit_count_since_last, 0);
        assert_eq!(meta.unchanged_refreshes, 0);
        assert!(meta.orphan_children.is_empty());
    }

    #[test]
    fn refresh_meta_no_change_increments_unchanged_counter() {
        let prior_statuses = vec![
            ("C-1".to_string(), ChildStatus::Active),
            ("C-2".to_string(), ChildStatus::Done),
        ];
        let prior_meta = RefreshMeta {
            last_refresh_at: "2026-05-16T12:00:00Z".into(),
            unchanged_refreshes: 3,
            ..Default::default()
        };
        let fresh = prior_statuses.clone();
        let meta = compute_refresh_meta(
            Some(&prior_meta),
            &prior_statuses,
            &fresh,
            vec![],
            fixed_now(),
            None,
            0,
        );
        assert!(meta.children_status_delta.is_empty());
        assert_eq!(meta.unchanged_refreshes, 4);
    }

    #[test]
    fn refresh_meta_status_change_resets_unchanged_counter() {
        let prior_statuses = vec![("C-1".to_string(), ChildStatus::Active)];
        let prior_meta = RefreshMeta {
            unchanged_refreshes: 5,
            ..Default::default()
        };
        let fresh = vec![("C-1".to_string(), ChildStatus::Done)];
        let meta = compute_refresh_meta(
            Some(&prior_meta),
            &prior_statuses,
            &fresh,
            vec![],
            fixed_now(),
            None,
            0,
        );
        assert_eq!(meta.children_status_delta.len(), 1);
        assert_eq!(meta.children_status_delta[0].child_id, "C-1");
        assert_eq!(meta.children_status_delta[0].from, "active");
        assert_eq!(meta.children_status_delta[0].to, "done");
        assert_eq!(meta.unchanged_refreshes, 0);
    }

    #[test]
    fn refresh_meta_commits_reset_unchanged_counter() {
        let prior_meta = RefreshMeta {
            unchanged_refreshes: 2,
            ..Default::default()
        };
        let fresh = vec![("C-1".to_string(), ChildStatus::Active)];
        let prior_statuses = fresh.clone();
        let meta = compute_refresh_meta(
            Some(&prior_meta),
            &prior_statuses,
            &fresh,
            vec![],
            fixed_now(),
            None,
            3, // commits arrived
        );
        // No status change, but commits != 0 → unchanged_refreshes resets.
        assert!(meta.children_status_delta.is_empty());
        assert_eq!(meta.commit_count_since_last, 3);
        assert_eq!(meta.unchanged_refreshes, 0);
    }

    #[test]
    fn refresh_meta_orphan_children_carried_through() {
        let fresh = vec![("C-1".to_string(), ChildStatus::Orphan)];
        let meta = compute_refresh_meta(
            None,
            &[],
            &fresh,
            vec!["C-1".to_string()],
            fixed_now(),
            None,
            0,
        );
        assert_eq!(meta.orphan_children, vec!["C-1".to_string()]);
    }

    #[test]
    fn refresh_meta_head_commit_passthrough() {
        let fresh = vec![("C-1".to_string(), ChildStatus::Active)];
        let meta = compute_refresh_meta(
            None,
            &[],
            &fresh,
            vec![],
            fixed_now(),
            Some("abc1234".to_string()),
            0,
        );
        assert_eq!(meta.last_refresh_commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn refresh_meta_idempotent_when_inputs_unchanged() {
        // Two back-to-back calls with same inputs (no time-skewing prior)
        // should produce byte-identical RefreshMeta modulo last_refresh_at.
        let prior_statuses = vec![("C-1".to_string(), ChildStatus::Active)];
        let fresh = prior_statuses.clone();
        let m1 = compute_refresh_meta(None, &prior_statuses, &fresh, vec![], fixed_now(), None, 0);
        let m2 = compute_refresh_meta(None, &prior_statuses, &fresh, vec![], fixed_now(), None, 0);
        assert_eq!(m1, m2);
    }

    // =================================================================
    // D6 — auto-close gate tests
    // =================================================================

    #[test]
    fn gate_blocks_too_few_children() {
        use crate::librarian::tools::goal_aggregation::{
            evaluate_gate, GateBlockReason, GateOutcome,
        };
        let params = json!({
            "children": [
                {"id":"C-1","status":"done"}
            ],
            "acceptance_signals": []
        });
        match evaluate_gate(&params) {
            GateOutcome::Block(GateBlockReason::TooFewChildren { count }) => assert_eq!(count, 1),
            other => panic!("expected TooFewChildren, got {other:?}"),
        }
    }

    #[test]
    fn gate_blocks_children_incomplete() {
        use crate::librarian::tools::goal_aggregation::{
            evaluate_gate, GateBlockReason, GateOutcome,
        };
        let params = json!({
            "children": [
                {"id":"C-1","status":"done"},
                {"id":"C-2","status":"active"}
            ],
            "acceptance_signals": []
        });
        match evaluate_gate(&params) {
            GateOutcome::Block(GateBlockReason::ChildrenIncomplete { incomplete }) => {
                assert_eq!(incomplete, vec!["C-2".to_string()]);
            }
            other => panic!("expected ChildrenIncomplete, got {other:?}"),
        }
    }

    #[test]
    fn gate_blocks_signals_unmet() {
        use crate::librarian::tools::goal_aggregation::{
            evaluate_gate, GateBlockReason, GateOutcome,
        };
        let params = json!({
            "children": [
                {"id":"C-1","status":"done"},
                {"id":"C-2","status":"done"}
            ],
            "acceptance_signals": [
                {"description":"A","met":true},
                {"description":"B","met":false}
            ]
        });
        match evaluate_gate(&params) {
            GateOutcome::Block(GateBlockReason::SignalsUnmet { unmet }) => {
                assert_eq!(unmet, vec!["B".to_string()]);
            }
            other => panic!("expected SignalsUnmet, got {other:?}"),
        }
    }

    #[test]
    fn gate_passes_when_all_conditions_met() {
        use crate::librarian::tools::goal_aggregation::{evaluate_gate, GateOutcome};
        let params = json!({
            "children": [
                {"id":"C-1","status":"done"},
                {"id":"C-2","status":"done"}
            ],
            "acceptance_signals": [
                {"description":"A","met":true},
                {"description":"B","met":true}
            ]
        });
        assert_eq!(evaluate_gate(&params), GateOutcome::AutoClose);
    }

    // =================================================================
    // D10 — scope-growth cap tests
    // =================================================================

    #[test]
    fn scope_growth_allows_initial_seed_with_many_children() {
        use crate::librarian::tools::goal_aggregation::validate_scope_growth;
        let prior = vec![];
        let submitted = vec![
            json!({"id":"C-1","status":"active"}),
            json!({"id":"C-2","status":"pending"}),
            json!({"id":"C-3","status":"done"}),
        ];
        let verdict = validate_scope_growth(&prior, &submitted).unwrap();
        assert_eq!(verdict.added_count, 3);
    }

    #[test]
    fn scope_growth_allows_one_new_child_per_refresh() {
        use crate::librarian::tools::goal_aggregation::validate_scope_growth;
        let prior = vec![
            json!({"id":"C-1","status":"done"}),
            json!({"id":"C-2","status":"active"}),
        ];
        let submitted = vec![
            json!({"id":"C-1","status":"done"}),
            json!({"id":"C-2","status":"active"}),
            json!({"id":"C-3","status":"pending"}),
        ];
        let verdict = validate_scope_growth(&prior, &submitted).unwrap();
        assert_eq!(verdict.added_count, 1);
        assert_eq!(verdict.added_ids, vec!["C-3".to_string()]);
    }

    #[test]
    fn scope_growth_rejects_two_new_children() {
        use crate::librarian::tools::goal_aggregation::{validate_scope_growth, ScopeGrowthError};
        let prior = vec![json!({"id":"C-1","status":"done"})];
        let submitted = vec![
            json!({"id":"C-1","status":"done"}),
            json!({"id":"C-2","status":"active"}),
            json!({"id":"C-3","status":"pending"}),
        ];
        match validate_scope_growth(&prior, &submitted).unwrap_err() {
            ScopeGrowthError::TooManyNewChildren { count, ids } => {
                assert_eq!(count, 2);
                assert!(ids.contains(&"C-2".to_string()));
                assert!(ids.contains(&"C-3".to_string()));
            }
        }
    }

    #[test]
    fn scope_growth_allows_status_only_change_no_new_ids() {
        use crate::librarian::tools::goal_aggregation::validate_scope_growth;
        let prior = vec![
            json!({"id":"C-1","status":"active"}),
            json!({"id":"C-2","status":"pending"}),
        ];
        let submitted = vec![
            json!({"id":"C-1","status":"done"}),
            json!({"id":"C-2","status":"done"}),
        ];
        let verdict = validate_scope_growth(&prior, &submitted).unwrap();
        assert_eq!(verdict.added_count, 0);
    }
}
