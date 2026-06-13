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
}
