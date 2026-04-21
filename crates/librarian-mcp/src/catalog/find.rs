use anyhow::Result;

use super::artifact::{row_from_sql, ArtifactRow};
use super::Catalog;
use crate::filter::{compile, FilterNode};

pub struct FindOpts {
    pub filter: Option<FilterNode>,
    pub limit: usize,
    pub offset: usize,
    /// Pre-computed embedding vector for semantic KNN search.
    /// When Some, results are sorted by cosine distance (closest first)
    /// rather than updated_at. Filter AST is still applied as a post-filter
    /// on the top-K candidates.
    pub semantic: Option<Vec<f32>>,
}

pub fn find(cat: &Catalog, opts: &FindOpts) -> Result<Vec<ArtifactRow>> {
    if let Some(ref vec) = opts.semantic {
        return find_semantic(cat, opts, vec);
    }

    let mut sql = String::from(
        "SELECT id, repo, rel_path, kind, status, title, owners, tags,\
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

/// Two-phase semantic search with iterative K backfill:
///
/// 1. KNN query against artifact_vec to get top-K candidate ids by cosine distance.
/// 2. Fetch full artifact rows for those ids, applying the filter AST.
/// 3. If the post-filter result set is smaller than requested and K < K_CAP,
///    double K and retry (ensures selective filters still return results).
///
/// Results are returned in KNN distance order (closest first).
fn find_semantic(cat: &Catalog, opts: &FindOpts, query_vec: &[f32]) -> Result<Vec<ArtifactRow>> {
    // Encode as little-endian f32 bytes — the format vec_f32() expects.
    let blob: Vec<u8> = query_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

    let target = opts.limit + opts.offset;
    let mut k = (target * 5).max(100) as i64;
    const K_CAP: i64 = 2000;

    loop {
        let knn_sql = "SELECT id FROM artifact_vec \
                       WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2";
        let mut knn_stmt = cat.conn.prepare(knn_sql)?;
        let candidate_ids: Vec<String> = knn_stmt
            .query_map(rusqlite::params![blob, k], |row| row.get(0))?
            .collect::<Result<_, _>>()?;

        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        // Params list: [candidate_id_0, candidate_id_1, ..., <filter params>]
        // Candidate ids occupy positions ?1..?N.
        let n = candidate_ids.len();
        let placeholders: String = (1..=n)
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");

        // CASE WHEN id = ?1 THEN 0 WHEN id = ?2 THEN 1 ... preserves KNN order.
        let order_case: String = (0..n)
            .map(|i| format!("WHEN id = ?{} THEN {}", i + 1, i))
            .collect::<Vec<_>>()
            .join(" ");

        let mut sql = format!(
            "SELECT id, repo, rel_path, kind, status, title, owners, tags, \
             topic, time_scope, source, created_at, updated_at, file_mtime, \
             file_sha256, confidence FROM artifact \
             WHERE id IN ({placeholders})",
        );

        let mut params: Vec<rusqlite::types::Value> = candidate_ids
            .iter()
            .map(|id| rusqlite::types::Value::Text(id.clone()))
            .collect();

        if let Some(f) = &opts.filter {
            let frag = compile(f)?;
            // Filter fragment uses ?1, ?2, ... but those are already taken by candidate ids.
            // Shift all ?N in the filter SQL by n so they start at ?(n+1).
            let shifted = shift_param_indices(&frag.sql, n);
            sql.push_str(" AND ");
            sql.push_str(&shifted);
            params.extend(frag.params);
        }

        // No LIMIT/OFFSET yet — collect all matching rows to check count before deciding
        // whether to retry with a larger K.
        sql.push_str(&format!(" ORDER BY CASE {order_case} ELSE {n} END"));

        let mut stmt = cat.conn.prepare(&sql)?;
        let all_rows: Vec<ArtifactRow> = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), row_from_sql)?
            .collect::<Result<_, _>>()?;

        // If we have enough rows, or we've hit K_CAP, return the page.
        if all_rows.len() >= target || k >= K_CAP {
            return Ok(all_rows
                .into_iter()
                .skip(opts.offset)
                .take(opts.limit)
                .collect());
        }

        // Not enough results yet and we can still expand — double K and retry.
        k = (k * 2).min(K_CAP);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use serde_json::json;

    fn art(id: &str, kind: &str, status: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
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
                semantic: None,
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
                semantic: None,
            },
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }
}
