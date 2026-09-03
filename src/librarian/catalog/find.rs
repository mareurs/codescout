use anyhow::Result;

use super::artifact::{row_from_sql, ArtifactRow};
use super::Catalog;
use crate::librarian::filter::{compile, FilterNode};

pub struct FindOpts {
    pub filter: Option<FilterNode>,
    pub limit: usize,
    pub offset: usize,
}

pub fn find(cat: &Catalog, opts: &FindOpts, cutoff_ms: i64) -> Result<Vec<ArtifactRow>> {
    let mut sql = String::from(
        "SELECT id, abs_path, kind, status, title, owners, tags,\
         topic, time_scope, source, created_at, updated_at, file_mtime,\
         file_sha256, confidence FROM artifact WHERE ",
    );
    sql.push_str(&super::gc::visibility_sql(cutoff_ms));
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = &opts.filter {
        let frag = compile(f)?;
        sql.push_str(" AND (");
        sql.push_str(&frag.sql);
        sql.push(')');
        params.extend(frag.params);
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");
    params.push(rusqlite::types::Value::Integer(opts.limit as i64));
    params.push(rusqlite::types::Value::Integer(opts.offset as i64));

    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_from_sql)?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

/// Count of artifacts matching `filter`. Used by listing tools to generate
/// progressive-disclosure hints ("N more in repo, M more in workspace").
pub fn count_matching(cat: &Catalog, filter: Option<&FilterNode>, cutoff_ms: i64) -> Result<usize> {
    let mut sql = String::from("SELECT COUNT(*) FROM artifact WHERE ");
    sql.push_str(&super::gc::visibility_sql(cutoff_ms));
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = filter {
        let frag = compile(f)?;
        sql.push_str(" AND (");
        sql.push_str(&frag.sql);
        sql.push(')');
        params.extend(frag.params);
    }
    let mut stmt = cat.conn.prepare(&sql)?;
    let n: i64 = stmt.query_row(rusqlite::params_from_iter(params.iter()), |r| r.get(0))?;
    Ok(n.max(0) as usize)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogSummary {
    pub total: usize,
    pub by_kind: std::collections::BTreeMap<String, usize>,
    pub augmented: usize,
}

/// Catalog-level summary for the given scoped filter: total non-archived
/// artifact count, count by kind, and count of augmented artifacts.
/// Caller is responsible for passing a filter that already excludes
/// archived/superseded rows if desired.
pub fn catalog_summary(
    cat: &Catalog,
    scoped_filter: Option<&FilterNode>,
    cutoff_ms: i64,
) -> Result<CatalogSummary> {
    let visibility = super::gc::visibility_sql(cutoff_ms);
    let (where_sql, params) = match scoped_filter {
        Some(f) => {
            let frag = compile(f)?;
            (
                format!(" WHERE {} AND ({})", visibility, frag.sql),
                frag.params,
            )
        }
        None => (format!(" WHERE {}", visibility), Vec::new()),
    };

    let mut by_kind = std::collections::BTreeMap::new();
    let mut total = 0usize;
    {
        let sql = format!(
            "SELECT kind, COUNT(*) FROM artifact{} GROUP BY kind",
            where_sql
        );
        let mut stmt = cat.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (kind, count) = row?;
            let c = count.max(0) as usize;
            total += c;
            by_kind.insert(kind, c);
        }
    }

    let augmented = {
        let aug_sql = format!(
            "SELECT COUNT(*) FROM artifact_augmentation \
             WHERE artifact_id IN (SELECT id FROM artifact{})",
            where_sql
        );
        let mut stmt = cat.conn.prepare(&aug_sql)?;
        let n: i64 = stmt.query_row(rusqlite::params_from_iter(params.iter()), |r| r.get(0))?;
        n.max(0) as usize
    };

    Ok(CatalogSummary {
        total,
        by_kind,
        augmented,
    })
}

/// Shift all `?N` parameter placeholders in a SQL fragment by `offset`.
/// e.g. shift_param_indices("x = ?1 AND y = ?2", 3) → "x = ?4 AND y = ?5"
fn shift_param_indices(sql: &str, offset: usize) -> String {
    // Replace ?N tokens with ?{N+offset}. Walk char-by-char to avoid regex dep.
    let mut out = String::with_capacity(sql.len() + 8);
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '?' {
            // Collect digits following '?'
            let mut digits = String::new();
            while chars.peek().map(|d| d.is_ascii_digit()).unwrap_or(false) {
                digits.push(chars.next().unwrap());
            }
            if digits.is_empty() {
                out.push('?');
            } else {
                let n: usize = digits.parse().unwrap_or(1);
                out.push('?');
                out.push_str(&(n + offset).to_string());
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Direct-id lookup, NO filter and NO visibility predicate applied. This is the
/// forensic bypass: `doc(action="get")`-style and `doctor`-style access to
/// a row (including ones hidden by the grace-period visibility predicate) must
/// go through here, never through `find`/`find_by_ids_filtered`.
pub fn find_by_ids(cat: &Catalog, ids: &[String]) -> Result<Vec<ArtifactRow>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let n = ids.len();
    let placeholders: String = (1..=n)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, abs_path, kind, status, title, owners, tags, \
         topic, time_scope, source, created_at, updated_at, file_mtime, \
         file_sha256, confidence FROM artifact \
         WHERE id IN ({placeholders})",
    );
    let params: Vec<rusqlite::types::Value> = ids
        .iter()
        .map(|id| rusqlite::types::Value::Text(id.clone()))
        .collect();
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_from_sql)?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

/// Hydrate + filter a set of KNN candidate ids into artifact rows, preserving
/// the candidate (KNN-distance) order. Applies the caller's filter AST as a
/// post-filter and returns ALL matching rows (no pagination) — the caller
/// decides retry/pagination. Empty `candidate_ids` → empty. Shared by both
/// vector backends (sqlite-vec + Qdrant) so semantic results are identical
/// regardless of where the KNN ran.
///
/// This IS a search path — the grace-period visibility predicate applies
/// (`cutoff_ms`). For the unfiltered forensic bypass see `find_by_ids`.
pub fn find_by_ids_filtered(
    cat: &Catalog,
    candidate_ids: &[String],
    filter: Option<&FilterNode>,
    cutoff_ms: i64,
) -> Result<Vec<ArtifactRow>> {
    if candidate_ids.is_empty() {
        return Ok(vec![]);
    }

    // Candidate ids occupy ?1..?N; the filter fragment's ?M are shifted past N.
    let n = candidate_ids.len();
    let placeholders: String = (1..=n)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    // CASE WHEN id = ?1 THEN 0 ... preserves the candidate (KNN) order.
    let order_case: String = (0..n)
        .map(|i| format!("WHEN id = ?{} THEN {}", i + 1, i))
        .collect::<Vec<_>>()
        .join(" ");

    let mut sql = format!(
        "SELECT id, abs_path, kind, status, title, owners, tags, \
         topic, time_scope, source, created_at, updated_at, file_mtime, \
         file_sha256, confidence FROM artifact \
         WHERE id IN ({placeholders}) AND {}",
        super::gc::visibility_sql(cutoff_ms),
    );

    let mut params: Vec<rusqlite::types::Value> = candidate_ids
        .iter()
        .map(|id| rusqlite::types::Value::Text(id.clone()))
        .collect();

    if let Some(f) = filter {
        let frag = compile(f)?;
        let shifted = shift_param_indices(&frag.sql, n);
        sql.push_str(" AND ");
        sql.push_str(&shifted);
        params.extend(frag.params);
    }

    sql.push_str(&format!(" ORDER BY CASE {order_case} ELSE {n} END"));

    let mut stmt = cat.conn.prepare(&sql)?;
    let rows: Vec<ArtifactRow> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), row_from_sql)?
        .collect::<rusqlite::Result<Vec<ArtifactRow>>>()?;
    Ok(rows)
}

