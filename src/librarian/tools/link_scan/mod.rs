// src/librarian/tools/link_scan/mod.rs
//! `librarian(action="link_scan")` — derive `cites` link edges from prose
//! citations in artifact bodies.
//!
//! Why derived: the catalog's reindex pre-clean (`catalog/artifact.rs`,
//! abs_path-wins upsert) CASCADE-drops a moved artifact's links, so
//! hand-curated edges do not durably survive. Scanner-derived edges are
//! idempotent (INSERT OR IGNORE on the (src,dst,rel) PK) and regenerate on
//! every run — the only population mechanism the substrate's semantics
//! support. DANGLING artifact-id findings double as the detector for the
//! stale-external-citation cost documented in `migrate_v6.rs`.
//!
//! Stages: `extract` (pure, per-body) → `resolve` (pure, corpus-wide policy)
//! → `diff`/`apply` (catalog). `write=false` (default) reports; `write=true`
//! materializes and prunes scanner-owned `cites` edges only.

pub mod diff;
pub mod extract;
pub mod resolve;

use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, Scope};
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::{find as cat_find, links};
use crate::util::fs::RepoPath;

/// Cap on artifacts scanned per run (same spirit as audit_doc_refs's file cap).
const MAX_ARTIFACTS_DEFAULT: usize = 10_000;
/// Caps on findings carried inline in the response.
const FINDINGS_CAP: usize = 50;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    scope: Option<Scope>,
    /// false (default): report only. true: create missing / prune stale
    /// `cites` edges.
    #[serde(default)]
    write: bool,
    /// Cap on artifacts scanned.
    #[serde(default)]
    limit: Option<usize>,
}

