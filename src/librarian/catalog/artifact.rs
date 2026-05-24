use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRow {
    pub id: String,
    pub abs_path: std::path::PathBuf,
    pub kind: String,
    pub status: String,
    pub title: Option<String>,
    pub owners: Vec<String>,
    pub tags: Vec<String>,
    pub topic: Option<String>,
    pub time_scope: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub file_mtime: i64,
    pub file_sha256: String,
    pub confidence: f64,
}

pub fn upsert(cat: &Catalog, row: &ArtifactRow) -> Result<()> {
    // F-6a fix (bug-tracker #5): the artifact schema declares
    // `abs_path TEXT NOT NULL UNIQUE`, but the INSERT below only handles
    // `ON CONFLICT(id)`. A row at the same abs_path with a *different* id
    // (e.g. caused by an id-algorithm change across catalog versions, or
    // path normalization drift between walks) would trigger an unhandled
    // UNIQUE constraint failure.
    //
    // The safe pre-clean: remove any row whose abs_path matches but id
    // differs. The natural identity of a file in this catalog is its
    // abs_path; the id is a derived hash. When the two diverge, the
    // abs_path wins (file content survives across id-algorithm changes;
    // the old id-based row is stale).
    let abs_path_str = crate::util::fs::to_forward_slash(&row.abs_path);
    cat.conn.execute(
        "DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2",
        params![abs_path_str, row.id],
    )?;

    cat.conn.execute(
        "INSERT INTO artifact (id, abs_path, kind, status, title, owners, tags,
            topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            abs_path=excluded.abs_path,
            kind=excluded.kind, status=excluded.status,
            title=excluded.title, owners=excluded.owners, tags=excluded.tags,
            topic=excluded.topic, time_scope=excluded.time_scope,
            source=excluded.source, updated_at=excluded.updated_at,
            file_mtime=excluded.file_mtime, file_sha256=excluded.file_sha256,
            confidence=excluded.confidence",
        params![
            row.id,
            abs_path_str,
            row.kind,
            row.status,
            row.title,
            serde_json::to_string(&row.owners)?,
            serde_json::to_string(&row.tags)?,
            row.topic,
            row.time_scope,
            row.source,
            row.created_at,
            row.updated_at,
            row.file_mtime,
            row.file_sha256,
            row.confidence,
        ],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, id: &str) -> Result<Option<ArtifactRow>> {
    cat.conn
        .prepare("SELECT id, abs_path, kind, status, title, owners, tags,
                  topic, time_scope, source, created_at, updated_at, file_mtime, file_sha256, confidence
                  FROM artifact WHERE id = ?1")?
        .query_row(params![id], row_from_sql)
        .optional()
        .map_err(Into::into)
}

pub fn delete(cat: &Catalog, id: &str) -> Result<bool> {
    Ok(cat
        .conn
        .execute("DELETE FROM artifact WHERE id = ?1", params![id])?
        > 0)
}

/// Delete all rows whose `abs_path` is not under any path in `active_roots`. Returns number removed.
pub fn delete_orphan_repos(cat: &Catalog, active_roots: &[&std::path::Path]) -> Result<usize> {
    if active_roots.is_empty() {
        let n = cat.conn.execute("DELETE FROM artifact", [])?;
        return Ok(n);
    }
    // Keep rows where abs_path starts with any active root (prefix match with trailing /).
    let keep_clauses: Vec<String> = (1..=active_roots.len())
        .map(|i| format!("abs_path LIKE ?{i}"))
        .collect();
    let sql = format!(
        "DELETE FROM artifact WHERE NOT ({})",
        keep_clauses.join(" OR ")
    );
    // Forward-slash normalize to match the form abs_paths are stored in
    // (artifact::upsert writes forward-slash via to_forward_slash). Without
    // this, on Windows the LIKE pattern uses backslash and matches NOTHING,
    // so the DELETE wipes every row.
    let params: Vec<String> = active_roots
        .iter()
        .map(|p| format!("{}/%", crate::util::fs::to_forward_slash(p)))
        .collect();
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let n = cat
        .conn
        .execute(&sql, rusqlite::params_from_iter(param_refs.iter().copied()))?;
    Ok(n)
}

pub(crate) fn row_from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
    let owners_s: String = r.get(5)?;
    let tags_s: String = r.get(6)?;
    let owners: Vec<String> = serde_json::from_str(&owners_s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tags: Vec<String> = serde_json::from_str(&tags_s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let abs_path_s: String = r.get(1)?;
    Ok(ArtifactRow {
        id: r.get(0)?,
        abs_path: std::path::PathBuf::from(abs_path_s),
        kind: r.get(2)?,
        status: r.get(3)?,
        title: r.get(4)?,
        owners,
        tags,
        topic: r.get(7)?,
        time_scope: r.get(8)?,
        source: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        file_mtime: r.get(12)?,
        file_sha256: r.get(13)?,
        confidence: r.get(14)?,
    })
}

/// Hydrate a frontmatter map from an `ArtifactRow`. Used as the seed for
/// `state_at::replay_state_at` (which then layers `field_patch` /
/// `status_change` events on top) and anywhere else that needs an
/// initial frontmatter view derived from catalog state.
///
/// Centralised here so the field list cannot drift between consumers.
pub fn build_frontmatter_map(art: &ArtifactRow) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::Value;
    let mut m = serde_json::Map::new();
    m.insert("status".into(), Value::String(art.status.clone()));
    if let Some(ref t) = art.title {
        m.insert("title".into(), Value::String(t.clone()));
    }
    m.insert("kind".into(), Value::String(art.kind.clone()));
    m.insert(
        "tags".into(),
        serde_json::to_value(&art.tags).unwrap_or(Value::Null),
    );
    m.insert(
        "owners".into(),
        serde_json::to_value(&art.owners).unwrap_or(Value::Null),
    );
    if let Some(ref t) = art.topic {
        m.insert("topic".into(), Value::String(t.clone()));
    }
    if let Some(ref t) = art.time_scope {
        m.insert("time_scope".into(), Value::String(t.clone()));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
            kind: "spec".into(),
            status: "active".into(),
            title: Some("T".into()),
            owners: vec!["marius".into()],
            tags: vec!["a".into(), "b".into()],
            topic: None,
            time_scope: None,
            source: Some("repo".into()),
            created_at: 1,
            updated_at: 2,
            file_mtime: 3,
            file_sha256: "abc".into(),
            confidence: 1.0,
        }
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        let row = sample("id1");
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched, row);
    }

    #[test]
    fn upsert_updates_on_conflict() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample("id1");
        upsert(&cat, &row).unwrap();
        row.status = "archived".into();
        row.updated_at = 99;
        upsert(&cat, &row).unwrap();
        let fetched = get(&cat, "id1").unwrap().unwrap();
        assert_eq!(fetched.status, "archived");
        assert_eq!(fetched.updated_at, 99);
    }

    #[test]
    fn delete_removes_row() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert(&cat, &sample("id1")).unwrap();
        assert!(delete(&cat, "id1").unwrap());
        assert!(get(&cat, "id1").unwrap().is_none());
    }

    #[test]
    fn delete_orphan_repos_drops_inactive() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut a = sample("a1");
        a.abs_path = std::path::PathBuf::from("/roots/alive/a.md");
        let mut b = sample("b1");
        b.abs_path = std::path::PathBuf::from("/roots/alive/b.md");
        let mut c = sample("c1");
        c.abs_path = std::path::PathBuf::from("/roots/ghost/c.md");
        for r in [&a, &b, &c] {
            upsert(&cat, r).unwrap();
        }
        let alive = std::path::Path::new("/roots/alive");
        let removed = delete_orphan_repos(&cat, &[alive]).unwrap();
        assert_eq!(removed, 1);
        assert!(get(&cat, "c1").unwrap().is_none());
        assert!(get(&cat, "a1").unwrap().is_some());
    }

    #[test]
    fn delete_orphan_repos_empty_active_wipes_all() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert(&cat, &sample("x")).unwrap();
        let n = delete_orphan_repos(&cat, &[]).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn get_surfaces_malformed_tags_json() {
        let cat = Catalog::open_in_memory().unwrap();
        // Insert a row bypassing upsert, with malformed tags JSON.
        cat.conn
            .execute(
                "INSERT INTO artifact (id, abs_path, kind, status, owners, tags,
                 created_at, updated_at, file_mtime, file_sha256, confidence)
                 VALUES ('bad', '/test/x.md', 'spec', 'active', '[]',
                         '{not valid json',
                         0, 0, 0, 'sha', 1.0)",
                [],
            )
            .unwrap();
        let err = get(&cat, "bad").unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("conversion")
                || err.to_string().contains("json")
        );
    }
}