/// The chunk that matched, when the store is chunk-keyed.
///
/// `None` means the hit came back without a resolvable chunk row — a stale
/// vector, or an artifact-grain store. A caller rendering a snippet should fall
/// back to the artifact rather than treat this as an error.
#[derive(Debug, Clone)]
pub struct ChunkHit {
    pub chunk_id: String,
    pub chunk_ix: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub entry_token: Option<String>,
    /// Which chunk of its entry this is, and how many there are — see
    /// [`ChunkRow::entry_part`](crate::librarian::catalog::chunk::ChunkRow::entry_part).
    /// `None` for a chunk belonging to no entry, and also for any row indexed
    /// before the columns existed; a consumer must treat absence as "unknown",
    /// never as "part 1 of 1".
    pub entry_part: Option<usize>,
    pub entry_parts: Option<usize>,
    pub content: String,
}

/// One semantic hit: the hydrated row plus how far it sat from the query.
///
/// `distance` is lower-is-closer and backend-scaled — see
/// [`ArtifactVectorStore::knn`](crate::librarian::artifact_store::ArtifactVectorStore::knn).
/// Comparable within one response; never across backends or across queries.
#[derive(Debug, Clone)]
pub struct SemanticHit {
    pub row: ArtifactRow,
    pub distance: f32,
    /// The chunk whose vector matched — the plan's whole point at the retrieval
    /// layer: a hit names the ENTRY that matched, not merely the artifact
    /// containing it.
    pub chunk: Option<ChunkHit>,
}

/// A page of semantic hits, plus whether the filter starved it.
///
/// The starvation fields exist because the widen-and-retry loop already KNOWS it
/// is scraping the barrel — `find.rs` even calls the branch "Selective filter
/// starved the page" — and used to return the same shape either way. A caller
/// could not distinguish "here are five close matches" from "here are the five
/// least-bad rows left after your filter removed everything relevant".
///
/// BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
#[derive(Debug, Clone, Default)]
pub struct SemanticPage {
    pub hits: Vec<SemanticHit>,
    /// How many times the loop had to widen `k` to fill the page. `0` means the
    /// first KNN pass already had enough survivors.
    pub widenings: usize,
    /// The loop hit `K_CAP` and returned a SHORT page — the corpus, or the
    /// filter, genuinely ran out.
    pub exhausted: bool,
    /// Hits dropped by `max_per_artifact`. Distinct from `exhausted`: the page
    /// is full and relevant, but one artifact had more to say than it was
    /// allowed. A caller that cannot see this cannot tell a capped page from a
    /// corpus that simply had nothing else — the same silent-partial defect
    /// this whole change exists to fix, one level up.
    pub cap_suppressed: usize,
    /// Candidates the store returned that resolved to NO `artifact_chunk` row,
    /// and were therefore dropped before hydration.
    ///
    /// A store holding ids at a grain this reader cannot resolve is invisible
    /// without this: the page comes back short and correct-looking, and the
    /// only other signal, `exhausted`, says "the corpus ran out" — which is a
    /// different and reassuring claim. Measured 2026-09-03 on the live Qdrant
    /// collection: 2476 of 5388 points (46%) were artifact-grain, discarded on
    /// every query, and nothing anywhere counted them. See
    /// `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
    ///
    /// Reset per widening pass, like `cap_suppressed` — it describes the pass
    /// that produced `hits`, not the loop's history.
    pub unresolved: usize,
}

