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

use crate::librarian::catalog::entry_cite;

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

/// One shape for every finding array: `src_id`, `raw`, `kind`, `line`, plus whatever the
/// arm adds on top.
///
/// This exists because the three arms used to build their own `json!` literal inline, and
/// they diverged. `ambiguous` called the cited text `token` while `dangling` and
/// `cross_repo` called the **identical** `c.raw` value `raw`, and only `dangling` carried
/// `kind` — even though `c.kind` was in scope for all three. No deliberate distinction,
/// just three literals with no shared owner.
///
/// The cost is a query that succeeds while answering half. A grep for `"token":"HY-…"`
/// across a whole report returns nothing from `dangling` and reads as "no HY token is
/// broken" — a zero that describes what was searched, not what is true. That mistake was
/// made on the way to filing the bug this fixes.
///
/// So the fix is a constructor rather than a rename: a rename leaves three literals free
/// to diverge again, and the module had no tests to notice
/// (`docs/issues/archive/2026-08-17-link-scan-names-the-same-field-raw-in-dangling-and-token-in-ambiguous.md`).
fn finding(src_id: &str, c: &extract::Citation, extra: Value) -> Value {
    let mut out = json!({
        "src_id": src_id,
        "raw": c.raw,
        "kind": c.kind.as_str(),
        "line": c.line,
    });
    if let (Some(base), Some(add)) = (out.as_object_mut(), extra.as_object()) {
        for (k, v) in add {
            base.insert(k.clone(), v.clone());
        }
    }
    out
}

/// Push into a findings array unless it is already at [`FINDINGS_CAP`].
/// Totals are counted separately so capped arrays never hide the true
/// distribution (the report-the-verdict-not-the-distribution trap).
fn push_capped(v: &mut Vec<Value>, val: Value) {
    if v.len() < FINDINGS_CAP {
        v.push(val);
    }
}

/// The `entry_cite.dst_ref` for a resolved citation — entry grain where the citation
/// named an entry, file grain where it named a file.
///
/// The two forms are exactly the two `resolve_cite_ref` already accepts on the write
/// path, so scanner rows and hand-written rows are the same shape and a reader never has
/// to know which produced a row:
///
/// - `EntryToken` (`R-43`) and `CrossRepoToken` (`stem:R-43`) → `<dst_slug>:<TOKEN>`,
///   the qualifier stripped because it is a *lookup* hint, not part of the identity.
/// - `ArtifactId` and `RelPathLink` → the bare 16-hex artifact id. These name a FILE;
///   inventing an entry for them would fabricate provenance the citation never claimed.
///
/// **Deliberately not `resolve_cite_ref`.** That function validates a `<slug>:<local>`
/// by looking the local up in the destination's augmentation `entry_collection` — but
/// most ledgers are prose, defining entries by heading with no params rows at all, and
/// `link_scan` binds tokens to headings. Routing scan rows through it would reject every
/// prose ledger while looking like a safety check. Validation here comes from
/// `resolve::resolve`, which yields `Edge` only for a uniquely-resolving token.
///
/// `None` when the destination has no slug — such a row cannot be keyed, since
/// `entry_cite.src_slug` FKs `artifact(slug)`.
fn entry_dst_ref(
    c: &extract::Citation,
    dst_id: &str,
    id_to_slug: &BTreeMap<String, String>,
) -> Option<String> {
    match c.kind {
        extract::CitationKind::EntryToken | extract::CitationKind::CrossRepoToken => {
            let token = match c.raw.rsplit_once(':') {
                Some((_, t)) => t,
                None => c.raw.as_str(),
            };
            let dst_slug = id_to_slug.get(dst_id)?;
            Some(format!("{dst_slug}:{token}"))
        }
        extract::CitationKind::ArtifactId | extract::CitationKind::RelPathLink => {
            Some(dst_id.to_string())
        }
        extract::CitationKind::MalformedQualifier => {
            // Never reached: `resolve::resolve` never yields `Edge` or `SelfCite` for
            // this kind (it is always report-only — see its `MalformedQualifier` arm),
            // so `attribute_entry_edge` never calls this function with one.
            // Exhaustiveness only.
            None
        }
    }
}

