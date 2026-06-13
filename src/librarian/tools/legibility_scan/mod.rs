//! `librarian(action="legibility_scan")` — runs the Phase-2a legibility engine and
//! reconciles the `docs/trackers/legibility-backlog.md` augmented artifact.
//! Phase 2b of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::librarian::tools::{RecoverableError, Tool, ToolContext};
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
    let project_root = args
        .project
        .clone()
        .unwrap_or_else(|| repo_root.to_string_lossy().into_owned());

    // Index lane — parse ONCE, keep `files` for auto-close re-measurement.
    let files = crate::legibility::parse_project(&repo_root);
    let mut structural = crate::legibility::over_budget_bodies(&files);
    structural.extend(crate::legibility::name_collisions(&files));
    structural.extend(crate::legibility::un_mappable_files(&files));

    // Recorder lane — open_db creates an empty db if absent (graceful degrade).
    let conn = crate::usage::db::open_db(&repo_root)?;
    let friction = crate::legibility::recorder_lane(&conn, &project_root).unwrap_or_default();

    let candidates = crate::legibility::score_and_rank(structural, &friction);
    let mut grouped = group_by_key(candidates);
    if let Some(limit) = args.limit {
        grouped.truncate(limit);
    }

    if !args.write {
        return Ok(build_dry_run(&grouped));
    }

    let today = now_date();
    let (id, rel) = ensure_tracker(ctx).await?;
    let prior = load_backlog(ctx, &id).await.unwrap_or_default();
    let new_rows = reconcile(&prior, &grouped, &files, &today);
    let n_open = new_rows.iter().filter(|r| r.status == "open").count() as u32;
    let n_closed = new_rows.iter().filter(|r| r.status == "closed").count();
    let backlog = BacklogParams {
        candidates: new_rows,
        scan_meta: ScanMeta {
            last_scan_at: Some(today.clone()),
            last_scan_commit: git_head(&repo_root),
            n_candidates: n_open,
            project_root,
        },
    };

    // Tracker-write failure must not fail the whole scan — return results + a note.
    if let Err(e) = write_backlog(ctx, &id, &backlog).await {
        tracing::warn!("legibility_scan: backlog write failed: {e:#}");
        return Ok(json!({
            "ok": true,
            "tracker_error": format!("{e:#}"),
            "open": n_open,
            "closed": n_closed,
        }));
    }

    Ok(json!({
        "ok": true,
        "tracker_id": id,
        "tracker_path": rel,
        "open": n_open,
        "closed": n_closed,
    }))
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
        let g = map
            .entry(c.key.clone())
            .or_insert_with(|| GroupedCandidate {
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
///
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
        let measure = Measure {
            tokens: c.tokens,
            budget: c.budget,
            lines: c.lines,
        };
        let cost = Cost {
            truncations: c.friction.truncations,
            edit_fails: c.friction.code_class_edit_fails,
            sessions: c.friction.sessions,
        };
        let defects: Vec<String> = c
            .defects
            .iter()
            .map(|d| defect_str(*d).to_string())
            .collect();
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
                .map(|(tokens, lines)| Measure {
                    tokens,
                    budget: crate::tools::MAX_INLINE_TOKENS,
                    lines,
                });
        }
    }
    rows
}

const TRACKER_REL_PATH: &str = "docs/trackers/legibility-backlog.md";

