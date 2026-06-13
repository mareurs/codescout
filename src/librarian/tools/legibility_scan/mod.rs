//! `librarian(action="legibility_scan")` — runs the Phase-2a legibility engine and
//! reconciles the `docs/trackers/legibility-backlog.md` augmented artifact.
//! Phase 2b of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::legibility::{Candidate, Defect, Friction, Tier};
use std::collections::BTreeMap;


fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct LegibilityScanArgs {
    /// Absolute path; defaults to the active project. Scopes the recorder lane.
    #[serde(default)]
    pub project: Option<String>,
    /// true (default) = reconcile the backlog tracker; false = dry-run JSON only.
    #[serde(default = "default_true")]
    pub write: bool,
    /// Cap candidates returned/written.
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: LegibilityScanArgs = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("legibility_scan: bad args: {e}"),
            "see librarian(action=\"legibility_scan\") input schema",
        )
    })?;
    let repo_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| {
            RecoverableError::new("legibility_scan: no active project; activate one first")
        })?
        .abs_path
        .clone();
    let _ = (&args, &repo_root); // wired in later tasks
    Ok(json!({ "ok": true }))
}

/// One backlog target after collapsing its per-defect `Candidate`s. `defects` holds
/// every structural defect on the target (defects-array, not a single dominant
/// defect). Friction is identical across same-key candidates (the recorder lane keys
/// by `name_path`), so it is taken from the first.
pub struct GroupedCandidate {
    pub key: String,
    pub rel_file: String,
    pub name_path: String,
    pub defects: Vec<Defect>,
    pub tier: Tier,
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
    pub friction: Friction,
    pub score: u32,
}

/// Stable defect ordering for deterministic `defects` arrays.
fn defect_rank(d: Defect) -> u8 {
    match d {
        Defect::OverBudgetBody => 0,
        Defect::NameCollision => 1,
        Defect::UnMappableFile => 2,
    }
}