/// Entry-grain attribution for one already-resolved citation.
///
/// Returns every `(src_slug, src_local, dst_ref)` triple the citation keys — one per
/// DISTINCT entry that mentions the token — or an empty vec when it can be attributed to
/// no entry at all. The caller counts a non-empty result as one `attributed` citation and
/// an empty one as `outside_any_entry`, which keeps those two a partition of the resolved
/// citations even though a single citation may now yield several edges.
///
/// **Every occurrence is walked, not just the first.** `extract` emits one `Citation` per
/// `(kind, raw)` per document carrying all its positions, because a token's first mention
/// is routinely a preamble line or a hand-maintained `## Index` row while the entries that
/// genuinely rest on it cite it further down. Reading `c.line` alone attributed to whatever
/// contained the first mention and dropped the rest
/// (`docs/issues/archive/2026-08-21-entry-attribution-follows-the-first-mention-only.md`).
///
/// **Both `Edge` and `SelfCite` come through here, and that is the point.** File grain and
/// entry grain disagree about self-citation, correctly: an artifact citing itself is a
/// self-loop and no edge, while two entries in one ledger are two nodes, so `**Kin:** R-3`
/// written inside `## R-41` is a genuine edge. The caller keeps the file-grain half of that
/// distinction by not inserting a `SelfCite` into `desired`; this function keeps the
/// entry-grain half by attributing it anyway.
///
/// The one case a same-file citation must still be refused is the **true** self-reference:
/// the citation sits inside the very entry that defines the token, so `src_local ==
/// dst_local`. Nothing rests on an entry naming itself, and a self-loop at entry grain
/// would inflate that entry's own indegree. Refused per-occurrence, so an entry that names
/// itself AND is named by a sibling still records the sibling's edge.
fn attribute_entry_edge(
    sections: &[extract::EntrySection],
    c: &extract::Citation,
    src_id: &str,
    dst_id: &str,
    id_to_slug: &BTreeMap<String, String>,
) -> Vec<(String, String, String)> {
    let (Some(src_slug), Some(dst_ref)) =
        (id_to_slug.get(src_id), entry_dst_ref(c, dst_id, id_to_slug))
    else {
        // A slugless endpoint cannot key an entry_cite row (`src_slug` FKs
        // `artifact(slug)`), so no occurrence of this citation can be attributed.
        return Vec::new();
    };
    let dst_local = dst_ref.rsplit_once(':').map(|(_, local)| local);

    let mut out: Vec<(String, String, String)> = Vec::new();
    for line in c.occurrences() {
        let Some(src_section) = extract::entry_section_at(sections, line) else {
            continue;
        };
        if src_id == dst_id {
            match dst_local {
                // Intra-ledger, two different entries: a real edge.
                Some(local) if local != src_section.id.as_str() => {}
                // Either the entry citing itself, or a file-grain self-link
                // (`ArtifactId` / `RelPathLink`) whose `dst_ref` names no entry at all —
                // `None` lands here deliberately, since "no entry named" is not
                // "a different entry".
                _ => continue,
            }
        }
        let triple = (src_slug.clone(), src_section.id.clone(), dst_ref.clone());
        // Two mentions inside ONE entry are one edge.
        //
        // This dedup is NOT observable in any reported count, and saying so is the
        // point: `desired_entry` is a BTreeSet that collapses duplicates anyway, and
        // `attributed` counts citations rather than triples. Removing it is an
        // equivalent mutation — confirmed by applying it, 70/70 still green. It is kept
        // only to bound the vec for a token repeated many times inside one entry, and a
        // reader should not infer that any number moves when it fires.
        if !out.contains(&triple) {
            out.push(triple);
        }
    }
    out
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
    // Worktree overlay: never scan another session's shadow rows (an in-repo
    // worktree sits under the main checkout's own prefix), and never scan a
    // main twin this session's worktree supersedes. link_scan WRITES edges, so
    // an undeduped pair does not merely read twice — it materializes `cites`
    // edges out of both copies.
    // docs/issues/archive/2026-08-15-context-and-state-at-never-dedup-the-worktree-overlay.md
    let (shadowed_mains, worktree_exclusions) = {
        let cat = ctx.catalog.lock();
        (
            crate::librarian::tools::worktree::shadowed_main_ids(&cat, current.map(|v| &**v))?,
            crate::librarian::tools::worktree::overlay_exclusions(&cat, current.map(|v| &**v))?,
        )
    };
    let (scoped_filter, applied) = apply_scope(
        None,
        effective_scope,
        &ctx.workspace,
        current.map(|v| &**v),
        &worktree_exclusions,
    )?;

    let git_root = current.map(|c| c.git_root.clone());

    let cat = ctx.catalog.lock();
    let limit = args.limit.unwrap_or(MAX_ARTIFACTS_DEFAULT);
    let cutoff_ms = crate::librarian::catalog::gc::visibility_cutoff_ms(
        &cat.conn,
        chrono::Utc::now().timestamp_millis(),
    )?;
    // Overfetch limit+1 to signal when the artifact scan itself was capped
    // (silent-cap family).
    let mut rows = cat_find::find(
        &cat,
        &cat_find::FindOpts {
            filter: scoped_filter,
            limit: limit + 1,
            offset: 0,
        },
        cutoff_ms,
    )?;
    rows.retain(|r| !shadowed_mains.contains(r.id.as_str()));
    let scan_truncated = rows.len() > limit;
    rows.truncate(limit);

    // ---- extraction pass (one body parse per artifact) ----
    // `entry_sections` is computed HERE, beside `extract`, because this is the only
    // place holding the body text. Entry-grain attribution needs to know which entry
    // encloses each citation's line, and re-reading every file later to answer that
    // would double the I/O and risk the two passes seeing different bytes.
    let mut extracts: Vec<(usize, extract::DocExtract, Vec<extract::EntrySection>)> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match std::fs::read_to_string(&row.abs_path) {
            Ok(text) => {
                let sections = extract::entry_sections(&text);
                extracts.push((i, extract::extract(&text), sections));
            }
            Err(_) => unreadable.push(row.id.clone()),
        }
    }

    // Artifact id -> slug. `ArtifactRow` does not carry `slug`, and `entry_cite`
    // is keyed by it on both sides (`src_slug` FKs `artifact(slug)`), so the map is
    // fetched once rather than per citation.
    let id_to_slug: BTreeMap<String, String> = {
        let mut stmt = cat
            .conn
            .prepare("SELECT id, slug FROM artifact WHERE slug IS NOT NULL")?;
        let pairs = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
        pairs.into_iter().collect()
    };

    // ---- corpus + definition index ----
    let index = resolve::DefinitionIndex::build(
        extracts
            .iter()
            .map(|(i, ex, _)| (rows[*i].id.as_str(), rows[*i].status.as_str(), ex)),
    );
    // Computed here rather than during resolution because it is a fact about the INDEX, not
    // about any one citation — and it must be reported even when every citation currently
    // resolves, which is exactly the `T` case. Artifact ids, consistent with `src_id` and
    // `dst_ref` in the arms below; resolve one with `artifact(action="get", id=…)`.
    let prefix_conflicts = index.prefix_conflicts();
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
        // Qualifier vocabulary for `<stem>:<TOKEN>` citations. Pushed rather than
        // inserted: stems collide across directories, and the resolver reports a
        // collision instead of picking one.
        if let Some(stem) = row.abs_path.file_stem().and_then(|s| s.to_str()) {
            corpus
                .by_stem
                .entry(stem.to_string())
                .or_default()
                .push(row.id.clone());
        }
    }

    // Artifact id -> rel_path, needed both for the human-reviewable edge lists below
    // AND (moved up from there) to key the by-source breakdowns in the resolution
    // pass — a triager reading `ambiguous_by_source` / `dangling_by_source` needs a
    // path, not an id, to recognize "that's a guide explaining the syntax."
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

    // ---- resolution pass ----
    let mut desired: BTreeSet<(String, String)> = BTreeSet::new();
    let mut self_cites = 0usize;
    let mut ambiguous: Vec<Value> = Vec::new();
    let mut dangling: Vec<Value> = Vec::new();
    let mut cross_repo: Vec<Value> = Vec::new();
    let mut malformed_qualifier: Vec<Value> = Vec::new();
    let (mut ambiguous_total, mut dangling_total, mut cross_repo_total) = (0usize, 0usize, 0usize);
    let mut malformed_qualifier_total = 0usize;
    // Per-source counts for the same three arms, uncapped (unlike the `ambiguous` /
    // `dangling` / `cross_repo` finding arrays above, which cap at FINDINGS_CAP) —
    // the whole point is to make the TOTAL interpretable, so a source contributing
    // past the cap must still show its true count.
    // docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md
    let mut ambiguous_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut dangling_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut cross_repo_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut malformed_qualifier_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut citations_total = 0usize;

    // Entry-grain edges: (src_slug, src_local, dst_ref). A set because one entry citing
    // one target twice is ONE edge, and because a stable order makes the emitted sample
    // diffable across runs.
    let mut desired_entry: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Citations that resolved to an edge but sit outside every entry section — a
    // preamble, a trailing `## Summary` that defines nothing, or (the bulk of them, 1397
    // of 1719 measured on this corpus) a document that is not a ledger at all and
    // defines no entries. Counted rather than dropped silently: entry-grain provenance
    // exists only where the CITING document is itself a ledger, which makes the entry
    // graph sparse on the source side in a way the citation graph is not.
    let mut edges_outside_any_entry = 0usize;
    // Citations that DID land inside an entry, counted per citation.
    //
    // Reported beside `derived` because the two are otherwise silently incomparable:
    // `derived` is the size of a DEDUPLICATED set of (src_slug, src_local, dst_ref)
    // triples, and publishing only that set size next to a per-citation counter invites
    // the ratio `derived / (derived + outside_any_entry)`, which divides edges by
    // citations and means nothing. With both present, `attributed` vs
    // `outside_any_entry` partitions the citations and `attributed` vs `derived` shows
    // the collapse — each comparison between like and like.
    //
    // The two differ less than one would guess, because `extract` ALREADY dedupes:
    // `push_citation` keeps one `Citation` per `(kind, raw)` per document. So repeating
    // one token inside one entry does not inflate `attributed`. What still collapses
    // here is a bare token and its stem-qualified twin — `R-43` and
    // `patterns:R-43` are different `raw` values that `entry_dst_ref` maps onto the same
    // `<dst_slug>:R-43`, so one entry citing both records one edge and two attributions.
    let mut edges_attributed = 0usize;

    for (i, ex, sections) in &extracts {
        let row = &rows[*i];
        let rel_dir = git_root
            .as_ref()
            .and_then(|root| row.abs_path.parent()?.strip_prefix(root).ok())
            .map(|p| RepoPath::from(p).into_string())
            .unwrap_or_default();
        citations_total += ex.citations.len();
        let src_rel = id_to_rel
            .get(row.id.as_str())
            .cloned()
            .unwrap_or_else(|| row.id.clone());
        for c in &ex.citations {
            match resolve::resolve(c, &row.id, &rel_dir, &index, &corpus) {
                Some(resolve::Outcome::Edge { dst_id }) => {
                    // Entry grain, derived from the SAME resolution — assembly only.
                    // `resolve` already proved the target unique, so nothing here
                    // re-decides what a citation points at.
                    let triples = attribute_entry_edge(sections, c, &row.id, &dst_id, &id_to_slug);
                    if triples.is_empty() {
                        edges_outside_any_entry += 1;
                    } else {
                        // Citation grain: ONE attributed citation however many entries
                        // mention the token, so this stays a partition with
                        // `outside_any_entry` over the resolved citations. The edge count
                        // is `derived`, below.
                        edges_attributed += 1;
                        desired_entry.extend(triples);
                    }
                    desired.insert((row.id.clone(), dst_id));
                }
                Some(resolve::Outcome::SelfCite { dst_id }) => {
                    self_cites += 1;
                    // FILE grain: deliberately no `desired.insert` — an artifact citing
                    // itself is a self-loop, and excluding it is load-bearing for
                    // exposure (`doctor::entry_indegree`: an entry's own `## Index` row
                    // must not inflate its own reach).
                    //
                    // ENTRY grain: still possibly an edge. `attribute_entry_edge` keeps
                    // the intra-ledger case and refuses only the true self-reference.
                    // Before this split, every `**Kin:**` and `**Chain.**` cross-reference
                    // a ledger wrote about its own entries was discarded here, unattributed
                    // and uncounted.
                    let triples = attribute_entry_edge(sections, c, &row.id, &dst_id, &id_to_slug);
                    if triples.is_empty() {
                        edges_outside_any_entry += 1;
                    } else {
                        edges_attributed += 1;
                        desired_entry.extend(triples);
                    }
                }
                Some(resolve::Outcome::Ambiguous { candidates, total }) => {
                    ambiguous_total += 1;
                    *ambiguous_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    push_capped(
                        &mut ambiguous,
                        finding(
                            &row.id,
                            c,
                            json!({"candidates": candidates, "candidates_total": total}),
                        ),
                    );
                }
                Some(resolve::Outcome::Dangling) => {
                    dangling_total += 1;
                    *dangling_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    push_capped(&mut dangling, finding(&row.id, c, json!({})));
                }
                Some(resolve::Outcome::CrossRepo) => {
                    cross_repo_total += 1;
                    *cross_repo_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    push_capped(&mut cross_repo, finding(&row.id, c, json!({})));
                }
                Some(resolve::Outcome::MalformedQualifier) => {
                    malformed_qualifier_total += 1;
                    *malformed_qualifier_by_source
                        .entry(src_rel.clone())
                        .or_insert(0) += 1;
                    push_capped(&mut malformed_qualifier, finding(&row.id, c, json!({})));
                }
                None => {} // suppressed noise / foreign-jurisdiction links
            }
        }
    }

    // ---- diff (and apply, in write mode) ----
    let prunable: HashSet<String> = extracts
        .iter()
        .map(|(i, _, _)| rows[*i].id.clone())
        .collect();
    let existing = links::by_rel(&cat, diff::CITES_REL)?;
    let d = diff::diff(&existing, &desired, &prunable);
    let (added, pruned) = if args.write {
        diff::apply(&cat, &d)?;
        (d.to_add.len(), d.stale.len())
    } else {
        (0, 0)
    };

    // ---- entry-grain materialization (write mode only) ----
    // Prune-then-re-derive rather than diff: scanner rows are wholly a function of the
    // prose, so re-deriving is the same work as computing a delta and cannot leave a
    // stale row behind. Scoped to the slugs of the artifacts THIS pass extracted, never
    // a bare `origin='scan'` sweep — see `entry_cite::prune_scan_rows`.
    let mut entry_report = entry_cite::MaterializeReport {
        derived: desired_entry.len(),
        ..Default::default()
    };
    let mut entry_pruned = 0usize;
    if args.write {
        let scanned_slugs: std::collections::BTreeSet<String> = extracts
            .iter()
            .filter_map(|(i, _, _)| id_to_slug.get(&rows[*i].id).cloned())
            .collect();
        let now = chrono::Utc::now().timestamp_millis();
        let tx = cat.conn.unchecked_transaction()?;
        entry_pruned = entry_cite::prune_scan_rows(&tx, &scanned_slugs)?;
        for (src_slug, src_local, dst_ref) in &desired_entry {
            let wrote = entry_cite::insert_with(
                &tx,
                &entry_cite::EntryCiteRow {
                    src_slug: src_slug.clone(),
                    src_local: src_local.clone(),
                    dst_ref: dst_ref.clone(),
                    rel: diff::CITES_REL.to_string(),
                    origin: entry_cite::ORIGIN_SCAN.to_string(),
                    created_at: now,
                },
            )?;
            // 0 means an existing row already covers this edge — almost always an
            // `origin='write'` row the scan must not clobber, because `origin` is not
            // in the PK. Counting calls instead of rows is the reporting defect
            // `statement-validity-session-log:F-5` names.
            entry_report.written += wrote;
        }
        entry_report.skipped_existing = entry_report.derived - entry_report.written;
        tx.commit()?;
    }

    // Human-reviewable edge lists (capped), with rel_paths for readability.
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
            "entry_edges": {
                // Citation-grain: these two partition the resolved citations.
                "attributed": edges_attributed,
                "outside_any_entry": edges_outside_any_entry,
                // Edge-grain: `derived` is `attributed` deduplicated by
                // (src_slug, src_local, dst_ref); `written` + `skipped_existing`
                // partition it. Do not compare across the two groups.
                "derived": entry_report.derived,
                "written": entry_report.written,
                "skipped_existing": entry_report.skipped_existing,
                "pruned": entry_pruned,
            },
            "edges_desired": desired.len(),
            "edges_unchanged": d.unchanged,
            "edges_missing": d.to_add.len(),
            "edges_stale": d.stale.len(),
            "edges_added": added,
            "edges_pruned": pruned,
            "ambiguous": ambiguous_total,
            "dangling": dangling_total,
            "cross_repo": cross_repo_total,
            // Shape-level, not a lookup failure: a citation with 2+ qualifier segments
            // before its entry token (`<repo>:<file-stem>:<ID>`) is malformed regardless
            // of corpus contents — see `resolve::Outcome::MalformedQualifier`.
            "malformed_qualifier": malformed_qualifier_total,
            "prefix_conflicts": prefix_conflicts.len(),
            // `len(dangling) == FINDINGS_CAP` reads identically whether the true count is
            // exactly the cap or 100x it — this states which arrays actually got cut, so a
            // reader never has to compare a count against an array length to find out.
            "truncated": {
                "ambiguous": ambiguous_total > ambiguous.len(),
                "dangling": dangling_total > dangling.len(),
                "cross_repo": cross_repo_total > cross_repo.len(),
                "malformed_qualifier": malformed_qualifier_total > malformed_qualifier.len(),
            },
        },
        "edges_missing": edge_view(&d.to_add),
        "edges_stale": edge_view(&d.stale),
        "ambiguous": ambiguous,
        "dangling": dangling,
        "cross_repo": cross_repo,
        "malformed_qualifier": malformed_qualifier,
        // Per-source breakdown of the three arms above, uncapped — the "attribute and
        // subtract" reading: a triager checks which keys are guides/conventions docs
        // explaining citation syntax (rather than genuinely broken references) and
        // discounts those before trusting the raw total in `counts`.
        "ambiguous_by_source": ambiguous_by_source,
        "dangling_by_source": dangling_by_source,
        "cross_repo_by_source": cross_repo_by_source,
        "malformed_qualifier_by_source": malformed_qualifier_by_source,
        // Latent rather than broken, so it sits beside the citation arms rather than in
        // one: a declared namespace with a second active definer has no failing citation
        // *yet*, and the arms above only ever report a citation that already resolves
        // wrong. Unbounded on purpose — it fires once on this corpus, and a report that
        // capped a single-digit list would hide the whole finding to save nothing.
        "prefix_conflicts": prefix_conflicts,
        "unreadable": unreadable,
        "hint": if args.write {
            "edges written as rel=\"cites\" (scanner-owned). Re-run any time — idempotent."
        } else {
            "report only — pass write=true to materialize/prune the cites edges above."
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::link_scan::extract::{Citation, CitationKind};
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;

    /// Write a ledger to disk, seed its catalog row, and give it a slug — all three are
    /// needed: `link_scan` reads the FILE for citations, the CATALOG for the abs_path to
    /// read it from, and the SLUG to key `entry_cite` on either side.
    fn seed_scan_artifact(cat: &Catalog, id: &str, path: &std::path::Path, slug: &str) {
        let row = TestArtifactRowBuilder::new(id).with_abs_path(path).build();
        art_upsert(cat, &row).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET slug=?1 WHERE id=?2",
                rusqlite::params![slug, id],
            )
            .unwrap();
    }

    fn cite(raw: &str) -> Citation {
        Citation {
            raw: raw.to_string(),
            kind: CitationKind::EntryToken,
            line: 7,
            repeat_lines: Vec::new(),
        }
    }

    fn slug_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn entry_dst_ref_uses_entry_grain_for_tokens_and_file_grain_for_files() {
        let map = slug_map(&[("dstid0000000000", "target-tracker")]);

        // A bare entry token names an ENTRY: `<dst_slug>:<TOKEN>`.
        let c = Citation {
            raw: "R-43".into(),
            kind: CitationKind::EntryToken,
            line: 7,
            repeat_lines: Vec::new(),
        };
        assert_eq!(
            entry_dst_ref(&c, "dstid0000000000", &map).unwrap(),
            "target-tracker:R-43"
        );

        // A stem-qualified token names the SAME entry — the qualifier is a lookup hint,
        // not part of the identity, so it must not survive into dst_ref. Keeping it
        // would make `stem:R-43` and `R-43` two different edges to one entry.
        let c = Citation {
            raw: "reconnaissance-patterns:R-43".into(),
            kind: CitationKind::CrossRepoToken,
            line: 7,
            repeat_lines: Vec::new(),
        };
        assert_eq!(
            entry_dst_ref(&c, "dstid0000000000", &map).unwrap(),
            "target-tracker:R-43"
        );

        // An artifact id or a rel_path link names a FILE. Inventing an entry for it
        // would fabricate provenance the citation never claimed.
        for kind in [CitationKind::ArtifactId, CitationKind::RelPathLink] {
            let c = Citation {
                raw: "whatever".into(),
                kind,
                line: 7,
                repeat_lines: Vec::new(),
            };
            assert_eq!(
                entry_dst_ref(&c, "dstid0000000000", &map).unwrap(),
                "dstid0000000000",
                "{kind:?} names a file, so dst_ref is the bare artifact id"
            );
        }
    }

    #[test]
    fn entry_dst_ref_is_none_when_the_destination_has_no_slug() {
        // `entry_cite.src_slug` FKs `artifact(slug)`; a slugless endpoint cannot key a
        // row. Empty after the Layer 3a backfill, but a row created since and not yet
        // minted must degrade to "no entry edge" rather than error or fabricate one.
        let c = Citation {
            raw: "R-43".into(),
            kind: CitationKind::EntryToken,
            line: 7,
            repeat_lines: Vec::new(),
        };
        assert!(entry_dst_ref(&c, "unminted0000000", &BTreeMap::new()).is_none());
    }

    #[tokio::test]
    async fn entry_edges_reports_citation_grain_and_edge_grain_separately() {
        // The two grains must each be internally comparable:
        //   attributed + outside_any_entry  = resolved citations
        //   derived                          = attributed, collapsed by dst_ref
        // A reader who compares ACROSS the groups gets a meaningless ratio, which is
        // why both are emitted rather than just the set size.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.path().join("target.md");
        std::fs::write(
            &dst,
            "## F-1 — target entry\n\nbody\n\n## F-2 — other entry\n\nbody\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        // W-1 cites F-1 both bare and stem-qualified — two distinct `raw` values, so
        // `extract` keeps both, but `entry_dst_ref` maps them onto one `target:F-1`.
        // The preamble cites F-2, which belongs to no entry.
        let src = tmp.path().join("source.md");
        std::fs::write(
            &src,
            "preamble mentions F-2\n\
             ## W-1 — citing entry\n\
             bare F-1 and qualified target:F-1\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(
            e["attributed"],
            json!(2),
            "both the bare and the qualified citation sit inside W-1: {out:#?}"
        );
        assert_eq!(
            e["outside_any_entry"],
            json!(1),
            "the preamble citation belongs to the file, not an entry: {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(1),
            "the qualifier is stripped, so both attributions collapse to ONE edge — \
             this is why `derived` cannot be compared to a per-citation count: {out:#?}"
        );
    }

    #[tokio::test]
    async fn a_token_first_mentioned_outside_an_entry_still_attributes_to_the_entry_citing_it() {
        // This test previously pinned the OPPOSITE, as a known limitation:
        // `push_citation` kept one Citation per (kind, raw) per document carrying only the
        // FIRST occurrence's line, so a passing mention in a preamble or a hand-maintained
        // `## Index` row consumed the citation and the entry that genuinely rested on the
        // token recorded nothing. Measured floor at the time: 799 self-cite citations
        // reaching attribution and failing it, almost all index-table mentions.
        //
        // `Citation` now carries `repeat_lines` and attribution walks `occurrences()`.
        // docs/issues/archive/2026-08-21-entry-attribution-follows-the-first-mention-only.md
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.path().join("target.md");
        std::fs::write(&dst, "## F-1 — target entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let src = tmp.path().join("source.md");
        std::fs::write(
            &src,
            "preamble mentions F-1 in passing\n\
             ## W-1 — the entry that actually rests on it\n\
             this is the real dependency on F-1\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(
            out["counts"]["citations"],
            json!(1),
            "STILL one Citation for the two mentions — the exposure guarantee, not an \
             accident. `doctor::entry_indegree` increments once per Citation and derives \
             its file-level count from that, so emitting one per occurrence would have \
             moved a metric three shipped checks are gated on: {out:#?}"
        );
        assert_eq!(
            e["attributed"],
            json!(1),
            "one citation, attributed — the partition counts citations, not occurrences: \
             {out:#?}"
        );
        assert_eq!(
            e["outside_any_entry"],
            json!(0),
            "the preamble mention no longer consumes it: {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(1),
            "W-1's genuine dependency on F-1 records its edge: {out:#?}"
        );
    }

    #[tokio::test]
    async fn one_citation_attributes_to_every_entry_that_mentions_the_token() {
        // The capability the first-mention fix actually buys: a token cited from THREE
        // entries yields three edges from ONE Citation. A ledger's `**Kin:**` lines
        // converge on the same handful of tokens, so this is the common shape rather than
        // an edge case — and before the fix at most one of the three could ever be
        // recorded, whichever happened to appear first in the file. Here the `## Index`
        // row appears first, which is precisely the arrangement that made the loss
        // systematic: 107 shadowed citations in reconnaissance-patterns.md alone.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.path().join("target.md");
        std::fs::write(&dst, "## F-1 — the widely-cited entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let src = tmp.path().join("source.md");
        std::fs::write(
            &src,
            "## Index\n\
             a row naming F-1 near the top, as ledgers do\n\
             ## W-1 — first dependent\n\
             rests on F-1\n\
             ## W-2 — second dependent\n\
             also rests on F-1\n\
             ## W-3 — third dependent\n\
             and F-1 again, twice: F-1\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(
            out["counts"]["citations"],
            json!(1),
            "five mentions, one Citation — exposure unmoved: {out:#?}"
        );
        assert_eq!(
            e["attributed"],
            json!(1),
            "citation grain: ONE attributed citation however many entries it reaches, so \
             `attributed` + `outside_any_entry` stays a partition of the resolved \
             citations. Counting per-occurrence here would make the two incomparable: \
             {out:#?}"
        );
        assert_eq!(
            e["outside_any_entry"],
            json!(0),
            "the citation reached at least one entry, so it is not in the other half of \
             the partition — the `## Index` mention alone would not have been enough: \
             {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(3),
            "edge grain: W-1, W-2 and W-3 each record their own edge. W-3 mentions F-1 \
             TWICE and still contributes exactly one — two mentions inside one entry are \
             one claim. `## Index` is not an entry, so its row attributes to nothing: \
             {out:#?}"
        );
    }

    #[tokio::test]
    async fn an_entry_citing_a_sibling_in_its_own_ledger_records_an_edge() {
        // The intra-ledger case, which `resolve` classifies `SelfCite` because the CITING
        // FILE defines the token. That verdict is right at file grain — an artifact citing
        // itself is a self-loop — and wrong at entry grain, where F-1 and F-2 are two
        // distinct nodes and `**Kin:**`/`**Chain.**` lines between them are the densest,
        // most deliberate edges a ledger has.
        //
        // Before the split, `SelfCite` short-circuited before `entry_section_at` ran, so
        // EVERY intra-ledger edge was discarded, unattributed and uncounted. The whole
        // 4389-test suite was green in that state.
        // docs/issues/archive/2026-08-21-selfcite-is-file-grain-so-intra-ledger-entry-edges-never-materialize.md
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — the sibling that is cited\n\
             \n\
             body\n\
             \n\
             ## F-2 — the entry that rests on it\n\
             \n\
             this rests on F-1\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "led", &p, "ledger");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(
            out["counts"]["self_cites"],
            json!(1),
            "the file-grain verdict is unchanged — this IS a self-cite at file grain: {out:#?}"
        );
        assert_eq!(
            out["counts"]["edges_desired"],
            json!(0),
            "and it must still create NO file-grain edge; a `cites` row to itself is a \
             self-loop, and excluding it is load-bearing for `entry_indegree` exposure: \
             {out:#?}"
        );
        assert_eq!(
            e["attributed"],
            json!(1),
            "but at ENTRY grain the citation sits inside F-2 and names F-1: {out:#?}"
        );
        assert_eq!(
            e["outside_any_entry"],
            json!(0),
            "nothing was dropped as unattributable: {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(1),
            "F-2 → F-1 is one entry edge: {out:#?}"
        );
    }

    #[tokio::test]
    async fn an_entry_naming_itself_records_no_edge() {
        // The one same-file case that must STILL be refused. `attribute_entry_edge` keeps
        // the intra-ledger edge only when the citing entry differs from the defining one;
        // an entry naming itself is a self-loop at entry grain too, and counting it would
        // let an entry inflate its own indegree — the exact failure `entry_indegree`'s
        // same-file exclusion exists to prevent, reintroduced one level down.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — an entry that names itself\n\
             \n\
             this entry, F-1, is about itself\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "led", &p, "ledger");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(
            out["counts"]["citations"],
            json!(1),
            "the body mention is a citation; the heading defines and does not self-cite: \
             {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(0),
            "F-1 → F-1 is not an edge between two nodes: {out:#?}"
        );
        assert_eq!(
            e["attributed"],
            json!(0),
            "and it is not counted as attributed either: {out:#?}"
        );
        assert_eq!(
            e["outside_any_entry"],
            json!(1),
            "refused citations stay in the partition, so attributed + outside_any_entry \
             still totals the resolved citations: {out:#?}"
        );
    }

    /// One shape for every finding array. `ambiguous` used to call the cited text
    /// `token` while `dangling` and `cross_repo` called the identical value `raw` —
    /// three adjacent `json!` literals, no shared owner, so they drifted.
    ///
    /// The cost is a query that succeeds while answering half: a grep for
    /// `"token":"HY-…"` over a whole report returns nothing from `dangling` and reads
    /// as "no HY token is broken". That mistake was made on the way to filing this,
    /// which is why the fix is a single constructor rather than a rename — a rename
    /// leaves three literals free to diverge again.
    ///
    /// docs/issues/archive/2026-08-17-link-scan-names-the-same-field-raw-in-dangling-and-token-in-ambiguous.md
    #[test]
    fn every_finding_array_names_the_cited_text_the_same_way() {
        let c = cite("F-3");
        let shapes = [
            (
                "ambiguous",
                finding(
                    "src-1",
                    &c,
                    json!({"candidates": [], "candidates_total": 2}),
                ),
            ),
            ("dangling", finding("src-1", &c, json!({}))),
            ("cross_repo", finding("src-1", &c, json!({}))),
        ];

        for (array, f) in &shapes {
            assert_eq!(
                f["raw"], "F-3",
                "{array} must carry the cited text as `raw`"
            );
            assert_eq!(
                f["kind"], "EntryToken",
                "{array} must carry the citation kind — it was present in one arm of three"
            );
            assert_eq!(f["src_id"], "src-1", "{array}");
            assert_eq!(f["line"], 7, "{array}");
            assert!(
                f.get("token").is_none(),
                "`token` is the name that split the vocabulary; {array} must not reintroduce it"
            );
        }
    }

    /// Arm-specific fields still ride along, so unifying the shared shape does not
    /// flatten what makes `ambiguous` useful.
    #[test]
    fn finding_carries_arm_specific_fields_alongside_the_shared_shape() {
        let f = finding(
            "src-1",
            &cite("B-1"),
            json!({"candidates": ["a", "b"], "candidates_total": 2}),
        );
        assert_eq!(f["candidates_total"], 2);
        assert_eq!(f["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(f["raw"], "B-1", "the shared shape survives the merge");
    }

    #[tokio::test]
    async fn counts_flags_truncation_per_finding_array_when_the_cap_is_exceeded() {
        // `FINDINGS_CAP` (50) caps every finding array; without a `truncated` flag,
        // `len(dangling) == 50` reads identically whether the true count is 50 or 5000 —
        // docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        // One entry defines the `F` prefix, so `F-2..F-52` are DANGLING (prefix known,
        // ids undefined) rather than inert (prefix unknown, reported nowhere at all).
        let dst = tmp.path().join("target.md");
        std::fs::write(&dst, "## F-1 — anchor entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let mut body = String::new();
        for n in 2..=52 {
            body.push_str(&format!("See F-{n}.\n"));
        }
        let src = tmp.path().join("source.md");
        std::fs::write(&src, &body).unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": false })).await.unwrap();

        assert_eq!(out["counts"]["dangling"], json!(51), "{out:#?}");
        assert_eq!(
            out["dangling"].as_array().unwrap().len(),
            50,
            "the array itself stays capped: {out:#?}"
        );
        assert_eq!(
            out["counts"]["truncated"]["dangling"],
            json!(true),
            "51 > the 50-entry cap must be visible without comparing two fields: {out:#?}"
        );
        assert_eq!(
            out["counts"]["truncated"]["ambiguous"],
            json!(false),
            "an array at or under its cap (here: empty) must not read as truncated: {out:#?}"
        );
    }

    /// End-to-end: a double-qualified citation must be flagged, not silently resolved
    /// as an edge -- even when the inner `<file-stem>:<ID>` form WOULD legitimately
    /// resolve on its own. This is the sharpest version of the bug: the qualifier
    /// segment being dropped is not merely lost, it lets a malformed citation succeed
    /// where a correctly-shaped one would be expected, with zero signal to the author.
    /// docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md
    #[tokio::test]
    async fn a_double_qualified_citation_is_reported_not_resolved_even_when_the_inner_form_would_resolve(
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.path().join("target.md");
        std::fs::write(&dst, "## F-2 — anchor entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let src = tmp.path().join("source.md");
        std::fs::write(&src, "See `codescout:target:F-2` for the rule.\n").unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
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
            .build();
        let out = call(&ctx, json!({ "write": true })).await.unwrap();

        assert_eq!(out["counts"]["malformed_qualifier"], json!(1), "{out:#?}");
        assert_eq!(
            out["malformed_qualifier"].as_array().unwrap().len(),
            1,
            "{out:#?}"
        );
        assert_eq!(
            out["malformed_qualifier"][0]["raw"], "codescout:target:F-2",
            "the whole three-part citation, not the collapsed inner form: {out:#?}"
        );
        assert_eq!(
            out["counts"]["cross_repo"],
            json!(0),
            "must not ALSO land in cross_repo: {out:#?}"
        );
        assert_eq!(
            out["counts"]["dangling"],
            json!(0),
            "must not land in dangling either -- it is malformed, not merely unresolved: {out:#?}"
        );
        assert_eq!(
            out["counts"]["edges_missing"],
            json!(0),
            "must never silently produce an edge, even though the inner `target:F-2` \
             form would legitimately resolve on its own: {out:#?}"
        );
    }
}
