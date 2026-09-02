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
    /// Skip this many findings per array before filling it. Default 0.
    ///
    /// Named `findings_*` rather than the bare `offset`/`limit` the bug file proposed,
    /// because `limit` on this action is already taken and means something else entirely
    /// — the cap on *artifacts scanned*. A bare `limit` meaning "artifacts" for one
    /// caller and "findings" for the next is a silent misread, not an error: both are
    /// integers, both plausible, and the response shape is identical either way.
    #[serde(default)]
    findings_offset: Option<usize>,
    /// Cap on findings carried per array. Defaults to [`FINDINGS_CAP`].
    #[serde(default)]
    findings_limit: Option<usize>,
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

/// A window into a findings array: skip `offset` findings, then keep up to `limit`.
///
/// The arrays are capped for budget reasons and the population was not reachable any
/// other way. Measured on this repo 2026-08-30: `dangling` produced 637 findings into a
/// 50-entry array — **7.8%** — so a reader who searched a bucket and found nothing had
/// searched almost none of it. `4c063b4e` made that truncation *visible*; the window
/// makes the remainder *reachable*, which is the step (3) that bug file left open
/// (`docs/issues/archive/2026-08-30-link-scan-truncation-is-accurate-and-unreachable.md`).
///
/// `_by_source` is deliberately NOT windowed: it is the only complete view of the
/// distribution, and it is what lets a *zero* be answered without paging at all.
#[derive(Clone, Copy)]
struct FindingWindow {
    offset: usize,
    limit: usize,
}

/// Record one finding: advance that array's own total, then keep the value if it falls
/// inside `w`.
///
/// **`seen` is the total, and this function owns it.** Each of the five call sites used
/// to increment its `*_total` by hand on the line above its push — and the total doubles
/// as the finding's zero-based index, which is the only thing that makes an offset
/// expressible. Two hand-maintained values that must agree, at five sites, is the shape a
/// sixth arm gets wrong silently: the array would still fill, and only the count beside
/// it would be off.
///
/// That is also why this takes `&mut usize` rather than an index: an index parameter
/// would put the same ordering hazard back at the call site, one argument to the left,
/// and `total - 1` is exactly the off-by-one nobody would see — a stale total is not a
/// crash, it is a plausible number.
///
/// Totals stay counted separately from the array, so a windowed view never hides the true
/// distribution (the report-the-verdict-not-the-distribution trap).
fn push_windowed(v: &mut Vec<Value>, seen: &mut usize, w: FindingWindow, val: Value) {
    let index = *seen;
    *seen += 1;
    if index >= w.offset && v.len() < w.limit {
        v.push(val);
    }
}