async fn ensure_tracker(ctx: &ToolContext) -> Result<(String, String)> {
    let find_args = json!({
        "action": "find",
        "filter": { "rel_path": { "contains": TRACKER_REL_PATH } },
        "include_archived": true
    });
    if let Ok(v) = crate::librarian::tools::find::call(ctx, find_args).await {
        if let Some(first) = v
            .get("items")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
        {
            if let Some(id) = first.get("id").and_then(|x| x.as_str()) {
                return Ok((id.to_string(), TRACKER_REL_PATH.to_string()));
            }
        }
    }
    let project_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| RecoverableError::new("legibility_scan: no active project"))?
        .abs_path
        .clone();
    std::fs::create_dir_all(project_root.join("docs/trackers"))?;
    let empty = serde_json::to_value(BacklogParams::default())?;
    let create_args = json!({
        "action": "create",
        "kind": "tracker",
        "title": "Legibility Backlog",
        "rel_path": TRACKER_REL_PATH,
        "tags": ["legibility", "dzo"],
        "body": "Auto-managed by `librarian(action=\"legibility_scan\")`. Dzo verdicts below the table.\n",
        "augment": { "prompt": include_str!("./render_prompt.md"), "params": empty }
    });
    let created = crate::librarian::tools::create::call(ctx, create_args).await?;
    let id = created
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("artifact create returned no id: {created}"))?
        .to_string();
    let augment_args = json!({
        "id": id,
        "prompt": include_str!("./render_prompt.md"),
        "params": serde_json::to_value(BacklogParams::default())?,
        "render_template": include_str!("./render_template.j2")
    });
    if let Err(e) = crate::librarian::tools::augment::ArtifactAugment
        .call(ctx, augment_args)
        .await
    {
        tracing::warn!("legibility_scan: failed to attach render_template: {e:#}");
    }
    Ok((id, TRACKER_REL_PATH.to_string()))
}

async fn load_backlog(ctx: &ToolContext, id: &str) -> Option<BacklogParams> {
    let v = crate::librarian::tools::get::call(ctx, json!({ "action": "get", "id": id }))
        .await
        .ok()?;
    let params = v.get("augmentation").and_then(|a| a.get("params"))?;
    serde_json::from_value::<BacklogParams>(params.clone()).ok()
}

async fn write_backlog(ctx: &ToolContext, id: &str, params: &BacklogParams) -> Result<()> {
    let augment_args = json!({ "id": id, "merge": true, "params": serde_json::to_value(params)? });
    crate::librarian::tools::augment::ArtifactAugment
        .call(ctx, augment_args)
        .await?;
    Ok(())
}