/// Project-scoped semantic artifact search: iterative-K backfill over a vector
/// store, hydrated + filtered through the catalog. `project_id = Some` narrows
/// the KNN to one project (the Qdrant backend filters on it; sqlite-vec ignores
/// it and relies on the catalog filter); `None` searches all. Results come back
/// in KNN-distance order, paginated by `limit`/`offset`.
///
/// The catalog lock is held only for the synchronous hydrate/filter step and
/// released before each `store.knn` await — never across an await.
///
/// **The loop's exit condition is page fullness, not relevance.** That is correct
/// for pagination and was invisible to callers: a selective filter does not shrink
/// the result set, it makes this function reach further down the KNN list until it
/// has `target` survivors. [`SemanticPage`] now reports both the reaching and the
/// distances, so "backfilled past the point of relevance" is a readable state
/// rather than an indistinguishable one.
///
/// **The store is chunk-keyed.** `knn` returns chunk ids, which are resolved to
/// their artifacts before hydrating — the catalog filter is artifact-level — and
/// the matching chunk rides along in [`SemanticHit::chunk`] so a hit names the
/// entry that matched. `max_per_artifact` bounds how many chunks one artifact
/// may contribute, applied in KNN order so each artifact keeps its BEST chunks;
/// whatever it drops is counted in [`SemanticPage::cap_suppressed`].
#[allow(clippy::too_many_arguments)]
pub async fn semantic_find(
    store: &dyn crate::librarian::artifact_store::ArtifactVectorStore,
    catalog: &parking_lot::Mutex<Catalog>,
    project_id: Option<&str>,
    query: &[f32],
    filter: Option<&FilterNode>,
    max_per_artifact: usize,
    limit: usize,
    offset: usize,
    cutoff_ms: i64,
) -> Result<SemanticPage> {
    let target = limit + offset;
    // With a per-artifact cap, candidates COLLAPSE before they are counted: one
    // ledger can contribute a hundred chunks and still yield `max_per_artifact`
    // hits, so `k` has to reach further than it did in the artifact-keyed era.
    let mut k = (target * 5 * max_per_artifact.max(1)).max(200);
    const K_CAP: usize = 8000;
    let mut widenings = 0usize;

    loop {
        let candidates = store.knn(project_id, query, k).await?;
        if candidates.is_empty() {
            return Ok(SemanticPage::default());
        }

        // `knn` returns CHUNK ids. Resolve each to its artifact so the catalog
        // filter still applies, and keep the chunk so the hit can name the entry.
        let chunk_rows = {
            let cat = catalog.lock();
            crate::librarian::catalog::chunk::rows_by_chunk_ids(
                &cat,
                &candidates
                    .iter()
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>(),
            )?
        };

        // KNN order IS the ranking, so walk it in order: the cap then keeps each
        // artifact's BEST chunks rather than an arbitrary `max_per_artifact` of
        // them. A chunk id with no row is stale, not an error — skip it.
        let mut seen_per_artifact: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut ordered: Vec<(ChunkHit, String, f32)> = Vec::new();
        let mut cap_suppressed = 0usize;
        let mut unresolved = 0usize;
        for (chunk_id, distance) in &candidates {
            let Some(row) = chunk_rows.get(chunk_id) else {
                // Counted, not merely skipped. The skip itself is right — a
                // stale vector is not an error — but an UNCOUNTED skip makes a
                // store holding ids at a grain this reader cannot resolve look
                // like a small corpus. See `SemanticPage::unresolved`.
                unresolved += 1;
                continue;
            };
            let n = seen_per_artifact
                .entry(row.artifact_id.clone())
                .or_insert(0);
            if *n >= max_per_artifact {
                cap_suppressed += 1;
                continue;
            }
            *n += 1;
            ordered.push((
                ChunkHit {
                    chunk_id: row.chunk_id.clone(),
                    chunk_ix: row.chunk_ix,
                    start_line: row.start_line,
                    end_line: row.end_line,
                    entry_token: row.entry_token.clone(),
                    entry_part: row.entry_part,
                    entry_parts: row.entry_parts,
                    content: row.content.clone(),
                },
                row.artifact_id.clone(),
                *distance,
            ));
        }

        // Distinct artifact ids in first-appearance (best-chunk) order. NOT
        // `dedup()`, which only collapses ADJACENT duplicates: two ledgers'
        // chunks interleave in KNN order routinely, so `dedup` would leave
        // repeats and hydrate the same artifact several times.
        let candidate_ids: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            ordered
                .iter()
                .map(|(_, a, _)| a.clone())
                .filter(|a| seen.insert(a.clone()))
                .collect()
        };

        let all_rows = {
            let cat = catalog.lock();
            find_by_ids_filtered(&cat, &candidate_ids, filter, cutoff_ms)?
        };
        let surviving: std::collections::HashMap<String, ArtifactRow> =
            all_rows.into_iter().map(|r| (r.id.clone(), r)).collect();

        // Hits are CHUNKS whose artifact survived the filter, still in KNN order.
        let hits_all: Vec<SemanticHit> = ordered
            .iter()
            .filter_map(|(chunk, artifact_id, distance)| {
                surviving.get(artifact_id).map(|row| SemanticHit {
                    row: row.clone(),
                    distance: *distance,
                    chunk: Some(chunk.clone()),
                })
            })
            .collect();

        // `target` counts SURVIVING hits after the cap, not raw candidates —
        // otherwise a big ledger's suppressed chunks would read as a full page.
        let enough = hits_all.len() >= target;
        // The vector store returned fewer candidates than asked for, so it holds
        // nothing further and widening `k` cannot find more. Without this the loop
        // re-queried a 2-row corpus five more times on its way to K_CAP, and —
        // worse — every small-corpus query came out looking filter-starved when
        // the truth was simply that the corpus is small. A signal that fires when
        // it is not true is worse than no signal.
        let store_exhausted = candidates.len() < k;
        let capped = k >= K_CAP;

        // Enough results, or nothing left to find → return the requested page.
        if enough || store_exhausted || capped {
            let hits = hits_all.into_iter().skip(offset).take(limit).collect();
            return Ok(SemanticPage {
                hits,
                widenings,
                // A SHORT page means the search ran out — of corpus, or of budget.
                // A full page at K_CAP is the ordinary large-corpus case and is not
                // exhaustion.
                exhausted: !enough,
                cap_suppressed,
                unresolved,
            });
        }

        // Selective filter starved the page — the store had at least `k` candidates
        // and the filter removed enough of them to leave the page short. Widen and
        // retry. `widenings > 0` is therefore the genuine filter-starvation signal:
        // corpus exhaustion exits above without ever incrementing it.
        k = (k * 2).min(K_CAP);
        widenings += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow, TestArtifactRowBuilder};
    use crate::librarian::catalog::gc;
    use serde_json::json;

    fn art(id: &str, kind: &str, status: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_abs_path(format!("/test/{id}.md"))
            .with_kind(kind)
            .with_status(status)
            .with_tags(vec!["t".into()])
            .with_updated_at(id.chars().last().map(|c| c as i64).unwrap_or(0))
            .with_file_sha256("x")
            .build()
    }

    #[test]
    fn find_by_kind() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "plan", "active")).unwrap();
        let rows = find(
            &cat,
            &FindOpts {
                filter: Some(serde_json::from_value(json!({"kind": {"eq": "spec"}})).unwrap()),
                limit: 10,
                offset: 0,
            },
            0,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }

    #[test]
    fn find_with_and_composition() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "spec", "archived")).unwrap();
        let rows = find(
            &cat,
            &FindOpts {
                filter: Some(
                    serde_json::from_value(json!({"and": [
                        {"kind": {"eq": "spec"}},
                        {"status": {"eq": "active"}}
                    ]}))
                    .unwrap(),
                ),
                limit: 10,
                offset: 0,
            },
            0,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }
    #[tokio::test]
    async fn semantic_find_orders_by_knn_and_hydrates() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("c", "plan", "active")).unwrap();

        let (ca, cb, cc) = (
            one_chunk(&cat, "a"),
            one_chunk(&cat, "b"),
            one_chunk(&cat, "c"),
        );
        let store = InMemoryArtifactStore::default();
        store.upsert("proj", &ca, "a", &[1.0, 0.0]).await.unwrap();
        store.upsert("proj", &cb, "b", &[0.8, 0.2]).await.unwrap();
        store.upsert("proj", &cc, "c", &[0.0, 1.0]).await.unwrap();

        let cat = parking_lot::Mutex::new(cat);
        let rows = semantic_find(&store, &cat, Some("proj"), &[1.0, 0.0], None, 1, 10, 0, 0)
            .await
            .unwrap();
        let ids: Vec<&str> = rows.hits.iter().map(|h| h.row.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"], "KNN distance order");
    }

    #[tokio::test]
    async fn a_candidate_with_no_chunk_row_is_counted_not_silently_dropped() {
        // The instrument whose absence hid a 46% loss for a whole session.
        // `semantic_find` resolves every id the store returns through
        // `artifact_chunk` and skips the misses. That skip is correct — a stale
        // vector is not an error — but it was UNCOUNTED, so a store holding ids
        // at a grain this reader cannot resolve produced a short, healthy-looking
        // page. Measured on the live Qdrant collection 2026-09-03: 2476 of 5388
        // points were artifact-grain and were discarded on every query.
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        let ca = one_chunk(&cat, "a");

        let store = InMemoryArtifactStore::default();
        store.upsert("proj", &ca, "a", &[1.0, 0.0]).await.unwrap();
        // LOAD-BEARING: a 16-hex id, which is exactly the shape of an
        // ARTIFACT id and therefore can never be an `artifact_chunk.chunk_id`
        // (those are UUID v4). This is what an artifact-grain point looks like
        // to a chunk-grain reader — the real defect, not an invented one. Give
        // it a vector CLOSE to the query so it is genuinely a candidate; a
        // distant one would be dropped by ranking and prove nothing.
        // Both ids are the SAME 16-hex value, which is precisely what a
        // pre-2026-09-03 point looks like: written when `upsert` had one slot,
        // so the artifact id was stored as the vector's identity too.
        store
            .upsert(
                "proj",
                "0123456789abcdef",
                "0123456789abcdef",
                &[0.99, 0.01],
            )
            .await
            .unwrap();

        let cat = parking_lot::Mutex::new(cat);
        // LOAD-BEARING `limit = 1`, not 10. `exhausted` means "the page came back
        // SHORT", so at limit=10 a two-point corpus sets it for a perfectly
        // legitimate reason and the last assertion below cannot discriminate.
        // At limit=1 the page is FULL and a candidate was still discarded, which
        // is the case that separates the two signals.
        let page = semantic_find(&store, &cat, Some("proj"), &[1.0, 0.0], None, 1, 1, 0, 0)
            .await
            .unwrap();

        assert_eq!(
            page.hits.len(),
            1,
            "only the resolvable candidate may hydrate"
        );
        assert_eq!(
            page.unresolved, 1,
            "the unresolvable candidate must be COUNTED, not silently dropped"
        );
        // The two states are distinct and must not be conflated: the page is
        // FULL, so nothing ran out — a candidate was discarded. Asserting only
        // `unresolved` would pass a change that also set `exhausted`, which
        // tells the caller the opposite thing.
        assert!(
            !page.exhausted,
            "a discarded candidate on a full page is not corpus exhaustion"
        );
    }

    #[tokio::test]
    async fn semantic_find_applies_catalog_filter() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "plan", "active")).unwrap();

        let (ca, cb) = (one_chunk(&cat, "a"), one_chunk(&cat, "b"));
        let store = InMemoryArtifactStore::default();
        store.upsert("proj", &ca, "a", &[1.0, 0.0]).await.unwrap();
        store.upsert("proj", &cb, "b", &[0.9, 0.1]).await.unwrap();

        let cat = parking_lot::Mutex::new(cat);
        let filter: FilterNode = serde_json::from_value(json!({"kind": {"eq": "spec"}})).unwrap();
        let rows = semantic_find(&store, &cat, None, &[1.0, 0.0], Some(&filter), 1, 10, 0, 0)
            .await
            .unwrap();
        let ids: Vec<&str> = rows.hits.iter().map(|h| h.row.id.as_str()).collect();
        assert_eq!(ids, vec!["a"], "the plan artifact is filtered out");
    }

    #[test]
    fn catalog_summary_counts_by_kind_and_total() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        for (id, kind) in [("a1", "tracker"), ("a2", "tracker"), ("a3", "plan")] {
            upsert(
                &cat,
                &ArtifactRow {
                    id: id.into(),
                    abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
                    kind: kind.into(),
                    status: "draft".into(),
                    title: None,
                    owners: vec![],
                    tags: vec![],
                    topic: None,
                    time_scope: None,
                    source: None,
                    created_at: now,
                    updated_at: now,
                    file_mtime: now,
                    file_sha256: "".into(),
                    confidence: 1.0,
                },
            )
            .unwrap();
        }
        let s = catalog_summary(&cat, None, 0).unwrap();
        assert_eq!(s.total, 3);
        assert_eq!(s.by_kind["tracker"], 2);
        assert_eq!(s.by_kind["plan"], 1);
        assert_eq!(s.augmented, 0);
    }

    #[test]
    fn catalog_summary_counts_augmented() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        use crate::librarian::catalog::augmentation;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let now_ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a1".into(),
                abs_path: std::path::PathBuf::from("/test/r/a1.md"),
                kind: "tracker".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a2".into(),
                abs_path: std::path::PathBuf::from("/test/r/a2.md"),
                kind: "plan".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &crate::librarian::catalog::augmentation::AugmentationRow {
                artifact_id: "a1".into(),
                prompt: "track".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now_ts.clone(),
                updated_at: now_ts,
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let s = catalog_summary(&cat, None, 0).unwrap();
        assert_eq!(s.total, 2);
        assert_eq!(s.augmented, 1);
    }

    #[test]
    fn catalog_summary_respects_scoped_filter() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        use crate::librarian::catalog::augmentation;
        use crate::librarian::filter::FilterNode;
        use serde_json::json;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let now_ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        for (id, repo) in [("a1", "repo-a"), ("a2", "repo-b")] {
            upsert(
                &cat,
                &ArtifactRow {
                    id: id.into(),
                    abs_path: std::path::PathBuf::from(format!("/{repo}/{id}.md")),
                    kind: "plan".into(),
                    status: "draft".into(),
                    title: None,
                    owners: vec![],
                    tags: vec![],
                    topic: None,
                    time_scope: None,
                    source: None,
                    created_at: now,
                    updated_at: now,
                    file_mtime: now,
                    file_sha256: "".into(),
                    confidence: 1.0,
                },
            )
            .unwrap();
        }
        // Augment the repo-b artifact — filter to repo-a must exclude it
        augmentation::upsert(
            &cat,
            &crate::librarian::catalog::augmentation::AugmentationRow {
                artifact_id: "a2".into(),
                prompt: "track".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now_ts.clone(),
                updated_at: now_ts,
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let f = FilterNode::Leaf(
            [("abs_path".to_string(), json!({"prefix": "/repo-a/"}))]
                .into_iter()
                .collect(),
        );
        let s = catalog_summary(&cat, Some(&f), 0).unwrap();
        assert_eq!(s.total, 1);
        assert_eq!(
            s.augmented, 0,
            "augmented count must respect the scope filter"
        );
    }

    #[test]
    fn find_hides_rows_missing_past_grace_and_shows_within_grace() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        // present row
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256) \
            VALUES ('live','/x/a.md','tracker','active','a',0,10,0,'x')", []).unwrap();
        // missing long ago (before cutoff) → hidden
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256,missing_since) \
            VALUES ('old','/x/b.md','tracker','active','b',0,9,0,'x', 100)", []).unwrap();
        // missing recently (within grace, after cutoff) → visible
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256,missing_since) \
            VALUES ('new','/x/c.md','tracker','active','c',0,8,0,'x', 5000)", []).unwrap();

        let cutoff = 1000i64; // rows with missing_since <= 1000 are hidden
        let opts = FindOpts {
            filter: None,
            limit: 100,
            offset: 0,
        };
        let rows = find(&cat, &opts, cutoff).unwrap();
        let ids: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
        assert!(ids.contains(&"live".to_string()));
        assert!(ids.contains(&"new".to_string()));
        assert!(
            !ids.contains(&"old".to_string()),
            "old missing row is hidden"
        );
        assert_eq!(gc::hidden_count(&cat.conn, cutoff).unwrap(), 1);
    }

    #[test]
    fn get_by_id_still_returns_hidden_row() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256,missing_since) \
            VALUES ('h','/x/h.md','tracker','active','h',0,1,0,'x', 100)", []).unwrap();
        // the direct-id path (used by artifact get) ignores visibility
        let rows = find_by_ids(&cat, &["h".to_string()]).unwrap();
        assert_eq!(rows.len(), 1);
    }
    #[test]
    fn count_matching_excludes_rows_missing_past_grace() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("live", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("old", "spec", "active")).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 100 WHERE id = 'old'",
                [],
            )
            .unwrap();

        let cutoff = 1000i64; // rows with missing_since <= 1000 are hidden
        let n = count_matching(&cat, None, cutoff).unwrap();
        assert_eq!(n, 1, "hidden row must not be counted");
    }

    #[test]
    fn catalog_summary_excludes_hidden_rows() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        use crate::librarian::catalog::augmentation;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let now_ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        for id in ["live", "old"] {
            upsert(
                &cat,
                &ArtifactRow {
                    id: id.into(),
                    abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
                    kind: "tracker".into(),
                    status: "draft".into(),
                    title: None,
                    owners: vec![],
                    tags: vec![],
                    topic: None,
                    time_scope: None,
                    source: None,
                    created_at: now,
                    updated_at: now,
                    file_mtime: now,
                    file_sha256: "".into(),
                    confidence: 1.0,
                },
            )
            .unwrap();
        }
        // "old" is missing past the grace cutoff AND augmented — both the
        // by_kind/total counts and the augmented subquery must exclude it.
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 100 WHERE id = 'old'",
                [],
            )
            .unwrap();
        augmentation::upsert(
            &cat,
            &crate::librarian::catalog::augmentation::AugmentationRow {
                artifact_id: "old".into(),
                prompt: "track".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now_ts.clone(),
                updated_at: now_ts,
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let cutoff = 1000i64;
        let s = catalog_summary(&cat, None, cutoff).unwrap();
        assert_eq!(s.total, 1, "hidden row excluded from total");
        assert_eq!(s.by_kind["tracker"], 1, "hidden row excluded from by_kind");
        assert_eq!(
            s.augmented, 0,
            "hidden row excluded from the augmented subquery too"
        );
    }

    #[test]
    fn find_by_ids_filtered_excludes_hidden_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("live", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("old", "spec", "active")).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 100 WHERE id = 'old'",
                [],
            )
            .unwrap();

        let cutoff = 1000i64;
        let ids = vec!["live".to_string(), "old".to_string()];
        let rows = find_by_ids_filtered(&cat, &ids, None, cutoff).unwrap();
        let ids_out: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
        assert!(ids_out.contains(&"live".to_string()));
        assert!(
            !ids_out.contains(&"old".to_string()),
            "hidden row excluded from id-set lookup"
        );
    }

    #[tokio::test]
    async fn semantic_find_excludes_hidden_rows() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("live", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("old", "spec", "active")).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 100 WHERE id = 'old'",
                [],
            )
            .unwrap();

        let (c_live, c_old) = (one_chunk(&cat, "live"), one_chunk(&cat, "old"));
        let store = InMemoryArtifactStore::default();
        store
            .upsert("proj", &c_live, "live", &[1.0, 0.0])
            .await
            .unwrap();
        store
            .upsert("proj", &c_old, "old", &[0.9, 0.1])
            .await
            .unwrap();

        let cat = parking_lot::Mutex::new(cat);
        let cutoff = 1000i64;
        let rows = semantic_find(
            &store,
            &cat,
            Some("proj"),
            &[1.0, 0.0],
            None,
            1,
            10,
            0,
            cutoff,
        )
        .await
        .unwrap();
        let ids: Vec<&str> = rows.hits.iter().map(|h| h.row.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["live"],
            "hidden row excluded from semantic search results"
        );
    }

    /// Give `id` a single chunk and return its chunk id, so a test can upsert a
    /// CHUNK-keyed vector while still asserting about artifacts.
    ///
    /// The vector store has been chunk-keyed since Task 7. A test that upserts
    /// an ARTIFACT id is exercising a contract that no longer exists — and it
    /// fails SILENTLY rather than loudly, because an unresolvable candidate id
    /// is skipped as stale, so the page simply comes back empty.
    fn one_chunk(cat: &Catalog, id: &str) -> String {
        let built = crate::librarian::catalog::chunk::build_chunks(id, "# T\n\nbody\n", 2048, 0);
        let rows = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built).unwrap();
        rows[0].chunk_id.clone()
    }

    /// A unit vector with `1.0` at `i`. Orthogonal per index, so a query for
    /// index `j` selects chunk `j` deterministically under cosine.
    fn unit(i: usize, n: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; n];
        v[i] = 1.0;
        v
    }

    /// One artifact whose body yields a preamble chunk plus two entry chunks,
    /// each embedded with a distinct orthogonal vector. Returns the chunk rows
    /// in `chunk_ix` order so a test can name the one it wants by POSITION —
    /// never by a literal id, which would not survive a chunking change.
    async fn fixture_two_entry_artifact() -> (
        parking_lot::Mutex<Catalog>,
        crate::librarian::artifact_store::test_support::InMemoryArtifactStore,
        Vec<crate::librarian::catalog::chunk::ChunkRow>,
    ) {
        use crate::librarian::artifact_store::ArtifactVectorStore;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let built = crate::librarian::catalog::chunk::build_chunks(
            "a",
            "# T\n\nintro\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
            2048,
            0,
        );
        let rows = crate::librarian::catalog::chunk::replace_chunks(&cat, "a", &built).unwrap();
        assert!(
            rows.len() > 2,
            "fixture must yield a preamble plus >1 entry chunk, got {}",
            rows.len()
        );
        let store =
            crate::librarian::artifact_store::test_support::InMemoryArtifactStore::default();
        for (i, r) in rows.iter().enumerate() {
            store
                .upsert("proj", &r.chunk_id, &r.artifact_id, &unit(i, rows.len()))
                .await
                .unwrap();
        }
        (parking_lot::Mutex::new(cat), store, rows)
    }

    /// Two artifacts: `big` with six entry chunks that all rank ABOVE `small`'s
    /// single one. The ranking gap is what makes the cap observable — without
    /// it, `small` could reach the page by luck rather than by the cap yielding.
    async fn fixture_big_and_small() -> (
        parking_lot::Mutex<Catalog>,
        crate::librarian::artifact_store::test_support::InMemoryArtifactStore,
        usize,
    ) {
        use crate::librarian::artifact_store::ArtifactVectorStore;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("big", "tracker", "active")).unwrap();
        artifact::upsert(&cat, &art("small", "tracker", "active")).unwrap();
        let big_body = (1..=6)
            .map(|i| format!("## W-{i} — t\n\nbody {i}\n"))
            .collect::<Vec<_>>()
            .join("\n");
        let big = crate::librarian::catalog::chunk::replace_chunks(
            &cat,
            "big",
            &crate::librarian::catalog::chunk::build_chunks("big", &big_body, 2048, 0),
        )
        .unwrap();
        let small = crate::librarian::catalog::chunk::replace_chunks(
            &cat,
            "small",
            &crate::librarian::catalog::chunk::build_chunks(
                "small",
                "## W-9 — t\n\nbody\n",
                2048,
                0,
            ),
        )
        .unwrap();
        let store =
            crate::librarian::artifact_store::test_support::InMemoryArtifactStore::default();
        // Query is [1, 0]. `big`'s chunks sit essentially on it; `small`'s is
        // 45° away, so every `big` chunk outranks it.
        for (i, r) in big.iter().enumerate() {
            store
                .upsert("proj", &r.chunk_id, &r.artifact_id, &[1.0, 0.01 * i as f32])
                .await
                .unwrap();
        }
        for r in &small {
            store
                .upsert("proj", &r.chunk_id, &r.artifact_id, &[0.5, 0.5])
                .await
                .unwrap();
        }
        (parking_lot::Mutex::new(cat), store, big.len())
    }

    #[tokio::test]
    async fn a_hit_names_the_chunk_that_matched_not_the_preamble() {
        // The whole point of the plan, at the retrieval layer: a hit must name
        // the ENTRY that matched, not the artifact's opening chunk.
        let (cat, store, rows) = fixture_two_entry_artifact().await;
        let want = rows.last().unwrap();
        let q = unit(rows.len() - 1, rows.len());
        let page = semantic_find(&store, &cat, None, &q, None, 3, 10, 0, 0)
            .await
            .unwrap();
        let hit = &page.hits[0];
        let chunk = hit.chunk.as_ref().expect("chunk-grain hit");
        assert_eq!(chunk.chunk_id, want.chunk_id);
        assert_eq!(chunk.entry_token.as_deref(), Some("W-2"));
        assert!(chunk.start_line > 1, "must not be the preamble chunk");
        assert!(chunk.content.contains("beta"));
        // Still hydrates to its ARTIFACT — chunk grain in, artifact identity out.
        assert_eq!(hit.row.id, "a");
    }

    #[tokio::test]
    async fn max_per_artifact_caps_without_emptying_the_page() {
        // All three clauses are required. "no more than 3" is an absence
        // assertion, and a cap that drops EVERYTHING satisfies it.
        let (cat, store, big_n) = fixture_big_and_small().await;
        let page = semantic_find(&store, &cat, None, &[1.0, 0.0], None, 3, 10, 0, 0)
            .await
            .unwrap();
        let from_big = page.hits.iter().filter(|h| h.row.id == "big").count();
        assert_eq!(from_big, 3, "capped at 3, got {from_big}");
        assert_eq!(
            page.cap_suppressed,
            big_n - 3,
            "and it reports exactly what it suppressed"
        );
        assert!(
            page.hits.iter().any(|h| h.row.id == "small"),
            "a lower-ranked chunk from another artifact must still make the page — \
             without this clause a cap that drops everything passes"
        );
    }

    #[tokio::test]
    async fn max_per_artifact_one_yields_distinct_artifacts() {
        // Assert DISTINCTNESS, not a count: a count of N is satisfied by N
        // chunks of ONE ledger, which is the regression this prevents.
        let (cat, store, _) = fixture_big_and_small().await;
        let page = semantic_find(&store, &cat, None, &[1.0, 0.0], None, 1, 10, 0, 0)
            .await
            .unwrap();
        let mut ids: Vec<&str> = page.hits.iter().map(|h| h.row.id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            before,
            "max_per_artifact=1 must yield distinct artifacts"
        );
        assert_eq!(before, 2, "and both artifacts must be represented");
    }

    #[tokio::test]
    async fn an_id_from_the_production_embed_queue_hydrates_through_semantic_find() {
        // RELOCATED from Task 7, where it could not go green: hydration is this
        // task's work. LOAD-BEARING: the ids fed to the store MUST come from
        // `embed_queue_items`, never from a literal. Every other semantic_find
        // test here hand-feeds an ARTIFACT id, which is exactly why all of them
        // stayed green when Task 6 re-keyed the queue to chunk ids and
        // hydration broke outright.
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let queue = crate::librarian::indexer::embed_queue_items(
            &cat,
            "a",
            Some("T".into()),
            "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
            crate::librarian::catalog::chunk::ChunkGrain::Chunk,
        )
        .unwrap();
        // Without >1 chunk the grain bug is UNREPRESENTABLE by this fixture.
        assert!(
            queue.len() > 1,
            "fixture must yield >1 chunk, got {}",
            queue.len()
        );

        let store = InMemoryArtifactStore::default();
        for item in &queue {
            store
                .upsert("proj", &item.chunk_id, &item.artifact_id, &[1.0, 0.0])
                .await
                .unwrap();
        }

        let cat = parking_lot::Mutex::new(cat);
        let page = semantic_find(&store, &cat, Some("proj"), &[1.0, 0.0], None, 1, 10, 0, 0)
            .await
            .unwrap();
        assert_eq!(
            page.hits
                .iter()
                .map(|h| h.row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a"],
            "a chunk id from the production queue must hydrate to its ARTIFACT, exactly once"
        );
        // The widening loop is the second half of the symptom: on the broken
        // path neither `enough` nor `store_exhausted` holds, so semantic_find
        // climbs toward K_CAP and re-queries repeatedly before returning empty.
        // The empty page is the visible failure; the burn is the expensive one.
        assert_eq!(page.widenings, 0, "hydration must not need to widen");
    }

    #[tokio::test]
    async fn the_real_sqlite_path_writes_and_reads_the_same_table() {
        // NOT in the plan, and it is the one that pins the actual outage. Every
        // other test here uses InMemoryArtifactStore, which covers the
        // hydration half and cannot see the half that broke: the production
        // WRITER targets artifact_vec_v2 (Task 7) while the production READER
        // — SqliteVecArtifactStore::knn — read artifact_vec. Writer and reader
        // on different tables is invisible to any test that supplies its own
        // store, because the table never enters the picture.
        use crate::librarian::artifact_store::{ArtifactVectorStore, SqliteVecArtifactStore};

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "tracker", "active")).unwrap();
        let queue = crate::librarian::indexer::embed_queue_items(
            &cat,
            "a",
            Some("T".into()),
            "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
            crate::librarian::catalog::chunk::ChunkGrain::Chunk,
        )
        .unwrap();
        assert!(queue.len() > 1, "fixture must yield >1 chunk");

        let shared = std::sync::Arc::new(parking_lot::Mutex::new(cat));
        let store = SqliteVecArtifactStore::new(shared.clone());
        // Write through the PRODUCTION writer, read through the PRODUCTION
        // reader. Nothing in this test names a table.
        for (i, item) in queue.iter().enumerate() {
            // 768-dim: `artifact_vec_v2` is declared `vec0(id, embedding
            // FLOAT[768])`, so a shorter vector is rejected by SQL rather than
            // by anything this test is about.
            store
                .upsert("proj", &item.chunk_id, &item.artifact_id, &unit(i, 768))
                .await
                .unwrap();
        }

        let page = semantic_find(
            &store,
            &shared,
            None,
            &unit(queue.len() - 1, 768),
            None,
            3,
            10,
            0,
            0,
        )
        .await
        .unwrap();
        assert!(
            !page.hits.is_empty(),
            "the production writer and reader must agree on a table — an empty \
             page here is the end-to-end outage, not a fixture problem"
        );
        assert_eq!(page.hits[0].row.id, "a");
        assert!(
            page.hits[0].chunk.is_some(),
            "and the hit must carry the chunk that matched"
        );
    }

    #[test]
    fn find_hides_row_at_exact_cutoff_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        // missing_since exactly equals cutoff. Predicate is `missing_since > cutoff`,
        // so equality must NOT satisfy visibility → hidden. Guards a `>` → `>=` mutation.
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256,missing_since) \
            VALUES ('boundary','/x/b.md','tracker','active','b',0,9,0,'x', 1000)", []).unwrap();

        let cutoff = 1000i64;
        let opts = FindOpts {
            filter: None,
            limit: 100,
            offset: 0,
        };
        let rows = find(&cat, &opts, cutoff).unwrap();
        assert!(
            !rows.iter().any(|r| r.id == "boundary"),
            "missing_since == cutoff must be hidden (predicate is `> cutoff`, not `>=`)"
        );
    }
}
