//! `librarian(action="legibility_scan")` — runs the Phase-2a legibility engine and
//! reconciles the `docs/trackers/legibility-backlog.md` augmented artifact.
//! Phase 2b of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::librarian::tools::{LibrarianRecoverableError, ToolContext};
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
    /// true (default) = reconcile the backlog tracker; false = dry-run JSON only.
    #[serde(default = "default_true")]
    pub write: bool,
    /// Cap candidates returned/written.
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: LegibilityScanArgs = serde_json::from_value(args).map_err(|e| {
        LibrarianRecoverableError::with_hint(
            format!("legibility_scan: bad args: {e}"),
            "see librarian(action=\"legibility_scan\") input schema",
        )
    })?;
    let repo_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| {
            LibrarianRecoverableError::new("legibility_scan: no active project; activate one first")
        })?
        .abs_path
        .clone();
    // Formerly a separate `project` param that re-scoped ONLY the recorder lane's
    // `project_root` filter, leaving the index lane / git head / backlog tracker tied
    // to the active project (`repo_root`) regardless — a split that was never fully
    // wired (dzo-legibility-session-log F-10: `project=` alone silently returned
    // empty candidates because the symbol-index lane ignored it) and had zero real
    // calls across 181,765 recorded uses. The framework's own `workspace=` pin
    // already redirects `repo_root` correctly for EVERY lane (see
    // `LibrarianAdapter::call`'s `active_root` resolution), so `project_root` just
    // follows `repo_root` now — no separate knob, no lane split.
    let project_root = repo_root.to_string_lossy().into_owned();

    // Index lane — parse ONCE, keep `files` for auto-close re-measurement.
    let files = crate::legibility::parse_project(&repo_root);
    let mut structural = crate::legibility::over_budget_bodies(&files);
    structural.extend(crate::legibility::un_mappable_files(&files));

    // Recorder lane — open_db creates an empty db if absent (graceful degrade).
    let conn = crate::usage::db::open_db(&repo_root)?;
    let friction = crate::legibility::recorder_lane(&conn, &project_root).unwrap_or_default();

    let candidates = crate::legibility::score_and_rank(structural, &friction);
    let grouped = group_by_key(candidates);

    if !args.write {
        // `limit` caps the OUTPUT head only — dry-run path.
        let head: &[GroupedCandidate] = match args.limit {
            Some(n) => &grouped[..grouped.len().min(n)],
            None => &grouped,
        };
        let mut dry = build_dry_run(head);
        dry["total_candidates"] = json!(grouped.len());
        dry["truncated"] = json!(grouped.len() > head.len());
        return Ok(dry);
    }

    // NB: `reconcile` ALWAYS receives the full grouped set — never truncated by
    // `limit`. Truncating here would make below-the-cut candidates absent from the
    // current scan and wrongly auto-close them as "defect gone".
    let today = now_date();
    let (id, rel) = ensure_tracker(ctx).await?;
    let prior = load_backlog(ctx, &id).await.unwrap_or_default();
    let new_rows = reconcile(&prior, &grouped, &files, &today);
    let n_open = new_rows.iter().filter(|r| r.status == "open").count() as u32;
    let n_closed = new_rows.iter().filter(|r| r.status == "closed").count();
    let n_retired = new_rows.iter().filter(|r| r.status == "retired").count();
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
            "retired": n_retired,
        }));
    }

    Ok(json!({
        "ok": true,
        "tracker_id": id,
        "tracker_path": rel,
        "open": n_open,
        "closed": n_closed,
        "retired": n_retired,
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
        Defect::UnMappableFile => 1,
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
    /// `open`, `closed` (the defect was repaired, or the target is gone — see
    /// `closed_reason`) or `retired` (the detector that produced the row no longer
    /// exists, so nothing was repaired).
    pub status: String,
    pub measure: Measure,
    pub cost: Cost,
    pub score: u32,
    pub first_seen: String,
    pub before: Measure,
    pub after: Option<Measure>,
    pub closed_at: Option<String>,
    /// Why a non-open row left `open`: `refactored`, `symbol_gone` or
    /// `detector_removed`. Absent on rows closed before the field existed, and a
    /// reader must treat absence as "unknown", never as "refactored".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_reason: Option<String>,
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
        Defect::UnMappableFile => "un_mappable_file",
    }
}
/// Every defect kind the current scan can emit. Keep in step with `Defect` and
/// `defect_str` (a new variant fails to compile in `defect_str`; list it here too).
const LIVE_DEFECTS: [Defect; 2] = [Defect::OverBudgetBody, Defect::UnMappableFile];

/// Is `s` a defect kind a scan can still produce? A persisted row may carry a kind whose
/// detector has since been deleted (`name_collision`, retired 2026-06-13).
fn is_live_defect(s: &str) -> bool {
    LIVE_DEFECTS.iter().any(|d| defect_str(*d) == s)
}

/// Reconcile the prior backlog with the current scan. Two passes:
/// 1. upsert every current candidate (update in place / insert new, preserving
///    `first_seen` and `before`; re-open a regressed closed or retired row);
/// 2. settle every prior `open` row whose key is absent from the current scan.
///    Absence alone proves nothing: a deleted detector empties `current` exactly as a
///    repair does. So the row's own `defects` decide what the absence means —
///    - any defect kind no scan can emit any more -> `retired`, no `after` delta (the
///      detector is gone; nothing was repaired);
///    - every kind still live and the target re-measures -> `closed` / `refactored`,
///      recording `after` and `closed_at`;
///    - every kind still live but the target no longer resolves (renamed, deleted)
///      -> `closed` / `symbol_gone`, no `after`.
///
/// Settled rows are retained for history.
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
            if row.status == "closed" || row.status == "retired" {
                row.status = "open".to_string(); // regression: defect returned
                row.after = None;
                row.closed_at = None;
                row.closed_reason = None;
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
                closed_reason: None,
                extra: serde_json::Map::new(),
            });
        }
    }

    for row in rows.iter_mut() {
        if row.status != "open" || current_keys.contains(row.key.as_str()) {
            continue;
        }
        row.closed_at = Some(today.to_string());
        if !row.defects.iter().all(|d| is_live_defect(d)) {
            row.status = "retired".to_string();
            row.closed_reason = Some("detector_removed".to_string());
            row.after = None;
            continue;
        }
        row.status = "closed".to_string();
        row.after = crate::legibility::measure_target(files, &row.rel_file, &row.name_path).map(
            |(tokens, lines)| Measure {
                tokens,
                budget: crate::tools::MAX_INLINE_TOKENS,
                lines,
            },
        );
        row.closed_reason = Some(
            if row.after.is_some() {
                "refactored"
            } else {
                "symbol_gone"
            }
            .to_string(),
        );
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
        .ok_or_else(|| LibrarianRecoverableError::new("legibility_scan: no active project"))?
        .abs_path
        .clone();
    std::fs::create_dir_all(project_root.join("docs/trackers"))?;
    let empty = serde_json::to_value(BacklogParams::default())?;
    let create_args = json!({
        "action": "create",
        "kind": "tracker",
        "title": "Legibility Backlog",
        "rel_path": TRACKER_REL_PATH,
        "tags": ["codescout", "legibility", "dzo"],
        "body": "## Backlog (auto-managed)\n\n_Pending first scan._\n\n---\n\n## Verdicts (Dzo-owned)\n\n_Per-key triage goes here — classify code-class vs tool-class, name the move, note human-cost. One `### <key>` section per target the Dzo picks up._\n",
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
    if let Err(e) = crate::librarian::tools::augment::call(ctx, augment_args).await {
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

/// Heading that separates the auto-rendered managed region from the Dzo's
/// hand-written verdicts. Everything from this heading to EOF is preserved
/// verbatim across every `write_backlog` render. Fixes F-8.
const VERDICTS_HEADING: &str = "## Verdicts";

/// Fallback used only when a backlog body somehow lacks a verdicts section
/// (e.g. a hand-edit removed it). The normal path preserves the live prose.
const DEFAULT_VERDICTS: &str = "## Verdicts (Dzo-owned)\n\n_Per-key triage goes here — classify code-class vs tool-class, name the move, note human-cost. One `### <key>` section per target the Dzo picks up._";

async fn write_backlog(ctx: &ToolContext, id: &str, params: &BacklogParams) -> Result<()> {
    let params_value = serde_json::to_value(params)?;
    let augment_args = json!({ "id": id, "merge": true, "params": params_value.clone() });
    crate::librarian::tools::augment::call(ctx, augment_args).await?;
    // F-8: project params onto the body's managed region (preserving the Dzo
    // verdicts prose). Best-effort — params is the source of truth, so a render
    // failure must warn, not fail the scan.
    if let Err(e) = render_managed_body(ctx, id, &params_value).await {
        tracing::warn!("legibility_scan: body render failed (params still updated): {e:#}");
    }
    Ok(())
}

/// Project `params` onto the managed region of the backlog body via the
/// attached `render_template`, preserving everything from `VERDICTS_HEADING`
/// onward. Fixes F-8: previously `params` updated but the body stayed stale,
/// forcing a manual re-render after every scan.
async fn render_managed_body(
    ctx: &ToolContext,
    id: &str,
    params: &serde_json::Value,
) -> Result<()> {
    let managed = crate::librarian::tools::render::render_params(
        include_str!("./render_template.j2"),
        params,
    )?;

    let current_body =
        crate::librarian::tools::get::call(ctx, json!({ "action": "get", "id": id, "full": true }))
            .await
            .ok()
            .and_then(|v| v.get("body").and_then(|b| b.as_str()).map(str::to_string))
            .unwrap_or_default();

    let verdicts = match current_body.find(VERDICTS_HEADING) {
        Some(i) => current_body[i..].trim_end().to_string(),
        None => DEFAULT_VERDICTS.to_string(),
    };

    let new_body = format!("{}\n\n---\n\n{}\n", managed.trim_end(), verdicts);

    crate::librarian::tools::update::call(
        ctx,
        json!({ "action": "update", "id": id, "force": true, "patch": { "body": new_body } }),
    )
    .await?;
    Ok(())
}

fn now_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Resolve the full git HEAD SHA for a directory. Returns None if `root` is not
/// a git repo or HEAD is unborn (no commits yet).
///
/// Uses libgit2 (no subprocess): on the locked-down Windows VDI every
/// `CreateProcessW` is taxed by EDR injection, and a raw `git rev-parse HEAD`
/// with no timeout could hang. Mirrors the siblings `agent::resolve_head_sha`
/// (WIN-14) and `probe_has_git_remote`, which already open a libgit2 repo.
/// `Oid::to_string()` is the full 40-char hex, matching `git rev-parse HEAD`.
fn git_head(root: &std::path::Path) -> Option<String> {
    let repo = git2::Repository::open(root).ok()?;
    let head = repo.revparse_single("HEAD").ok()?;
    Some(head.id().to_string())
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
            cand(k, Defect::UnMappableFile, 0, 42, fr.clone()),
        ];
        let grouped = group_by_key(cands);
        assert_eq!(grouped.len(), 1, "same key collapses to one row");
        let g = &grouped[0];
        assert_eq!(
            g.defects,
            vec![Defect::OverBudgetBody, Defect::UnMappableFile]
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
        assert_eq!(rows2[0].closed_reason.as_deref(), Some("refactored"));
        assert_eq!(rows2[0].closed_at.as_deref(), Some("2026-06-14"));
        assert_eq!(rows2[0].before.tokens, 4180, "before preserved");
        let after = rows2[0].after.as_ref().expect("after delta recorded");
        assert!(after.tokens < 2500, "after is the now-sub-budget measure");
    }
    /// One open row for `Foo/big`, built through `reconcile` (so every field is a real
    /// one), then stamped with the given defect strings — the way a row written by an
    /// older detector roster looks in a persisted backlog.
    fn prior_with_defects(defects: &[&str]) -> BacklogParams {
        let g = grouped("src/foo.rs::Foo/big", "Foo/big", 4180, Friction::default());
        let mut rows = reconcile(&BacklogParams::default(), &[g], &[], "2026-06-13");
        rows[0].defects = defects.iter().map(|s| s.to_string()).collect();
        BacklogParams {
            candidates: rows,
            scan_meta: Default::default(),
        }
    }

    /// The claim of the fix: a removed detector and a repaired defect hand `reconcile`
    /// the byte-identical input (`current` empty, same re-measurable file), and the two
    /// must come out as different terminal states. Asserting a label alone would pass
    /// on a rule that stamped everything with it, so the two outcomes are compared.
    #[test]
    fn reconcile_tells_a_retired_detector_from_a_repaired_defect() {
        let files = [small_file()];

        // Detector gone: the row's only defect is a kind no scan can emit any more.
        let retired = reconcile(
            &prior_with_defects(&["name_collision"]),
            &[],
            &files,
            "2026-06-14",
        );
        assert_eq!(retired[0].status, "retired");
        assert_eq!(
            retired[0].closed_reason.as_deref(),
            Some("detector_removed")
        );
        assert!(
            retired[0].after.is_none(),
            "no before->after delta for a repair that never happened: {:?}",
            retired[0].after
        );

        // Positive twin: same absence, but the defect kind is live and the body re-measures
        // small -> a genuine repair, with its delta.
        let repaired = reconcile(
            &prior_with_defects(&["over_budget_body"]),
            &[],
            &files,
            "2026-06-14",
        );
        assert_eq!(repaired[0].status, "closed");
        assert_eq!(repaired[0].closed_reason.as_deref(), Some("refactored"));
        assert!(repaired[0].after.is_some(), "refactor records its delta");

        assert_ne!(
            retired[0].status, repaired[0].status,
            "the two causes must be distinguishable"
        );
    }

    /// A row with one retired kind among live ones is not a repair either: "every defect
    /// is still producible" is the precondition for reading absence as a repair.
    #[test]
    fn reconcile_retires_a_row_carrying_any_retired_defect_kind() {
        let rows = reconcile(
            &prior_with_defects(&["over_budget_body", "name_collision"]),
            &[],
            &[small_file()],
            "2026-06-14",
        );
        assert_eq!(rows[0].status, "retired");
        assert!(rows[0].after.is_none());
    }

    /// The other roster-independent cause: a live-detector row whose target no longer
    /// exists (renamed / deleted) cannot be re-measured, so it is not a "refactored" row
    /// either — and it keeps `status == "closed"` because the detector is not at fault.
    #[test]
    fn reconcile_closes_a_vanished_target_as_symbol_gone() {
        let rows = reconcile(
            &prior_with_defects(&["over_budget_body"]),
            &[],
            &[], // no parsed files: measure_target returns None
            "2026-06-14",
        );
        assert_eq!(rows[0].status, "closed");
        assert_eq!(rows[0].closed_reason.as_deref(), Some("symbol_gone"));
        assert!(rows[0].after.is_none());
        assert_eq!(rows[0].closed_at.as_deref(), Some("2026-06-14"));
    }

    /// Both live kinds count as producible (positive control for the roster: a roster
    /// that rejected `un_mappable_file` would retire every file-level row).
    #[test]
    fn every_producible_defect_kind_is_live_and_a_removed_one_is_not() {
        for d in [Defect::OverBudgetBody, Defect::UnMappableFile] {
            assert!(is_live_defect(defect_str(d)), "{d:?} must be live");
        }
        assert!(!is_live_defect("name_collision"));
        assert!(!is_live_defect(""));
    }

    /// A closed or retired row whose candidate shows up in a later scan is open again
    /// and sheds its terminal reason.
    #[test]
    fn reconcile_reopens_a_regressed_row_and_clears_its_reason() {
        let key = "src/foo.rs::Foo/big";
        for (defects, want) in [
            (&["over_budget_body"][..], "closed"),
            (&["name_collision"][..], "retired"),
        ] {
            let ended = BacklogParams {
                candidates: reconcile(&prior_with_defects(defects), &[], &[small_file()], "d2"),
                scan_meta: Default::default(),
            };
            assert_eq!(ended.candidates[0].status, want);
            assert!(ended.candidates[0].closed_reason.is_some());

            let back = reconcile(
                &ended,
                &[grouped(key, "Foo/big", 4000, Friction::default())],
                &[],
                "d3",
            );
            assert_eq!(back[0].status, "open", "{want} row re-opens");
            assert!(back[0].closed_reason.is_none());
            assert!(back[0].closed_at.is_none());
            assert!(back[0].after.is_none());
            assert_eq!(back[0].defects, vec!["over_budget_body".to_string()]);
        }
    }

    /// Params written before `closed_reason` existed must still load, and must not grow
    /// a `closed_reason` key when written back (absence = "unknown", never "refactored").
    #[test]
    fn legacy_rows_without_closed_reason_load_and_round_trip_unchanged() {
        let legacy = json!({
            "key": "k", "rel_file": "f.rs", "name_path": "a/b",
            "defects": ["name_collision"], "tier": 2, "status": "closed",
            "measure": {"tokens": 0, "budget": 0, "lines": 0},
            "cost": {"truncations": 0, "edit_fails": 0, "sessions": 0},
            "score": 1, "first_seen": "2026-06-01",
            "before": {"tokens": 0, "budget": 0, "lines": 0},
            "after": null, "closed_at": "2026-06-13"
        });
        let row: CandidateRow = serde_json::from_value(legacy).unwrap();
        assert!(row.closed_reason.is_none());
        assert!(
            row.extra.is_empty(),
            "closed_reason must not leak into extra"
        );
        let back = serde_json::to_value(&row).unwrap();
        assert!(back.get("closed_reason").is_none());

        let with_reason = CandidateRow {
            closed_reason: Some("refactored".into()),
            ..row
        };
        let back = serde_json::to_value(&with_reason).unwrap();
        assert_eq!(back["closed_reason"], "refactored");
        let again: CandidateRow = serde_json::from_value(back).unwrap();
        assert_eq!(again.closed_reason.as_deref(), Some("refactored"));
    }

    /// The tracker body must not call a retired row "refactored". Sections are cut at
    /// their headings so a key appearing under the wrong one fails, not just a key
    /// appearing somewhere.
    #[test]
    fn render_files_retired_rows_outside_the_refactored_table() {
        let files = [small_file()];
        let mut cands = Vec::new();
        for (defects, key_tag) in [
            (&["over_budget_body"][..], "refactored"),
            (&["name_collision"][..], "retired"),
        ] {
            let mut p = prior_with_defects(defects);
            p.candidates[0].key = format!("k::{key_tag}");
            cands.extend(reconcile(&p, &[], &files, "2026-06-14"));
        }
        let mut gone = prior_with_defects(&["over_budget_body"]);
        gone.candidates[0].key = "k::symbol_gone".into();
        cands.extend(reconcile(&gone, &[], &[], "2026-06-14"));
        let mut legacy = prior_with_defects(&["name_collision"]);
        legacy.candidates[0].key = "k::legacy".into();
        legacy.candidates[0].status = "closed".into(); // pre-closed_reason row
        legacy.candidates[0].closed_at = Some("2026-06-13".into());
        cands.extend(legacy.candidates);

        let params = BacklogParams {
            candidates: cands,
            scan_meta: Default::default(),
        };
        let md = crate::librarian::tools::render::render_params(
            include_str!("./render_template.j2"),
            &serde_json::to_value(&params).unwrap(),
        )
        .unwrap();

        let section = |start: &str| -> String {
            let from = md
                .find(start)
                .unwrap_or_else(|| panic!("no `{start}` in:\n{md}"));
            let rest = &md[from + start.len()..];
            let to = rest.find("\n### ").unwrap_or(rest.len());
            rest[..to].to_string()
        };
        let refactored = section("### Closed (refactored");
        let gone_s = section("### Closed (target gone");
        let unknown = section("### Closed (reason not recorded");
        let retired = section("### Retired");

        assert!(refactored.contains("k::refactored"));
        assert!(!refactored.contains("k::retired"), "{refactored}");
        assert!(!refactored.contains("k::symbol_gone"), "{refactored}");
        assert!(!refactored.contains("k::legacy"), "{refactored}");
        assert!(gone_s.contains("k::symbol_gone"));
        assert!(unknown.contains("k::legacy"));
        assert!(!unknown.contains("k::retired"));
        assert!(retired.contains("k::retired"));
        assert!(!retired.contains("k::refactored"));
        // the "cleared" claim lives only in the refactored section
        assert!(!md.contains("defects cleared") || refactored.contains("defects cleared"));
    }

    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_smoke_ctx(root: std::path::PathBuf) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                main_root: None,
                umbrella: None,
            }))
            .build()
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
            None,
            None,
            Default::default(),
            None,
            Default::default(),
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

    #[tokio::test]
    async fn limit_does_not_auto_close_below_cut_candidates_on_write() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        // two over-budget functions of different sizes → deterministic ranking
        let big_fn = |n: usize, lines: usize| {
            let mut s = format!("fn huge{n}() {{\n");
            for i in 0..lines {
                s.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
            }
            s.push_str("}\n");
            s
        };
        std::fs::write(tmp.path().join("a.rs"), big_fn(1, 260)).unwrap();
        std::fs::write(tmp.path().join("b.rs"), big_fn(2, 210)).unwrap();

        // scan 1: no limit → both open
        let out1 = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();
        let id = out1["tracker_id"].as_str().unwrap().to_string();
        let b1 = load_backlog(&ctx, &id).await.unwrap();
        assert_eq!(
            b1.candidates.iter().filter(|c| c.status == "open").count(),
            2,
            "scan 1 should open both over-budget fns: {:?}",
            b1.candidates
        );

        // scan 2: limit=1 on the write path must NOT auto-close the below-cut
        // (still over-budget) candidate.
        call(
            &ctx,
            json!({ "action": "legibility_scan", "write": true, "limit": 1 }),
        )
        .await
        .unwrap();
        let b2 = load_backlog(&ctx, &id).await.unwrap();
        assert_eq!(
            b2.candidates
                .iter()
                .filter(|c| c.status == "closed")
                .count(),
            0,
            "limit must not auto-close still-defective candidates: {:?}",
            b2.candidates
        );
        assert_eq!(
            b2.candidates.iter().filter(|c| c.status == "open").count(),
            2,
            "both candidates must remain open: {:?}",
            b2.candidates
        );
    }

    #[tokio::test]
    async fn scan_write_renders_body_and_preserves_verdicts() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        // one real over-budget body → at least one open row to render
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(tmp.path().join("huge.rs"), &src).unwrap();

        let out = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();
        let id = out["tracker_id"].as_str().unwrap().to_string();

        // Inject a hand-written verdict into the prose region, then re-scan.
        crate::librarian::tools::update::call(
            &ctx,
            json!({ "action": "update", "id": id, "force": true, "patch": { "body":
                "## Backlog (auto-managed)\n\n_stale managed region_\n\n---\n\n## Verdicts (Dzo-owned)\n\n### huge — keep me\nDzo says: do not lose this prose.\n" }}),
        )
        .await
        .unwrap();

        let _ = call(&ctx, json!({ "action": "legibility_scan", "write": true }))
            .await
            .unwrap();

        let got = crate::librarian::tools::get::call(
            &ctx,
            json!({ "action": "get", "id": id, "full": true }),
        )
        .await
        .unwrap();
        let body = got.get("body").and_then(|b| b.as_str()).unwrap();

        // managed region re-rendered from params: stale text gone, fresh row in
        assert!(
            body.contains("## Backlog (auto-managed)"),
            "managed header: {body}"
        );
        assert!(
            body.contains("huge") && body.contains("over_budget_body"),
            "rendered open row for huge: {body}"
        );
        assert!(
            !body.contains("_stale managed region_"),
            "stale managed region replaced: {body}"
        );
        // hand-written verdict prose preserved verbatim
        assert!(
            body.contains("### huge — keep me"),
            "verdict heading preserved: {body}"
        );
        assert!(
            body.contains("do not lose this prose"),
            "verdict body preserved: {body}"
        );
    }
}