fn now_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn git_head(root: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn build_dry_run(grouped: &[GroupedCandidate]) -> Value {
    let rows: Vec<Value> = grouped
        .iter()
        .map(|c| {
            json!({
                "key": c.key,
                "defects": c.defects.iter().map(|d| defect_str(*d)).collect::<Vec<_>>(),
                "tier": c.tier.rank(),
                "tokens": c.tokens,
                "budget": c.budget,
                "lines": c.lines,
                "score": c.score,
                "cost": { "truncations": c.friction.truncations,
                          "edit_fails": c.friction.code_class_edit_fails,
                          "sessions": c.friction.sessions },
            })
        })
        .collect();
    json!({ "ok": true, "dry_run": true, "candidates": rows, "n": rows.len() })
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
            tier: if fr.is_empty() {
                Tier::Latent
            } else {
                Tier::BitingNow
            },
            tokens,
            budget: 2500,
            lines: 242,
            friction: fr,
            score,
        }
    }

    #[test]
    fn group_by_key_unions_defects_for_same_target() {
        let fr = Friction {
            truncations: 14,
            ..Default::default()
        };
        let k = "src/lsp/manager.rs::LspManager/get_or_start";
        let cands = vec![
            cand(k, Defect::OverBudgetBody, 4180, 42, fr.clone()),
            cand(k, Defect::NameCollision, 0, 42, fr.clone()),
        ];
        let grouped = group_by_key(cands);
        assert_eq!(grouped.len(), 1, "same key collapses to one row");
        let g = &grouped[0];
        assert_eq!(
            g.defects,
            vec![Defect::OverBudgetBody, Defect::NameCollision]
        );
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
            tier: if fr.is_empty() {
                Tier::Latent
            } else {
                Tier::BitingNow
            },
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
        let g1 = grouped(
            key,
            "Foo/big",
            4180,
            Friction {
                truncations: 14,
                ..Default::default()
            },
        );
        let rows1 = reconcile(&BacklogParams::default(), &[g1], &[], "2026-06-13");
        assert_eq!(rows1.len(), 1);
        assert_eq!(rows1[0].status, "open");
        assert_eq!(rows1[0].before.tokens, 4180);
        assert!(rows1[0].after.is_none());

        // scan 2: refactored under budget → absent from current scan → auto-close
        let prior = BacklogParams {
            candidates: rows1,
            scan_meta: Default::default(),
        };
        let rows2 = reconcile(&prior, &[], &[small_file()], "2026-06-14");
        assert_eq!(rows2.len(), 1, "closed rows stay for history");
        assert_eq!(rows2[0].status, "closed");
        assert_eq!(rows2[0].closed_at.as_deref(), Some("2026-06-14"));
        assert_eq!(rows2[0].before.tokens, 4180, "before preserved");
        let after = rows2[0].after.as_ref().expect("after delta recorded");
        assert!(after.tokens < 2500, "after is the now-sub-budget measure");
    }

    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_smoke_ctx(root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: root.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                umbrella: None,
            })),
        }
    }

    #[tokio::test]
    async fn ensure_tracker_creates_backlog_artifact() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let (id, rel) = ensure_tracker(&ctx).await.unwrap();
        assert!(!id.is_empty());
        assert_eq!(rel, "docs/trackers/legibility-backlog.md");
        let prior = load_backlog(&ctx, &id).await.unwrap_or_default();
        assert!(prior.candidates.is_empty());
    }

    #[tokio::test]
    async fn scan_writes_ranked_backlog_for_a_real_over_budget_body() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        // a real over-budget function in the project
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(tmp.path().join("huge.rs"), src).unwrap();
        // friction on the target
        std::fs::create_dir_all(tmp.path().join(".codescout")).unwrap();
        let conn = crate::usage::db::open_db(tmp.path()).unwrap();
        crate::usage::db::write_record(
            &conn,
            "symbols",
            1,
            "success",
            true,
            None,
            "cs",
            None,
            "s1",
            None,
            None,
            Some("ccs1"),
            Some("huge"),
            Some(3500),
            None,
            Some(&tmp.path().to_string_lossy()),
        )
        .unwrap();
        drop(conn);

        let out = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();
        let id = out
            .get("tracker_id")
            .and_then(|x| x.as_str())
            .expect("tracker_id");
        let backlog = load_backlog(&ctx, id).await.unwrap();
        assert!(
            backlog
                .candidates
                .iter()
                .any(|c| c.name_path.contains("huge") && c.status == "open"),
            "expected an open backlog row for huge: {:?}",
            backlog.candidates
        );
    }

    #[tokio::test]
    async fn missing_usage_db_still_runs_index_lane() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(tmp.path().join("huge.rs"), src).unwrap();
        // NO usage.db rows written at all.
        let out = call(&ctx, json!({ "action": "legibility_scan", "write": false }))
            .await
            .unwrap();
        let cands = out.get("candidates").and_then(|c| c.as_array()).unwrap();
        // present as latent (tier 2 — structural defect, zero friction)
        assert!(
            cands
                .iter()
                .any(|c| c["tier"] == 2 && c["key"].as_str().unwrap().contains("huge")),
            "expected a latent (tier 2) candidate for huge: {cands:?}"
        );
    }

    #[tokio::test]
    async fn end_to_end_scan_creates_then_auto_closes_on_refactor() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let path = tmp.path().join("huge.rs");
        // scan 1: over budget
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(&path, &src).unwrap();
        let out1 = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();
        let id = out1["tracker_id"].as_str().unwrap().to_string();
        let b1 = load_backlog(&ctx, &id).await.unwrap();
        assert!(
            b1.candidates
                .iter()
                .any(|c| c.name_path.contains("huge") && c.status == "open"),
            "scan 1 should open a candidate for huge: {:?}",
            b1.candidates
        );

        // scan 2: refactor under budget (tiny body) → auto-close
        std::fs::write(&path, "fn huge() {\n    let v = 1;\n}\n").unwrap();
        let _out2 = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();
        let b2 = load_backlog(&ctx, &id).await.unwrap();
        let row = b2
            .candidates
            .iter()
            .find(|c| c.name_path.contains("huge"))
            .unwrap();
        assert_eq!(row.status, "closed", "auto-closed after refactor");
        assert!(
            row.after.as_ref().map(|m| m.tokens < 2500).unwrap_or(false),
            "after-delta recorded below budget: {:?}",
            row.after
        );
        assert!(row.before.tokens > 2500, "before preserved");
    }
}
