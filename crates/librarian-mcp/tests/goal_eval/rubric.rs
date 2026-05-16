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
        .flatten()
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
    let _ = before; // currently unused; kept in the signature for future delta scoring.

    // Correct status (all checkpoints)
    let correct_status = Some(
        after
            .get("status")
            .and_then(|v| v.as_str())
            .is_some_and(|actual| actual == expected_status),
    );

    // Correct evidence citation (T1, T2)
    let correct_evidence_citation = if matches!(cp, Checkpoint::T1 | Checkpoint::T2) {
        let log = after
            .get("progress_log")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.last());
        Some(log.is_some_and(|entry| {
            entry
                .get("evidence_artifacts")
                .and_then(|v| v.as_array())
                .is_some_and(|arr| !arr.is_empty())
                || entry
                    .get("evidence_commits")
                    .and_then(|v| v.as_array())
                    .is_some_and(|arr| !arr.is_empty())
        }))
    } else {
        None
    };

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
    let no_fabrication = Some(cited_commits.iter().all(|c| {
        real_commits
            .iter()
            .any(|r| c.starts_with(r) || r.starts_with(c.as_str()))
    }));

    // Appropriate decomposition (T0 only)
    let appropriate_decomposition = if matches!(cp, Checkpoint::T0) {
        let children_count = after
            .get("children")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
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
            .is_some_and(|arr| {
                arr.iter().all(|c| {
                    c.get("archetype")
                        .and_then(|v| v.as_str())
                        .is_some_and(|a| valid_archetypes.contains(&a))
                })
            });
        Some((0..=8).contains(&children_count) && all_valid_archetypes)
    } else {
        None
    };

    RubricScore {
        correct_status,
        correct_evidence_citation,
        no_fabrication,
        appropriate_decomposition,
    }
}
