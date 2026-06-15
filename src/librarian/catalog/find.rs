use anyhow::Result;

use super::artifact::{row_from_sql, ArtifactRow};
use super::Catalog;
use crate::librarian::filter::{compile, FilterNode};

pub struct FindOpts {
    pub filter: Option<FilterNode>,
    pub limit: usize,
    pub offset: usize,
}

pub fn find(cat: &Catalog, opts: &FindOpts) -> Result<Vec<ArtifactRow>> {
    let mut sql = String::from(
        "SELECT id, abs_path, kind, status, title, owners, tags,\
         topic, time_scope, source, created_at, updated_at, file_mtime,\
         file_sha256, confidence FROM artifact",
    );
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = &opts.filter {
        let frag = compile(f)?;
        sql.push_str(" WHERE ");
        sql.push_str(&frag.sql);
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
pub fn count_matching(cat: &Catalog, filter: Option<&FilterNode>) -> Result<usize> {
    let mut sql = String::from("SELECT COUNT(*) FROM artifact");
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = filter {
        let frag = compile(f)?;
        sql.push_str(" WHERE ");
        sql.push_str(&frag.sql);
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
) -> Result<CatalogSummary> {
    let (where_sql, params) = match scoped_filter {
        Some(f) => {
            let frag = compile(f)?;
            (format!(" WHERE {}", frag.sql), frag.params)
        }
        None => (String::new(), Vec::new()),
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
/// Hydrate + filter a set of KNN candidate ids into artifact rows, preserving
/// the candidate (KNN-distance) order. Applies the caller's filter AST as a
/// post-filter and returns ALL matching rows (no pagination) — the caller
/// decides retry/pagination. Empty `candidate_ids` → empty. Shared by both
/// vector backends (sqlite-vec + Qdrant) so semantic results are identical
/// regardless of where the KNN ran.
pub fn find_by_ids_filtered(
    cat: &Catalog,
    candidate_ids: &[String],
    filter: Option<&FilterNode>,
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
         WHERE id IN ({placeholders})",
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

/// Project-scoped semantic artifact search: iterative-K backfill over a vector
/// store, hydrated + filtered through the catalog. `project_id = Some` narrows
/// the KNN to one project (the Qdrant backend filters on it; sqlite-vec ignores
/// it and relies on the catalog filter); `None` searches all. Results come back
/// in KNN-distance order, paginated by `limit`/`offset`.
///
/// The catalog lock is held only for the synchronous hydrate/filter step and
/// released before each `store.knn` await — never across an await.
pub async fn semantic_find(
    store: &dyn crate::librarian::artifact_store::ArtifactVectorStore,
    catalog: &parking_lot::Mutex<Catalog>,
    project_id: Option<&str>,
    query: &[f32],
    filter: Option<&FilterNode>,
    limit: usize,
    offset: usize,
) -> Result<Vec<ArtifactRow>> {
    let target = limit + offset;
    let mut k = (target * 5).max(100);
    const K_CAP: usize = 2000;

    loop {
        let candidate_ids = store.knn(project_id, query, k).await?;
        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        let all_rows = {
            let cat = catalog.lock();
            find_by_ids_filtered(&cat, &candidate_ids, filter)?
        };

        // Enough results, or KNN exhausted → return the requested page.
        if all_rows.len() >= target || k >= K_CAP {
            return Ok(all_rows.into_iter().skip(offset).take(limit).collect());
        }

        // Selective filter starved the page — widen K and retry.
        k = (k * 2).min(K_CAP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow};
    use serde_json::json;

    fn art(id: &str, kind: &str, status: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: kind.into(),
            status: status.into(),
            title: None,
            owners: vec![],
            tags: vec!["t".into()],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: id.chars().last().map(|c| c as i64).unwrap_or(0),
            file_mtime: 0,
            file_sha256: "x".into(),
            confidence: 1.0,
        }
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

        let store = InMemoryArtifactStore::default();
        store.upsert("proj", "a", &[1.0, 0.0]).await.unwrap();
        store.upsert("proj", "b", &[0.8, 0.2]).await.unwrap();
        store.upsert("proj", "c", &[0.0, 1.0]).await.unwrap();

        let cat = parking_lot::Mutex::new(cat);
        let rows = semantic_find(&store, &cat, Some("proj"), &[1.0, 0.0], None, 10, 0)
            .await
            .unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"], "KNN distance order");
    }

    #[tokio::test]
    async fn semantic_find_applies_catalog_filter() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        artifact::upsert(&cat, &art("b", "plan", "active")).unwrap();

        let store = InMemoryArtifactStore::default();
        store.upsert("proj", "a", &[1.0, 0.0]).await.unwrap();
        store.upsert("proj", "b", &[0.9, 0.1]).await.unwrap();

        let cat = parking_lot::Mutex::new(cat);
        let filter: FilterNode = serde_json::from_value(json!({"kind": {"eq": "spec"}})).unwrap();
        let rows = semantic_find(&store, &cat, None, &[1.0, 0.0], Some(&filter), 10, 0)
            .await
            .unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
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
        let s = catalog_summary(&cat, None).unwrap();
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
            },
        )
        .unwrap();
        let s = catalog_summary(&cat, None).unwrap();
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
            },
        )
        .unwrap();
        let f = FilterNode::Leaf(
            [("abs_path".to_string(), json!({"prefix": "/repo-a/"}))]
                .into_iter()
                .collect(),
        );
        let s = catalog_summary(&cat, Some(&f)).unwrap();
        assert_eq!(s.total, 1);
        assert_eq!(
            s.augmented, 0,
            "augmented count must respect the scope filter"
        );
    }
}