/// Are there findings past the end of the window the caller received?
///
/// This is the `truncated` flag's meaning, and it is deliberately NOT `total > shown`.
/// The two agree only while `offset == 0`, which is why the older form survived: every
/// call was the first page. With paging they diverge exactly where it matters — on the
/// last page of 637 at `offset = 600`, `total > shown` is true while nothing remains, so
/// a caller who paged all the way to the end would be told the list was still cut. The
/// flag would then be wrong only for the reader who did the most work to trust it.
///
/// Saturating because `offset` is caller-supplied and may exceed `total`; the sum must
/// not wrap into a "more beyond" answer on a window that is entirely past the end.
fn more_beyond(w: FindingWindow, total: usize, shown: usize) -> bool {
    w.offset.saturating_add(shown) < total
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

/// The `rests-on` edge a Statement's `**Rests on:**` declaration names, if it names
/// one the resolver can reach.
///
/// Deliberately **not** line-attributed, which is the whole difference from
/// [`attribute_entry_edge`]. That function has to infer which entry a prose citation
/// belongs to by position, and pays 12.1% attribution error for it. A `**Rests on:**`
/// declaration is parsed *out of one section's own text*, so the owning entry is
/// given rather than inferred, and no heuristic is involved.
///
/// The self-reference rule is the same one `attribute_entry_edge` applies: an entry
/// resting on a sibling in its own ledger is a real edge; an entry resting on itself,
/// or on a `dst_ref` naming no entry at all inside its own artifact, is not.
fn rests_on_edge(
    section: &extract::EntrySection,
    c: &extract::Citation,
    src_id: &str,
    dst_id: &str,
    id_to_slug: &BTreeMap<String, String>,
) -> Option<(String, String, String)> {
    let src_slug = id_to_slug.get(src_id)?;
    let dst_ref = entry_dst_ref(c, dst_id, id_to_slug)?;
    if src_id == dst_id {
        match dst_ref.rsplit_once(':').map(|(_, local)| local) {
            // Intra-ledger, a different entry: a real edge.
            Some(local) if local != section.id.as_str() => {}
            _ => return None,
        }
    }
    Some((src_slug.clone(), section.id.clone(), dst_ref))
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
    let window = FindingWindow {
        offset: args.findings_offset.unwrap_or(0),
        limit: args.findings_limit.unwrap_or(FINDINGS_CAP),
    };
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
    // Computed AFTER the resolution loop rather than here, since 2026-09-02: the conflict
    // itself is still a fact about the INDEX and is reported even when every citation
    // resolves — exactly the `T` case as filed in 2026-08-18 — but `colliding_tokens` now
    // annotates each one with what it costs today, and those tallies exist only once the
    // citations have been walked. Membership is unchanged: a conflict with zero citations is
    // still emitted. See `resolve::DefinitionIndex::prefix_conflicts`.
    // Artifact ids throughout, consistent with `src_id` and `dst_ref` in the arms below;
    // resolve one with `artifact(action="get", id=…)`.
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
    // Layer 3c. A `**Rests on:**` value whose token the resolver refused: the author
    // named a proof the graph cannot reach. Reported rather than guessed, same rule as
    // every other refusal here — but reported SEPARATELY from `ambiguous`, because the
    // remedy differs. An ambiguous prose citation is usually incidental; an unresolvable
    // `**Rests on:**` is a Statement whose declared basis does not resolve, which is the
    // one thing the field exists to prevent.
    let mut rests_on_unresolvable: Vec<Value> = Vec::new();
    let mut rests_on_unresolvable_total = 0usize;
    let mut dangling: Vec<Value> = Vec::new();
    let mut cross_repo: Vec<Value> = Vec::new();
    let mut malformed_qualifier: Vec<Value> = Vec::new();
    let mut cross_repo_file_qualified: Vec<Value> = Vec::new();
    let (mut ambiguous_total, mut dangling_total, mut cross_repo_total) = (0usize, 0usize, 0usize);
    let mut malformed_qualifier_total = 0usize;
    let mut cross_repo_file_qualified_total = 0usize;
    // Per-source counts for the same three arms, uncapped (unlike the `ambiguous` /
    // `dangling` / `cross_repo` finding arrays above, which cap at FINDINGS_CAP) —
    // the whole point is to make the TOTAL interpretable, so a source contributing
    // past the cap must still show its true count.
    // docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md
    let mut ambiguous_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut dangling_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut cross_repo_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut malformed_qualifier_by_source: BTreeMap<String, usize> = BTreeMap::new();
    let mut cross_repo_file_qualified_by_source: BTreeMap<String, usize> = BTreeMap::new();
    // token -> (citing_sources, citing_mentions), for `prefix_conflicts`' intersection.
    // TWO units on purpose, and conflating them is a measured failure rather than a
    // hypothetical: one `Citation` is emitted per `(kind, raw)` per document, so the first
    // counts artifacts and the second counts mentions. `tracker-hygiene-log:HY-21` predicted
    // -3 from three mentions of one token and measured -2, because a single citer's two
    // mentions had always been one finding.
    //
    // Ambiguous citations only. A bare token with two active definers is ALWAYS ambiguous,
    // and a `<stem>:TOKEN` citation still resolves — so this counts exactly the citations the
    // collision breaks, never the ones a reader already qualified around.
    let mut colliding_token_citations: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut citations_total = 0usize;

    // Which registered workspace root, if any, contains a given row — used below to
    // split `MalformedQualifier` into "redundant self-repo prefix" (outer segment ==
    // the citing artifact's own root name) vs "names a different repo" (everything
    // else). `ctx.workspace.roots` is THIS repo's own multi-project roots (e.g.
    // "codescout" + "codescout-embed"), so a match here is a genuine self-reference,
    // not a claim that the outer segment names a real, known SIBLING repo — Option 2
    // of docs/issues/archive/2026-08-27-cross-repo-file-qualified-citation-unsupported.md
    // deliberately does not attempt that stronger, unbuilt form of resolution.
    let root_paths: Vec<std::path::PathBuf> =
        ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();

    // Entry-grain edges: (src_slug, src_local, dst_ref). A set because one entry citing
    // one target twice is ONE edge, and because a stable order makes the emitted sample
    // diffable across runs.
    let mut desired_entry: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    // Layer 3c: counted separately from `edges_attributed`, which is a CITATION count
    // over prose. This is an EDGE count over declarations — different unit, different
    // denominator, so summing them would be meaningless.
    let mut rests_on_derived = 0usize;
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
        // Names under which this row's own repo may legitimately be cited. Two
        // independent sources, UNIONED rather than ranked: either match is positive
        // evidence of a self-reference, and the policy at the use site only claims
        // "genuinely cross-repo" on positive evidence.
        //
        // The registry name alone was the original source, and it is `None` for most
        // repos. `ctx.workspace.roots` is the user's optional `[[roots]]` registry in
        // ~/.config/librarian/workspace.toml, which a repo has to be hand-added to;
        // codescout is not in its own, appearing there only as an umbrella MEMBER. So
        // `containing_root` returned None for all 1183 rows, `is_some_and` folded that
        // into the self-reference branch, and the cross-repo bucket was unreachable in
        // every real repo while its unit test passed on a hand-built absolute root.
        // docs/issues/archive/2026-08-27-cross-repo-file-qualified-bucket-never-fires.md
        let mut self_names: Vec<&str> = Vec::new();
        if let Some(name) = crate::librarian::tools::containing_root(&root_paths, &row.abs_path)
            .and_then(|matched| {
                ctx.workspace
                    .roots
                    .iter()
                    .find(|r| &r.path == matched)
                    .map(|r| r.name.as_str())
            })
        {
            self_names.push(name);
        }
        // Git-root basename — the same convention `artifact(action="create")`'s `repo`
        // field documents ("workspace root name (git repo basename)"), and the one a
        // citation author is actually spelling. Gated on the row living under that git
        // root so a scope="umbrella"/"all" scan cannot stamp this repo's name onto
        // another repo's rows; `containing_root` is reused for the comparison rather
        // than a bare `starts_with` so the component-boundary and Windows-verbatim
        // handling stay in one place.
        if let Some(name) = git_root
            .as_ref()
            .filter(|root| {
                crate::librarian::tools::containing_root(std::slice::from_ref(root), &row.abs_path)
                    .is_some()
            })
            .and_then(|root| root.file_name())
            .and_then(|n| n.to_str())
        {
            self_names.push(name);
        }
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
                        desired_entry.extend(
                            triples
                                .into_iter()
                                .map(|(s, l, d)| (s, l, d, diff::CITES_REL.to_string())),
                        );
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
                        desired_entry.extend(
                            triples
                                .into_iter()
                                .map(|(s, l, d)| (s, l, d, diff::CITES_REL.to_string())),
                        );
                    }
                }
                Some(resolve::Outcome::Ambiguous { candidates, total }) => {
                    *ambiguous_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    // Keyed by the RAW citation text, so a qualified form never lands here:
                    // `T-14` matches a token, `tool-usage-patterns:T-14` matches nothing and
                    // is skipped by the lookup in `prefix_conflicts`.
                    let tally = colliding_token_citations
                        .entry(c.raw.clone())
                        .or_insert((0, 0));
                    tally.0 += 1;
                    tally.1 += c.occurrences().count();
                    push_windowed(
                        &mut ambiguous,
                        &mut ambiguous_total,
                        window,
                        finding(
                            &row.id,
                            c,
                            json!({"candidates": candidates, "candidates_total": total}),
                        ),
                    );
                }
                Some(resolve::Outcome::Dangling) => {
                    *dangling_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    push_windowed(
                        &mut dangling,
                        &mut dangling_total,
                        window,
                        finding(&row.id, c, json!({})),
                    );
                }
                Some(resolve::Outcome::CrossRepo) => {
                    *cross_repo_by_source.entry(src_rel.clone()).or_insert(0) += 1;
                    push_windowed(
                        &mut cross_repo,
                        &mut cross_repo_total,
                        window,
                        finding(&row.id, c, json!({})),
                    );
                }
                Some(resolve::Outcome::MalformedQualifier) => {
                    let outer = c.raw.split(':').next().unwrap_or("");
                    // Positive evidence required to call it cross-repo: at least one
                    // name for the citing repo must be known AND the outer segment
                    // must match none of them. Either "no name known" or "outer is one
                    // of our own names" falls back to the generic bucket — the safe
                    // default when we cannot confirm a self-reference, since claiming
                    // "genuinely cross-repo" is the stronger assertion.
                    if !self_names.is_empty() && !self_names.contains(&outer) {
                        *cross_repo_file_qualified_by_source
                            .entry(src_rel.clone())
                            .or_insert(0) += 1;
                        push_windowed(
                            &mut cross_repo_file_qualified,
                            &mut cross_repo_file_qualified_total,
                            window,
                            finding(&row.id, c, json!({})),
                        );
                    } else {
                        *malformed_qualifier_by_source
                            .entry(src_rel.clone())
                            .or_insert(0) += 1;
                        push_windowed(
                            &mut malformed_qualifier,
                            &mut malformed_qualifier_total,
                            window,
                            finding(&row.id, c, json!({})),
                        );
                    }
                }
                None => {} // suppressed noise / foreign-jurisdiction links
            }
        }

        // ---- Layer 3c: `**Rests on:**` -> rel="rests-on" -------------------
        //
        // A second derivation over the SAME sections, resolved through the SAME
        // `resolve::resolve`. Only `Outcome::Edge` becomes a row; Ambiguous,
        // Dangling, CrossRepo and MalformedQualifier stay reported by the loop
        // above and are never guessed here either.
        //
        // Tokens come from `extract::extract` run over the declaration's own text
        // rather than a local regex, so there is exactly ONE tokenizer over the
        // `PREFIX-N` namespace. A second one would drift from this one on every
        // qualifier or escaping rule added later — the defect class this repo
        // tracks as IC-6 (`addressing-without-an-escape-hatch`), 27 instances.
        // The line numbers `extract` reports are relative to the declaration and
        // are unused: the owning entry is given, not inferred.
        for section in sections {
            let declared = crate::librarian::statements::declared_section_text(section, sections);
            let Some(value) = crate::librarian::statements::parse_rests_on(&declared) else {
                continue;
            };
            for rc in &extract::extract(&value).citations {
                // BOTH `Edge` and `SelfCite`, mirroring the prose path above.
                //
                // The spec's § Resolution and materialization says "only `Edge`
                // becomes a row … `SelfCite` stays reported". That sentence predates
                // `2026-08-21-selfcite-is-file-grain-so-intra-ledger-entry-edges-never-materialize`:
                // `SelfCite` is a FILE-grain verdict, correct there and wrong at entry
                // grain, where F-1 and F-2 are two nodes. Following the spec literally
                // here dropped every intra-ledger declaration — and intra-ledger is the
                // common shape for `**Rests on:**`, since an entry most often rests on
                // a sibling in its own ledger. Caught by
                // `a_rests_on_declaration_naming_a_sibling_records_a_rests_on_edge`.
                //
                // The true self-reference is refused inside `rests_on_edge`, not here.
                let dst = match resolve::resolve(rc, &row.id, &rel_dir, &index, &corpus) {
                    Some(resolve::Outcome::Edge { dst_id })
                    | Some(resolve::Outcome::SelfCite { dst_id }) => dst_id,
                    other => {
                        // `None` is suppressed noise — a prose acronym, or a rel-path
                        // link that is `audit_doc_refs`'s jurisdiction. Reporting it
                        // would make every ordinary capitalised word in a declaration a
                        // finding, which is how a worklist becomes unreadable.
                        let (reason, extra) = match &other {
                            Some(resolve::Outcome::Ambiguous { candidates, total }) => (
                                "ambiguous",
                                json!({"candidates": candidates, "candidates_total": total}),
                            ),
                            Some(resolve::Outcome::Dangling) => ("dangling", json!({})),
                            Some(resolve::Outcome::CrossRepo) => ("cross_repo", json!({})),
                            Some(resolve::Outcome::MalformedQualifier) => {
                                ("malformed_qualifier", json!({}))
                            }
                            _ => continue,
                        };
                        let mut f = finding(&row.id, rc, extra);
                        if let Some(o) = f.as_object_mut() {
                            o.insert("entry".into(), json!(section.id));
                            o.insert("reason".into(), json!(reason));
                        }
                        push_windowed(
                            &mut rests_on_unresolvable,
                            &mut rests_on_unresolvable_total,
                            window,
                            f,
                        );
                        continue;
                    }
                };
                if let Some((s, l, d)) = rests_on_edge(section, rc, &row.id, &dst, &id_to_slug) {
                    rests_on_derived += 1;
                    desired_entry.insert((s, l, d, diff::RESTS_ON_REL.to_string()));
                }
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
        // Cites-grain ONLY. `desired_entry` now carries two rels, and `derived` is
        // documented in the response as `attributed` deduplicated — a claim about
        // prose citations. Counting `desired_entry.len()` here would fold rests-on
        // edges into a number whose stated denominator is citations, which is the
        // "a count arrives with its unit or not at all" rule.
        derived: desired_entry
            .iter()
            .filter(|(_, _, _, rel)| rel == diff::CITES_REL)
            .count(),
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
        for (src_slug, src_local, dst_ref, rel) in &desired_entry {
            let wrote = entry_cite::insert_with(
                &tx,
                &entry_cite::EntryCiteRow {
                    src_slug: src_slug.clone(),
                    src_local: src_local.clone(),
                    dst_ref: dst_ref.clone(),
                    rel: rel.clone(),
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
        // Over BOTH rels, matching the insert loop above. Deliberately not
        // `entry_report.derived - written`: `derived` is cites-only by design (see
        // its construction), so that subtraction underflows the moment a single
        // rests-on edge is written. usize subtraction panics rather than wrapping,
        // which is the safe failure, but it is still a panic on real data.
        entry_report.skipped_existing = desired_entry.len() - entry_report.written;
        tx.commit()?;
    }

    let prefix_conflicts = index.prefix_conflicts(&colliding_token_citations);
    // The count a reader needs but would otherwise have to derive: how many of these are
    // broken NOW. A bare total stops working as a tripwire once any structural member is
    // permanent — a declared ledger whose entries live in companion files is one by
    // construction — and nobody reads 3 -> 4 the way they read 0 -> 1.
    let prefix_conflicts_live = prefix_conflicts
        .iter()
        .filter(|c| !c.colliding_tokens.is_empty())
        .count();

    // Human-reviewable edge lists (windowed), with rel_paths for readability.
    // Paged by the same window as the finding arrays: these were capped too, and a
    // caller who can reach every dangling citation but only the first 50 stale edges has
    // been handed the same defect in a quieter place.
    let edge_view = |pairs: &[(String, String)]| -> Vec<Value> {
        pairs
            .iter()
            .skip(window.offset)
            .take(window.limit)
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
                // Layer 3c. A separate unit: edges derived from `**Rests on:**`
                // DECLARATIONS, not from prose citations, so it shares no
                // denominator with the four above and must not be summed with them.
                // `written`/`skipped_existing` cover both rels together, because the
                // insert loop does.
                "rests_on_derived": rests_on_derived,
                // Statements whose declared basis the resolver refused. NOT a subset of
                // `derived` — these produced no edge at all — and not comparable to the
                // top-level `ambiguous` count, which is over prose citations.
                "rests_on_unresolvable": rests_on_unresolvable_total,
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
            // Same underlying `MalformedQualifier` outcome, split out from the arm
            // above: the outer segment names something other than the citing
            // artifact's own workspace root, so it presumably names a different
            // repo rather than redundantly repeating this one's. Still never an
            // edge — reported only, same as `cross_repo` above and for the same
            // reason (edges cannot span workspaces) — this bucket exists purely so
            // a reader does not have to check each `malformed_qualifier` entry's
            // outer segment by hand to tell "strip this" from "leave it, it's
            // prose pointing at a sibling repo."
            // docs/issues/archive/2026-08-27-cross-repo-file-qualified-citation-unsupported.md
            "cross_repo_file_qualified": cross_repo_file_qualified_total,
            "prefix_conflicts": prefix_conflicts.len(),
            // Conflicts with at least one token defined twice — the subset breaking citations
            // today, as opposed to two allocators that have not collided yet.
            "prefix_conflicts_live": prefix_conflicts_live,
            // The window a caller actually received, so `truncated` and the array
            // lengths are interpretable without the caller re-deriving them from the
            // arguments they happened to send. Absent from the request, present in the
            // response: a default is a value, and a reader who assumes 0/50 is guessing.
            "findings_window": { "offset": window.offset, "limit": window.limit },
            // `len(dangling) == FINDINGS_CAP` reads identically whether the true count is
            // exactly the cap or 100x it — this states which arrays actually got cut, so a
            // reader never has to compare a count against an array length to find out.
            //
            // It means MORE BEYOND THIS WINDOW, not `total > shown` — those differ exactly
            // where paging is used. On the last page of 637 with offset 600, `total > len`
            // is true while nothing remains, which would report every final page as cut and
            // make the flag useless precisely for the caller who paged to reach the end.
            "truncated": {
                "rests_on_unresolvable": more_beyond(
                    window,
                    rests_on_unresolvable_total,
                    rests_on_unresolvable.len(),
                ),
                "ambiguous": more_beyond(window, ambiguous_total, ambiguous.len()),
                "dangling": more_beyond(window, dangling_total, dangling.len()),
                "cross_repo": more_beyond(window, cross_repo_total, cross_repo.len()),
                "malformed_qualifier": more_beyond(
                    window,
                    malformed_qualifier_total,
                    malformed_qualifier.len(),
                ),
                "cross_repo_file_qualified": more_beyond(
                    window,
                    cross_repo_file_qualified_total,
                    cross_repo_file_qualified.len(),
                ),
            },
        },
        "edges_missing": edge_view(&d.to_add),
        "edges_stale": edge_view(&d.stale),
        "rests_on_unresolvable": rests_on_unresolvable,
        "ambiguous": ambiguous,
        "dangling": dangling,
        "cross_repo": cross_repo,
        "malformed_qualifier": malformed_qualifier,
        "cross_repo_file_qualified": cross_repo_file_qualified,
        // Per-source breakdown of the three arms above, uncapped — the "attribute and
        // subtract" reading: a triager checks which keys are guides/conventions docs
        // explaining citation syntax (rather than genuinely broken references) and
        // discounts those before trusting the raw total in `counts`.
        "ambiguous_by_source": ambiguous_by_source,
        "dangling_by_source": dangling_by_source,
        "cross_repo_by_source": cross_repo_by_source,
        "malformed_qualifier_by_source": malformed_qualifier_by_source,
        "cross_repo_file_qualified_by_source": cross_repo_file_qualified_by_source,
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

    /// Layer 3c. A `**Rests on:**` declaration naming a resolvable target becomes an
    /// `entry_cite` row with `rel="rests-on"`.
    #[tokio::test]
    async fn a_rests_on_declaration_naming_a_sibling_records_a_rests_on_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — the decision that is rested on\n\
             \n\
             body\n\
             \n\
             ## F-2 — the entry that rests on it\n\
             \n\
             **Rests on:** F-1\n",
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
            e["rests_on_derived"],
            json!(1),
            "F-2 rests on F-1: {out:#?}"
        );
        assert_eq!(
            e["derived"],
            json!(1),
            "and the SAME line also yields a cites edge, because a `**Rests on:** F-1` \
             line is prose that contains `F-1`. That is intended, not double-counting: \
             the two rels are two rows between one pair (see \
             `a_rests_on_edge_coexists_with_a_cites_edge_between_the_same_pair`), and \
             the exposure tap the spec designs fires on `max(reads, rests-on \
             in-degree)` rather than a sum. Asserted here so that if the derivation is \
             ever narrowed to suppress the cites half, it is a deliberate change: \
             {out:#?}"
        );
    }

    /// **The property the whole Layer 3c design rests on.** `entry_cite`'s primary key
    /// is `(src_slug, src_local, dst_ref, rel)`, so a `rests-on` edge and a `cites`
    /// edge between the *same pair* are two rows, not one.
    ///
    /// Asserted through `written` in write mode rather than by reading the table,
    /// because `written` counts rows the DB actually accepted: if `rel` were absent
    /// from the PK the second insert would be a no-op and this reads 1.
    ///
    /// **Load-bearing fixture detail:** F-2 both declares `**Rests on:** F-1` *and*
    /// mentions `F-1` in prose. Drop either line and the test still passes while no
    /// longer testing coexistence at all.
    #[tokio::test]
    async fn a_rests_on_edge_coexists_with_a_cites_edge_between_the_same_pair() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — the decision that is rested on\n\
             \n\
             body\n\
             \n\
             ## F-2 — the entry that rests on it\n\
             \n\
             **Rests on:** F-1\n\
             \n\
             and the prose also cites F-1 directly\n",
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
        let out = call(&ctx, json!({ "write": true })).await.unwrap();
        let e = &out["counts"]["entry_edges"];

        assert_eq!(e["derived"], json!(1), "one cites edge: {out:#?}");
        assert_eq!(
            e["rests_on_derived"],
            json!(1),
            "one rests-on edge: {out:#?}"
        );
        assert_eq!(
            e["written"],
            json!(2),
            "BOTH rows must land. A 1 here means `rel` stopped discriminating in the \
             primary key and one edge silently replaced the other: {out:#?}"
        );
        assert_eq!(
            e["skipped_existing"],
            json!(0),
            "nothing was skipped, so `written` is the whole population: {out:#?}"
        );
    }

    /// The self-reference rule, same as the prose path: an entry resting on *itself*
    /// is not an edge. Distinct from resting on a sibling, which
    /// `a_rests_on_declaration_naming_a_sibling_records_a_rests_on_edge` covers.
    #[tokio::test]
    async fn a_rests_on_declaration_naming_its_own_entry_records_no_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — the entry that names itself\n\
             \n\
             **Rests on:** F-1\n",
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
        assert_eq!(
            out["counts"]["entry_edges"]["rests_on_derived"],
            json!(0),
            "a self-reference is not an edge: {out:#?}"
        );
    }

    /// Prose that merely *mentions* the field is not a declaration, and a declaration
    /// naming nothing resolvable stays prose — `Outcome::Edge` is the only outcome that
    /// becomes a row.
    ///
    /// Both halves matter and fail in opposite directions: the first would over-report
    /// (any sentence about `Rests on:` becomes an edge), the second would guess at a
    /// target the resolver refused.
    #[tokio::test]
    async fn a_mentioned_or_unresolvable_rests_on_records_no_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let p = tmp.path().join("ledger.md");
        std::fs::write(
            &p,
            "## F-1 — the entry\n\
             \n\
             see the **Rests on:** field for how this works\n\
             \n\
             ## F-2 — another entry\n\
             \n\
             **Rests on:** ZZZ-999 which nothing defines\n",
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
        assert_eq!(
            out["counts"]["entry_edges"]["rests_on_derived"],
            json!(0),
            "neither a mention nor an unresolvable target is an edge: {out:#?}"
        );
        assert_eq!(
            out["counts"]["entry_edges"]["rests_on_unresolvable"],
            json!(0),
            "and NEITHER is a worklist item. `ZZZ-999` is a prefix nothing defines \
             anywhere, which resolves as prose noise (`None`) rather than `Dangling` — \
             the same gate that keeps `UTF-8` and `SHA-256` silent. Reporting suppressed \
             noise would make every capitalised word in a declaration a finding, which \
             is how a worklist stops being read: {out:#?}"
        );
    }

    /// Layer 3c's worklist. A `**Rests on:**` naming a token with two active definers
    /// resolves to `Ambiguous` — no edge, but the author declared a basis the graph
    /// cannot reach, which is the one failure this field exists to prevent. Reported,
    /// never guessed.
    ///
    /// **Fixture needs three files, and that is load-bearing.** Two ledgers must define
    /// `F-1` for it to be ambiguous at all, and the *citing* file must be a third: put
    /// the declaration in either definer and `resolve` returns `SelfCite` instead (the
    /// local definition wins), which is a different arm and would silently pass.
    #[tokio::test]
    async fn an_ambiguous_rests_on_declaration_is_reported_with_its_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let a = tmp.path().join("a.md");
        std::fs::write(&a, "## F-1 — one definer\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "aa", &a, "alpha");

        let b = tmp.path().join("b.md");
        std::fs::write(&b, "## F-1 — the other definer\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "bb", &b, "beta");

        let c = tmp.path().join("c.md");
        std::fs::write(
            &c,
            "## W-1 — the entry that declares an unreachable basis\n\
             \n\
             **Rests on:** F-1\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "cc", &c, "gamma");

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

        assert_eq!(
            out["counts"]["entry_edges"]["rests_on_derived"],
            json!(0),
            "ambiguity is never guessed into an edge: {out:#?}"
        );
        assert_eq!(
            out["counts"]["entry_edges"]["rests_on_unresolvable"],
            json!(1),
            "but it MUST be reported — a silent drop here is a Statement whose declared \
             basis nothing can reach, with no signal to its author: {out:#?}"
        );
        let f = &out["rests_on_unresolvable"][0];
        assert_eq!(
            f["entry"],
            json!("W-1"),
            "names the declaring entry: {out:#?}"
        );
        assert_eq!(f["reason"], json!("ambiguous"), "{out:#?}");
        assert_eq!(
            f["candidates_total"],
            json!(2),
            "and carries the candidates, which is the actionable part — they tell the \
             author which qualified form to write: {out:#?}"
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

    /// Fixture for the window tests: `F-2..F-(count+1)` cited from one source, with
    /// `F-1` DEFINED in a second file.
    ///
    /// That definition is the load-bearing detail. It is what makes the `F` prefix
    /// *known*, which is what makes `F-2…` resolve as **dangling** rather than as
    /// unrecognised prose — and unrecognised prose is reported nowhere at all, so
    /// without it every assertion below would read zero findings and pass vacuously
    /// under any mutation to the window.
    fn dangling_corpus(tmp: &std::path::Path, count: usize) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.join("target.md");
        std::fs::write(&dst, "## F-1 — anchor entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let mut body = String::new();
        for n in 2..=(count + 1) {
            body.push_str(&format!("See F-{n}.\n"));
        }
        let src = tmp.join("source.md");
        std::fs::write(&src, &body).unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.to_path_buf();
        TestToolContextBuilder::new(cat)
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

    /// Paging reaches the WHOLE population, and each page is disjoint from the last.
    ///
    /// This is the defect the truncation warning could only describe: `4c063b4e` made
    /// `dangling[50 of 637]` visible, and the other 587 stayed unreachable by any
    /// argument. Asserting that a single offset returns *something* would not test that
    /// — it is the union over pages equalling the population that says the findings are
    /// reachable, and the absence of duplicates that says the pages are a partition
    /// rather than overlapping prefixes.
    ///
    /// Mutations that must kill this: drop `.skip`/the `index >= w.offset` guard (every
    /// page returns the same prefix — duplicates, and a short union); ignore
    /// `findings_limit` (page one swallows everything, later pages come back empty).
    #[tokio::test]
    async fn findings_offset_pages_the_whole_population_without_repeats() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = dangling_corpus(tmp.path(), 51);

        let mut seen: Vec<String> = Vec::new();
        let mut last_truncated = true;
        for page in 0..6 {
            let out = call(
                &ctx,
                json!({ "write": false, "findings_offset": page * 10, "findings_limit": 10 }),
            )
            .await
            .unwrap();

            assert_eq!(
                out["counts"]["findings_window"],
                json!({ "offset": page * 10, "limit": 10 }),
                "the response must echo the window it applied, so a reader never has to \
                 re-derive it from the arguments they happened to send: {out:#?}"
            );

            for f in out["dangling"].as_array().unwrap() {
                seen.push(f["raw"].as_str().unwrap().to_string());
            }
            last_truncated = out["counts"]["truncated"]["dangling"].as_bool().unwrap();
        }

        let unique: std::collections::BTreeSet<&String> = seen.iter().collect();
        assert_eq!(
            unique.len(),
            51,
            "every dangling finding must be reachable by paging — got {} distinct of 51: {seen:?}",
            unique.len()
        );
        assert_eq!(
            seen.len(),
            51,
            "pages must PARTITION the population, not overlap: {} rows for {} distinct",
            seen.len(),
            unique.len()
        );
        assert!(
            !last_truncated,
            "the page that reaches the end must report truncated=false, or a caller who \
             paged all the way is told the list is still cut and pages forever"
        );
    }

    /// The window changes what is SHOWN and never what is COUNTED.
    ///
    /// `push_windowed` owns the total that used to be incremented by hand beside each
    /// push, so a window bug and a counting bug are now the same edit — which is the
    /// point, but it also means the count needs its own assertion. A total that tracked
    /// the array would make `truncated` and every `_by_source` figure agree with each
    /// other and with nothing real.
    ///
    /// Mutation that must kill this: move `*seen += 1` inside the `if`, so only kept
    /// findings count. `counts.dangling` drops to the page size and every page reports
    /// a complete-looking scan.
    #[tokio::test]
    async fn the_window_never_changes_the_totals_or_the_per_source_distribution() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = dangling_corpus(tmp.path(), 51);

        let full = call(&ctx, json!({ "write": false })).await.unwrap();
        let paged = call(
            &ctx,
            json!({ "write": false, "findings_offset": 40, "findings_limit": 5 }),
        )
        .await
        .unwrap();

        assert_eq!(full["counts"]["dangling"], json!(51));
        assert_eq!(
            paged["counts"]["dangling"], full["counts"]["dangling"],
            "the total is a fact about the corpus, not about the page: {paged:#?}"
        );
        assert_eq!(
            paged["dangling_by_source"], full["dangling_by_source"],
            "`_by_source` is the only complete view and is what lets a ZERO be answered \
             without paging at all — windowing it would remove the one instrument that \
             needs no pagination: {paged:#?}"
        );
        assert_eq!(
            paged["dangling"].as_array().unwrap().len(),
            5,
            "the page itself is windowed: {paged:#?}"
        );
        assert_eq!(
            paged["counts"]["truncated"]["dangling"],
            json!(true),
            "40 + 5 < 51, so more remains beyond this window: {paged:#?}"
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
                name: "codescout".into(),
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

    /// docs/issues/archive/2026-08-27-cross-repo-file-qualified-citation-unsupported.md
    ///
    /// A 3-part qualified citation (`<repo>:<file-stem>:<TOKEN>`) is retracted by
    /// `MalformedQualifier` regardless of what its two qualifier segments name — it
    /// never becomes an edge either way, and that part is unchanged by this test.
    /// But the `malformed_qualifier` bucket used to lump two very different causes
    /// together: a genuinely redundant same-repo prefix (should be stripped) and a
    /// citation that names a real sibling repo + file (intentional, prose-only,
    /// nothing to fix). This pins the split: the outer segment is compared against
    /// the CITING artifact's own registered workspace root name.
    #[tokio::test]
    async fn a_cross_repo_file_qualified_citation_is_reported_separately_from_a_redundant_same_repo_one(
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = tmp.path().join("target.md");
        std::fs::write(&dst, "## F-2 — anchor entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        let src = tmp.path().join("source.md");
        std::fs::write(
            &src,
            "Redundant: `citer:target:F-2`. Cross-repo: `claude-plugins:target:F-2`.\n",
        )
        .unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = tmp.path().to_path_buf();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "citer".into(),
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

        assert_eq!(
            out["counts"]["malformed_qualifier"],
            json!(1),
            "only the redundant self-repo citation stays here: {out:#?}"
        );
        assert_eq!(
            out["malformed_qualifier"][0]["raw"], "citer:target:F-2",
            "{out:#?}"
        );
        assert_eq!(
            out["counts"]["cross_repo_file_qualified"],
            json!(1),
            "the genuinely-different-repo citation gets its own bucket: {out:#?}"
        );
        assert_eq!(
            out["cross_repo_file_qualified"][0]["raw"], "claude-plugins:target:F-2",
            "{out:#?}"
        );
        assert_eq!(
            out["cross_repo_file_qualified_by_source"]["source.md"],
            json!(1),
            "per-source breakdown must mirror the other three arms: {out:#?}"
        );
        assert_eq!(
            out["counts"]["cross_repo"],
            json!(0),
            "neither must ALSO land in the plain two-part cross_repo bucket: {out:#?}"
        );
        assert_eq!(
            out["counts"]["edges_missing"],
            json!(0),
            "neither may silently resolve, even though the inner `target:F-2` form \
             would on its own: {out:#?}"
        );
    }

    /// The production shape the test above cannot reach: **no `[[roots]]` entry at
    /// all**.
    ///
    /// `ctx.workspace.roots` is the user's optional per-machine registry
    /// (`~/.config/librarian/workspace.toml`), and a repo has to be hand-added to it.
    /// codescout is not in its own — it appears there only as an umbrella *member* —
    /// so `containing_root` returned `None` for every row, `is_some_and(None)` folded
    /// that into the self-reference branch, and `cross_repo_file_qualified` was
    /// unreachable in every real repo. Measured 2026-08-27 before the fix: 0 in the
    /// new bucket against 20 `malformed_qualifier` findings in codescout, 13 of which
    /// named another repo, plus 4 of 4 in claude-plugins.
    ///
    /// The sibling test above passes throughout, because it hand-builds the `Root` a
    /// real run derives. That is the class: **a test that constructs the state
    /// production computes cannot tell you the computation works.**
    ///
    /// So this one registers nothing and leans on the git-root basename instead. The
    /// tmpdir's own name stands in for the repo name, read back rather than hardcoded,
    /// which is what makes the self-reference arm real here.
    #[tokio::test]
    async fn the_split_still_fires_when_the_repo_is_absent_from_the_roots_registry() {
        let tmp = tempfile::tempdir().unwrap();
        // A git root whose basename is a grammar-VALID repo name. `double_qualified_re`
        // requires every qualifier segment to match `[a-z][a-z0-9_-]{1,119}`, and a bare
        // tempdir basename (`.tmpAbC123`) matches none of it — so the self-repo citation
        // would never be extracted, and this test's redundant-prefix arm would assert on
        // a citation that does not exist. Found by running it: the cross-repo arm passed
        // and the self-repo arm reported zero.
        let repo_root = tmp.path().join("my-repo");
        std::fs::create_dir(&repo_root).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let dst = repo_root.join("target.md");
        std::fs::write(&dst, "## F-2 — anchor entry\n\nbody\n").unwrap();
        seed_scan_artifact(&cat, "dst", &dst, "target");

        // Read back rather than hardcoded: the production code derives this from the
        // git root, so the test must too or it stops pinning the derivation.
        let repo_name = repo_root.file_name().unwrap().to_str().unwrap().to_string();

        let src = repo_root.join("source.md");
        std::fs::write(
            &src,
            format!(
                "Redundant: `{repo_name}:target:F-2`. Cross-repo: `claude-plugins:target:F-2`.\n"
            ),
        )
        .unwrap();
        seed_scan_artifact(&cat, "src", &src, "source");

        let root = repo_root;
        let ctx = TestToolContextBuilder::new(cat)
            // Deliberately NO .with_root(...) — that is the whole point.
            .with_current_project(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                main_root: None,
                umbrella: None,
            }))
            .build();
        let out = call(&ctx, json!({ "write": true })).await.unwrap();

        assert_eq!(
            out["counts"]["cross_repo_file_qualified"],
            json!(1),
            "an unregistered repo must still reach its own bucket: {out:#?}"
        );
        assert_eq!(
            out["cross_repo_file_qualified"][0]["raw"], "claude-plugins:target:F-2",
            "{out:#?}"
        );
        assert_eq!(
            out["counts"]["malformed_qualifier"],
            json!(1),
            "and the self-repo one must still be recognised as redundant, via the \
             git-root basename rather than a registry name: {out:#?}"
        );
        assert_eq!(
            out["malformed_qualifier"][0]["raw"],
            format!("{repo_name}:target:F-2"),
            "{out:#?}"
        );
    }
}