/// Push into a findings array unless it is already at [`FINDINGS_CAP`].
/// Totals are counted separately so capped arrays never hide the true
/// distribution (the report-the-verdict-not-the-distribution trap).
fn push_capped(v: &mut Vec<Value>, val: Value) {
    if v.len() < FINDINGS_CAP {
        v.push(val);
    }
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: Args = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("link_scan: bad args: {e}"),
            "see librarian(action=\"link_scan\") input schema",
        )
    })?;

    let effective_scope = args.scope.unwrap_or(Scope::Project);
    let current = ctx.current_project.as_ref();
    let (scoped_filter, applied) =
        apply_scope(None, effective_scope, &ctx.workspace, current.map(|v| &**v))?;

    let git_root = current.map(|c| c.git_root.clone());

    let cat = ctx.catalog.lock();
    let limit = args.limit.unwrap_or(MAX_ARTIFACTS_DEFAULT);
    // Overfetch limit+1 to signal when the artifact scan itself was capped
    // (silent-cap family).
    let mut rows = cat_find::find(
        &cat,
        &cat_find::FindOpts {
            filter: scoped_filter,
            limit: limit + 1,
            offset: 0,
        },
    )?;
    let scan_truncated = rows.len() > limit;
    rows.truncate(limit);

    // ---- extraction pass (one body parse per artifact) ----
    let mut extracts: Vec<(usize, extract::DocExtract)> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match std::fs::read_to_string(&row.abs_path) {
            Ok(text) => extracts.push((i, extract::extract(&text))),
            Err(_) => unreadable.push(row.id.clone()),
        }
    }

    // ---- corpus + definition index ----
    let index = resolve::DefinitionIndex::build(
        extracts
            .iter()
            .map(|(i, ex)| (rows[*i].id.as_str(), rows[*i].status.as_str(), ex)),
    );
    let mut corpus = resolve::Corpus::default();
    for row in &rows {
        corpus.ids.insert(row.id.clone());
        if let Some(root) = &git_root {
            if let Ok(rel) = row.abs_path.strip_prefix(root) {
                corpus
                    .by_rel_path
                    .insert(RepoPath::from(rel).into_string(), row.id.clone());
            }
        }
    }

    // ---- resolution pass ----
    let mut desired: BTreeSet<(String, String)> = BTreeSet::new();
    let mut self_cites = 0usize;
    let mut ambiguous: Vec<Value> = Vec::new();
    let mut dangling: Vec<Value> = Vec::new();
    let mut cross_repo: Vec<Value> = Vec::new();
    let (mut ambiguous_total, mut dangling_total, mut cross_repo_total) = (0usize, 0usize, 0usize);
    let mut citations_total = 0usize;

    for (i, ex) in &extracts {
        let row = &rows[*i];
        let rel_dir = git_root
            .as_ref()
            .and_then(|root| row.abs_path.parent()?.strip_prefix(root).ok())
            .map(|p| RepoPath::from(p).into_string())
            .unwrap_or_default();
        citations_total += ex.citations.len();
        for c in &ex.citations {
            match resolve::resolve(c, &row.id, &rel_dir, &index, &corpus) {
                Some(resolve::Outcome::Edge { dst_id }) => {
                    desired.insert((row.id.clone(), dst_id));
                }
                Some(resolve::Outcome::SelfCite) => self_cites += 1,
                Some(resolve::Outcome::Ambiguous { candidates, total }) => {
                    ambiguous_total += 1;
                    push_capped(
                        &mut ambiguous,
                        json!({
                            "src_id": row.id, "token": c.raw,
                            "line": c.line, "candidates": candidates,
                            "candidates_total": total,
                        }),
                    );
                }
                Some(resolve::Outcome::Dangling) => {
                    dangling_total += 1;
                    push_capped(
                        &mut dangling,
                        json!({
                            "src_id": row.id, "raw": c.raw,
                            "kind": format!("{:?}", c.kind), "line": c.line,
                        }),
                    );
                }
                Some(resolve::Outcome::CrossRepo) => {
                    cross_repo_total += 1;
                    push_capped(
                        &mut cross_repo,
                        json!({
                            "src_id": row.id, "raw": c.raw, "line": c.line,
                        }),
                    );
                }
                None => {} // suppressed noise / foreign-jurisdiction links
            }
        }
    }

    // ---- diff (and apply, in write mode) ----
    let prunable: HashSet<String> = extracts.iter().map(|(i, _)| rows[*i].id.clone()).collect();
    let existing = links::by_rel(&cat, diff::CITES_REL)?;
    let d = diff::diff(&existing, &desired, &prunable);
    let (added, pruned) = if args.write {
        diff::apply(&cat, &d)?;
        (d.to_add.len(), d.stale.len())
    } else {
        (0, 0)
    };

    // Human-reviewable edge lists (capped), with rel_paths for readability.
    let id_to_rel: BTreeMap<&str, String> = rows
        .iter()
        .map(|r| {
            let rel = git_root
                .as_ref()
                .and_then(|root| r.abs_path.strip_prefix(root).ok())
                .map(|p| RepoPath::from(p).into_string())
                .unwrap_or_else(|| r.abs_path.display().to_string());
            (r.id.as_str(), rel)
        })
        .collect();
    let edge_view = |pairs: &[(String, String)]| -> Vec<Value> {
        pairs
            .iter()
            .take(FINDINGS_CAP)
            .map(|(s, t)| {
                json!({
                    "src_id": s, "dst_id": t,
                    "src": id_to_rel.get(s.as_str()),
                    "dst": id_to_rel.get(t.as_str()),
                })
            })
            .collect()
    };

    Ok(json!({
        "scope": applied.to_json(),
        "write": args.write,
        "counts": {
            "artifacts_scanned": extracts.len(),
            "scan_truncated": scan_truncated,
            "unreadable": unreadable.len(),
            "citations": citations_total,
            "self_cites": self_cites,
            "edges_desired": desired.len(),
            "edges_unchanged": d.unchanged,
            "edges_missing": d.to_add.len(),
            "edges_stale": d.stale.len(),
            "edges_added": added,
            "edges_pruned": pruned,
            "ambiguous": ambiguous_total,
            "dangling": dangling_total,
            "cross_repo": cross_repo_total,
        },
        "edges_missing": edge_view(&d.to_add),
        "edges_stale": edge_view(&d.stale),
        "ambiguous": ambiguous,
        "dangling": dangling,
        "cross_repo": cross_repo,
        "unreadable": unreadable,
        "hint": if args.write {
            "edges written as rel=\"cites\" (scanner-owned). Re-run any time — idempotent."
        } else {
            "report only — pass write=true to materialize/prune the cites edges above."
        },
    }))
}