/// Collapse per-defect candidates sharing a key into one target carrying all defects.
/// Output is sorted: tier asc, score desc, tokens desc, key asc.
pub fn group_by_key(cands: Vec<Candidate>) -> Vec<GroupedCandidate> {
    let mut map: BTreeMap<String, GroupedCandidate> = BTreeMap::new();
    for c in cands {
        let g = map.entry(c.key.clone()).or_insert_with(|| GroupedCandidate {
            key: c.key.clone(),
            rel_file: c.rel_file.clone(),
            name_path: c.name_path.clone(),
            defects: Vec::new(),
            tier: c.tier,
            tokens: 0,
            budget: c.budget,
            lines: c.lines,
            friction: c.friction.clone(),
            score: c.score,
        });
        if !g.defects.contains(&c.defect) {
            g.defects.push(c.defect);
        }
        g.tokens = g.tokens.max(c.tokens);
        g.lines = g.lines.max(c.lines);
    }
    let mut out: Vec<GroupedCandidate> = map.into_values().collect();
    for g in &mut out {
        g.defects.sort_by_key(|d| defect_rank(*d));
    }
    out.sort_by(|a, b| {
        a.tier
            .rank()
            .cmp(&b.tier.rank())
            .then(b.score.cmp(&a.score))
            .then(b.tokens.cmp(&a.tokens))
            .then(a.key.cmp(&b.key))
    });
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Measure {
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cost {
    pub truncations: u32,
    pub edit_fails: u32,
    pub sessions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRow {
    pub key: String,
    pub rel_file: String,
    pub name_path: String,
    pub defects: Vec<String>,
    pub tier: u8,
    pub status: String,
    pub measure: Measure,
    pub cost: Cost,
    pub score: u32,
    pub first_seen: String,
    pub before: Measure,
    pub after: Option<Measure>,
    pub closed_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMeta {
    pub last_scan_at: Option<String>,
    pub last_scan_commit: Option<String>,
    pub n_candidates: u32,
    pub project_root: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BacklogParams {
    pub candidates: Vec<CandidateRow>,
    pub scan_meta: ScanMeta,
}

fn defect_str(d: Defect) -> &'static str {
    match d {
        Defect::OverBudgetBody => "over_budget_body",
        Defect::NameCollision => "name_collision",
        Defect::UnMappableFile => "un_mappable_file",
    }
}

/// Reconcile the prior backlog with the current scan. Two passes:
/// 1. upsert every current candidate (update in place / insert new, preserving
///    `first_seen` and `before`; re-open a regressed closed row);
/// 2. auto-close every prior `open` row whose key is absent from the current scan —
///    its defect is gone — recording `after` (re-measured) and `closed_at`.
/// Closed rows are retained for history.
pub fn reconcile(
    prior: &BacklogParams,
    current: &[GroupedCandidate],
    files: &[crate::legibility::FileSymbols],
    today: &str,
) -> Vec<CandidateRow> {
    use std::collections::HashSet;
    let current_keys: HashSet<&str> = current.iter().map(|c| c.key.as_str()).collect();
    let mut rows = prior.candidates.clone();

    for c in current {
        let measure = Measure { tokens: c.tokens, budget: c.budget, lines: c.lines };
        let cost = Cost {
            truncations: c.friction.truncations,
            edit_fails: c.friction.code_class_edit_fails,
            sessions: c.friction.sessions,
        };
        let defects: Vec<String> = c.defects.iter().map(|d| defect_str(*d).to_string()).collect();
        if let Some(row) = rows.iter_mut().find(|r| r.key == c.key) {
            row.defects = defects;
            row.tier = c.tier.rank();
            row.measure = measure;
            row.cost = cost;
            row.score = c.score;
            if row.status == "closed" {
                row.status = "open".to_string(); // regression: defect returned
                row.after = None;
                row.closed_at = None;
            }
        } else {
            rows.push(CandidateRow {
                key: c.key.clone(),
                rel_file: c.rel_file.clone(),
                name_path: c.name_path.clone(),
                defects,
                tier: c.tier.rank(),
                status: "open".to_string(),
                measure: measure.clone(),
                cost,
                score: c.score,
                first_seen: today.to_string(),
                before: measure,
                after: None,
                closed_at: None,
                extra: serde_json::Map::new(),
            });
        }
    }

    for row in rows.iter_mut() {
        if row.status == "open" && !current_keys.contains(row.key.as_str()) {
            row.status = "closed".to_string();
            row.closed_at = Some(today.to_string());
            row.after = crate::legibility::measure_target(files, &row.rel_file, &row.name_path)
                .map(|(tokens, lines)| Measure { tokens, budget: crate::tools::MAX_INLINE_TOKENS, lines });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legibility::{Candidate, Defect, Friction, Tier};

    fn cand(key: &str, defect: Defect, tokens: usize, score: u32, fr: Friction) -> Candidate {
        Candidate {
            key: key.to_string(),
            rel_file: "src/lsp/manager.rs".to_string(),
            name_path: "LspManager/get_or_start".to_string(),
            defect,
            tier: if fr.is_empty() { Tier::Latent } else { Tier::BitingNow },
            tokens,
            budget: 2500,
            lines: 242,
            friction: fr,
            score,
        }
    }

    #[test]
    fn group_by_key_unions_defects_for_same_target() {
        let fr = Friction { truncations: 14, ..Default::default() };
        let k = "src/lsp/manager.rs::LspManager/get_or_start";
        let cands = vec![
            cand(k, Defect::OverBudgetBody, 4180, 42, fr.clone()),
            cand(k, Defect::NameCollision, 0, 42, fr.clone()),
        ];
        let grouped = group_by_key(cands);
        assert_eq!(grouped.len(), 1, "same key collapses to one row");
        let g = &grouped[0];
        assert_eq!(g.defects, vec![Defect::OverBudgetBody, Defect::NameCollision]);
        assert_eq!(g.tokens, 4180, "max structural magnitude across defects");
        assert_eq!(g.tier, Tier::BitingNow);
        assert_eq!(g.score, 42);
    }

    use crate::legibility::FileSymbols;
    use crate::lsp::symbols::{SymbolInfo, SymbolKind};

    fn grouped(key: &str, np: &str, tokens: usize, fr: Friction) -> GroupedCandidate {
        GroupedCandidate {
            key: key.to_string(),
            rel_file: "src/foo.rs".to_string(),
            name_path: np.to_string(),
            defects: vec![Defect::OverBudgetBody],
            tier: if fr.is_empty() { Tier::Latent } else { Tier::BitingNow },
            tokens,
            budget: 2500,
            lines: 242,
            friction: fr,
            score: 42,
        }
    }

    /// A parsed file where `Foo/big` is now a tiny (sub-budget) body, so
    /// measure_target returns an `after` measure below the budget.
    fn small_file() -> FileSymbols {
        let small = SymbolInfo {
            name: "big".to_string(),
            name_path: "Foo/big".to_string(),
            kind: SymbolKind::Method,
            file: std::path::PathBuf::from("x.rs"),
            start_line: 0,
            end_line: 3,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        };
        FileSymbols {
            rel_file: "src/foo.rs".to_string(),
            lines: (0..4).map(|_| "x".repeat(40)).collect(),
            symbols: vec![small],
        }
    }

    #[test]
    fn reconcile_opens_then_auto_closes_with_delta() {
        let key = "src/foo.rs::Foo/big";
        // scan 1: candidate over budget → open, before captured
        let g1 = grouped(key, "Foo/big", 4180, Friction { truncations: 14, ..Default::default() });
        let rows1 = reconcile(&BacklogParams::default(), &[g1], &[], "2026-06-13");
        assert_eq!(rows1.len(), 1);
        assert_eq!(rows1[0].status, "open");
        assert_eq!(rows1[0].before.tokens, 4180);
        assert!(rows1[0].after.is_none());

        // scan 2: refactored under budget → absent from current scan → auto-close
        let prior = BacklogParams { candidates: rows1, scan_meta: Default::default() };
        let rows2 = reconcile(&prior, &[], &[small_file()], "2026-06-14");
        assert_eq!(rows2.len(), 1, "closed rows stay for history");
        assert_eq!(rows2[0].status, "closed");
        assert_eq!(rows2[0].closed_at.as_deref(), Some("2026-06-14"));
        assert_eq!(rows2[0].before.tokens, 4180, "before preserved");
        let after = rows2[0].after.as_ref().expect("after delta recorded");
        assert!(after.tokens < 2500, "after is the now-sub-budget measure");
    }

}
