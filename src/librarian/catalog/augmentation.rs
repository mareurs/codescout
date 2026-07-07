use crate::librarian::catalog::Catalog;
use crate::librarian::tools::{schema_validate, RecoverableError};
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};

pub struct AugmentationRow {
    pub artifact_id: String,
    pub prompt: String,
    pub params: String, // raw JSON text
    pub last_refreshed_at: Option<String>,
    pub refresh_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Optional MiniJinja template projecting `params` into a markdown snippet
    /// rendered into `librarian_context` output. Decouples live state (params)
    /// from prose (artifact body).
    pub render_template: Option<String>,
    /// Optional JSON Schema (draft-07+) validating `params` on every merge.
    pub params_schema: Option<String>,
    /// When true, artifact_update prepends a new dated section instead of replacing the body.
    pub append_mode: bool,
    /// Max number of dated `## YYYY-MM-DD` sections to retain. Oldest are dropped beyond cap.
    pub history_cap: Option<i64>,
    /// Names the params array whose objects are the tracker's filterable
    /// entry rows (e.g. "failures", "children"). None = not entry-filterable.
    pub entry_collection: Option<String>,
    /// Server-computed provenance: repo HEAD at the last commit_refresh. None until a
    /// refresh runs with a resolvable HEAD. Surfaced by artifact(get) as
    /// provenance.refreshed_at_commit; NOT overwritten by re-augment.
    pub refreshed_at_commit: Option<String>,
}

pub fn upsert(cat: &Catalog, row: &AugmentationRow) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO artifact_augmentation
           (artifact_id, prompt, params, last_refreshed_at, refresh_count,
            created_at, updated_at, render_template, params_schema,
            append_mode, history_cap, entry_collection, refreshed_at_commit)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(artifact_id) DO UPDATE SET
           prompt = excluded.prompt,
           params = excluded.params,
           render_template = excluded.render_template,
           params_schema = excluded.params_schema,
           append_mode = excluded.append_mode,
           history_cap = excluded.history_cap,
           entry_collection = excluded.entry_collection,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            row.artifact_id,
            row.prompt,
            row.params,
            row.last_refreshed_at,
            row.refresh_count,
            row.created_at,
            row.updated_at,
            row.render_template,
            row.params_schema,
            row.append_mode as i64,
            row.history_cap,
            row.entry_collection,
            row.refreshed_at_commit,
        ],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, artifact_id: &str) -> Result<Option<AugmentationRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap, entry_collection, refreshed_at_commit
         FROM artifact_augmentation WHERE artifact_id = ?1",
    )?;
    let mut rows = stmt.query_map([artifact_id], row_from_sql)?;
    Ok(rows.next().transpose()?)
}

fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<AugmentationRow> {
    Ok(AugmentationRow {
        artifact_id: row.get(0)?,
        prompt: row.get(1)?,
        params: row.get(2)?,
        last_refreshed_at: row.get(3)?,
        refresh_count: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        render_template: row.get(7)?,
        params_schema: row.get(8)?,
        append_mode: row.get::<_, i64>(9).map(|v| v != 0)?,
        history_cap: row.get(10)?,
        entry_collection: row.get(11)?,
        refreshed_at_commit: row.get(12)?,
    })
}

/// Validates glob syntax for every rule's `paths` entries when the write
/// targets a `rules` entry_collection (the constitution-tracker convention
/// `find_matching_rules` reads, in `src/librarian/tools/constitution_check.rs`).
/// JSON Schema can't itself validate glob syntax, so this runs as a sibling
/// check next to `schema_validate` at every params write site — a malformed
/// glob must fail loud at authoring time, not silently disable the rule at
/// query time.
fn validate_rule_globs(entry_collection: &str, params: &Value) -> Result<()> {
    if entry_collection != "rules" {
        return Ok(());
    }
    let Some(rules) = params.get("rules").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    for rule in rules {
        let Some(paths) = rule.get("paths").and_then(|v| v.as_array()) else {
            continue;
        };
        for p in paths {
            let Some(s) = p.as_str() else { continue };
            if let Err(e) = globset::Glob::new(s) {
                let rule_id = rule.get("id").and_then(|v| v.as_str()).unwrap_or("<no id>");
                return Err(RecoverableError::new(format!(
                    "invalid glob `{s}` in rule `{rule_id}`'s paths: {e}"
                )));
            }
        }
    }
    Ok(())
}

/// Merge `patch` into the artifact's stored params and validate the result
/// against `params_schema` (if any) WITHOUT writing. Returns the serialized
/// merged params on success, or `None` when the artifact has no augmentation.
///
/// Split out of [`merge_params`] so a params patch can be validated *before*
/// other mutations (notably the body write in artifact `update`). A schema
/// violation must abort before anything is persisted — otherwise the body is
/// written but the params are not, leaving the artifact half-updated. See
/// docs/issues/2026-06-13-artifact-update-body-applies-before-params-validation.md.
fn merge_params_dry(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<Option<String>> {
    let Some(existing) = get(cat, artifact_id)? else {
        return Ok(None);
    };
    let mut current: Value = serde_json::from_str(&existing.params).unwrap_or_else(|_| json!({}));
    apply_merge_patch(&mut current, patch);
    if let Some(schema_text) = existing.params_schema.as_deref() {
        schema_validate::validate_against_stored(schema_text, &current).map_err(|e| {
            RecoverableError::new(format!("merge_params: patch violates params_schema: {e}"))
        })?;
    }
    validate_rule_globs(existing.entry_collection.as_deref().unwrap_or(""), &current)?;
    Ok(Some(serde_json::to_string(&current)?))
}

/// Validate a params patch against the stored schema without persisting it.
/// `Ok(())` guarantees a subsequent [`merge_params`] with the same patch will
/// not fail schema validation. Artifacts without an augmentation validate
/// trivially (nothing to check).
pub fn validate_params_patch(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<()> {
    merge_params_dry(cat, artifact_id, patch).map(|_| ())
}

pub fn merge_params(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<bool> {
    let Some(new_params) = merge_params_dry(cat, artifact_id, patch)? else {
        return Ok(false);
    };
    cat.conn.execute(
        "UPDATE artifact_augmentation SET params = ?1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?2",
        rusqlite::params![new_params, artifact_id],
    )?;
    Ok(true)
}

/// Atomically assigns the next `<id_prefix>-N` id and appends `entry` to
/// `params.<entry_collection>`. Runs inside a single `IMMEDIATE` transaction
/// so the read-max-write is safe under both intra-process and cross-process
/// concurrency (paired with `busy_timeout` set on the connection).
pub fn append_entry(
    cat: &mut Catalog,
    artifact_id: &str,
    entry_collection: &str,
    id_prefix: &str,
    mut entry: Value,
) -> Result<String> {
    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let row: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT params, params_schema, entry_collection
             FROM artifact_augmentation WHERE artifact_id = ?1",
            [artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let Some((params_text, params_schema, declared_collection)) = row else {
        return Err(RecoverableError::new(format!(
            "append_entry: artifact `{artifact_id}` has no augmentation — augment it with an entry_collection first"
        )));
    };

    if declared_collection.as_deref() != Some(entry_collection) {
        return Err(RecoverableError::with_hint(
            format!("append_entry: `{entry_collection}` is not this artifact's entry_collection"),
            match declared_collection {
                Some(c) => format!("This artifact's entry_collection is `{c}` — pass that instead."),
                None => "This artifact has no entry_collection declared — set one via artifact_augment first.".to_string(),
            },
        ));
    }

    let mut params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));
    let existing_ids: Vec<String> = params
        .get(entry_collection)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    let new_id = format!("{id_prefix}-{}", next_index(&existing_ids, id_prefix));

    if let Some(obj) = entry.as_object_mut() {
        obj.insert("id".to_string(), json!(new_id));
    }

    match params.get_mut(entry_collection) {
        Some(Value::Array(arr)) => arr.push(entry),
        _ => {
            params[entry_collection] = json!([entry]);
        }
    }

    if let Some(schema_text) = params_schema.as_deref() {
        schema_validate::validate_against_stored(schema_text, &params).map_err(|e| {
            RecoverableError::new(format!(
                "append_entry: new entry violates params_schema: {e}"
            ))
        })?;
    }
    validate_rule_globs(entry_collection, &params)?;

    let new_params_text = serde_json::to_string(&params)?;
    tx.execute(
        "UPDATE artifact_augmentation SET params = ?1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?2",
        rusqlite::params![new_params_text, artifact_id],
    )?;
    tx.commit()?;
    Ok(new_id)
}

fn next_index(existing_ids: &[String], id_prefix: &str) -> u64 {
    let re = regex::Regex::new(&format!(r"^{}-(\d+)$", regex::escape(id_prefix))).unwrap();
    existing_ids
        .iter()
        .filter_map(|id| re.captures(id))
        .filter_map(|c| c[1].parse::<u64>().ok())
        .max()
        .map(|m| m + 1)
        .unwrap_or(1)
}

/// Shallow RFC 7396 merge-patch applied in place to `target`. `null` keys in the
/// patch delete; non-null values overwrite the corresponding target key entirely.
/// Nested objects are overwritten in full (not recursively merged). This is intentional —
/// artifact params are expected to be flat key-value objects. Non-object patches are
/// no-ops (the tool schema enforces object at the boundary).
pub fn apply_merge_patch(target: &mut Value, patch: &Value) {
    if let (Value::Object(t), Value::Object(p)) = (target, patch) {
        for (k, v) in p {
            if v.is_null() {
                t.remove(k);
            } else {
                t.insert(k.clone(), v.clone());
            }
        }
    }
}

pub fn commit_refresh(cat: &Catalog, artifact_id: &str, head_commit: Option<&str>) -> Result<bool> {
    let n = cat.conn.execute(
        "UPDATE artifact_augmentation
         SET refresh_count = refresh_count + 1,
             last_refreshed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             refreshed_at_commit = COALESCE(?2, refreshed_at_commit),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?1",
        rusqlite::params![artifact_id, head_commit],
    )?;
    Ok(n > 0)
}

pub fn list_all_ids(cat: &Catalog) -> Result<Vec<String>> {
    let mut stmt = cat
        .conn
        .prepare("SELECT artifact_id FROM artifact_augmentation ORDER BY artifact_id")?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn get_batch(
    cat: &Catalog,
    ids: &[String],
) -> Result<std::collections::HashMap<String, AugmentationRow>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap, entry_collection, refreshed_at_commit
         FROM artifact_augmentation WHERE artifact_id IN ({placeholders})"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(params.as_slice(), row_from_sql)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|r| (r.artifact_id.clone(), r))
        .collect())
}

#[derive(Debug, Clone)]
pub struct StaleEntry {
    pub artifact_id: String,
    pub abs_path: std::path::PathBuf,
    pub kind: String,
    pub title: Option<String>,
    pub last_refreshed_at: Option<String>,
    pub refresh_count: i64,
}

/// Return augmented artifacts whose `last_refreshed_at` is older than
/// `threshold_iso` (ISO-8601), or has never been refreshed (NULL).
/// Results are ordered oldest-first (NULLs first — SQLite sorts NULLs as
/// less than any value in ASC order).
pub fn list_stale(
    cat: &Catalog,
    threshold_iso: &str,
    limit: usize,
    abs_path_prefix: Option<&std::path::Path>,
) -> Result<Vec<StaleEntry>> {
    let mut sql = String::from(
        "SELECT a.id, a.abs_path, a.kind, a.title, \
         au.last_refreshed_at, au.refresh_count \
         FROM artifact_augmentation au \
         JOIN artifact a ON a.id = au.artifact_id \
         WHERE (au.last_refreshed_at IS NULL OR au.last_refreshed_at < ?1)",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(threshold_iso.to_string())];
    let mut idx = 2usize;

    if let Some(prefix) = abs_path_prefix {
        let prefix_s = crate::util::fs::RepoPath::from(prefix);
        if !prefix_s.as_str().is_empty() {
            sql.push_str(&format!(" AND a.abs_path LIKE ?{idx}"));
            params.push(Box::new(format!("{prefix_s}/%")));
            idx += 1;
        }
    }

    sql.push_str(&format!(" ORDER BY au.last_refreshed_at ASC LIMIT ?{idx}"));
    params.push(Box::new(limit as i64));

    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            let abs_path_s: String = row.get(1)?;
            Ok(StaleEntry {
                artifact_id: row.get(0)?,
                abs_path: std::path::PathBuf::from(abs_path_s),
                kind: row.get(2)?,
                title: row.get(3)?,
                last_refreshed_at: row.get(4)?,
                refresh_count: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use chrono::Utc;

    fn sample_art(id: &str) -> ArtifactRow {
        let now = Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: Some("T".to_string()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "abc".to_string(),
            confidence: 1.0,
        }
    }

    fn aug(artifact_id: &str) -> AugmentationRow {
        AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: "test prompt".to_string(),
            params: "{}".to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: None,
            refreshed_at_commit: None,
        }
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let row = get(&cat, "art1").unwrap().expect("row should exist");
        assert_eq!(row.artifact_id, "art1");
        assert_eq!(row.prompt, "test prompt");
        assert_eq!(row.refresh_count, 0);
    }

    #[test]
    fn upsert_replaces_on_conflict() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let mut updated = aug("art1");
        updated.prompt = "New prompt".to_string();
        upsert(&cat, &updated).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "New prompt");
        assert_eq!(row.refresh_count, 0);
    }

    #[test]
    fn upsert_preserves_refresh_count_on_update() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        // Simulate a refresh having happened
        commit_refresh(&cat, "art1", None).unwrap();
        // Re-augment with new prompt
        let mut updated = aug("art1");
        updated.prompt = "Updated prompt".to_string();
        upsert(&cat, &updated).unwrap();
        // refresh_count must NOT be reset
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(
            row.refresh_count, 1,
            "refresh_count must survive re-augment"
        );
        assert!(
            row.last_refreshed_at.is_some(),
            "last_refreshed_at must survive re-augment"
        );
        assert_eq!(row.prompt, "Updated prompt");
    }

    #[test]
    fn merge_params_adds_key() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let patch = json!({"format": "table"});
        let found = merge_params(&cat, "art1", &patch).unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["format"], "table");
    }

    #[test]
    fn merge_params_null_deletes_key() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.params = r#"{"format":"table"}"#.to_string();
        upsert(&cat, &a).unwrap();
        let patch = json!({"format": null});
        merge_params(&cat, "art1", &patch).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert!(params.get("format").is_none());
    }

    #[test]
    fn merge_params_missing_artifact_returns_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let found = merge_params(&cat, "nope", &json!({"x": 1})).unwrap();
        assert!(!found);
    }
    #[test]
    fn merge_params_rejects_violation() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "additionalProperties": false
        });
        let mut a = aug("art1");
        a.params_schema = Some(serde_json::to_string(&schema).unwrap());
        upsert(&cat, &a).unwrap();
        let patch = json!({"count": "not-a-number"});
        let err = merge_params(&cat, "art1", &patch).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("merge_params: patch violates params_schema"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn merge_params_accepts_valid() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "additionalProperties": false
        });
        let mut a = aug("art1");
        a.params_schema = Some(serde_json::to_string(&schema).unwrap());
        upsert(&cat, &a).unwrap();
        let patch = json!({"count": 42});
        let found = merge_params(&cat, "art1", &patch).unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["count"], 42);
    }

    #[test]
    fn commit_refresh_increments_count() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let found = commit_refresh(&cat, "art1", None).unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.refresh_count, 1);
        assert!(row.last_refreshed_at.is_some());
    }

    #[test]
    fn commit_refresh_records_head_commit() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        assert!(commit_refresh(&cat, "art1", Some("deadbeef")).unwrap());
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.refreshed_at_commit.as_deref(), Some("deadbeef"));
        assert_eq!(row.refresh_count, 1);
    }

    #[test]
    fn refreshed_at_commit_preserved_on_reaugment() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        commit_refresh(&cat, "art1", Some("deadbeef")).unwrap();
        // Re-augment (upsert on conflict) must NOT wipe the recorded refresh commit.
        let mut re = aug("art1");
        re.prompt = "new prompt".into();
        upsert(&cat, &re).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "new prompt");
        assert_eq!(
            row.refreshed_at_commit.as_deref(),
            Some("deadbeef"),
            "re-augment must not wipe refreshed_at_commit"
        );
    }

    #[test]
    fn commit_refresh_missing_returns_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let found = commit_refresh(&cat, "nope", None).unwrap();
        assert!(!found);
    }

    #[test]
    fn cascade_delete_removes_augmentation() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        crate::librarian::catalog::artifact::delete(&cat, "art1").unwrap();
        assert!(get(&cat, "art1").unwrap().is_none());
    }

    #[test]
    fn list_all_ids_returns_augmented() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        art_upsert(&cat, &sample_art("art2")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let ids = list_all_ids(&cat).unwrap();
        assert_eq!(ids, vec!["art1"]);
    }

    #[test]
    fn get_batch_returns_map() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        art_upsert(&cat, &sample_art("art2")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let map = get_batch(&cat, &["art1".to_string(), "art2".to_string()]).unwrap();
        assert!(map.contains_key("art1"));
        assert!(!map.contains_key("art2"));
    }

    #[test]
    fn append_mode_and_history_cap_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        let art = sample_art("a1");
        crate::librarian::catalog::artifact::upsert(&cat, &art).unwrap();
        let mut row = aug("a1");
        row.append_mode = true;
        row.history_cap = Some(5);
        upsert(&cat, &row).unwrap();
        let got = get(&cat, "a1").unwrap().unwrap();
        assert!(got.append_mode);
        assert_eq!(got.history_cap, Some(5));
    }

    #[test]
    fn append_mode_defaults_to_false() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        let art = sample_art("a2");
        crate::librarian::catalog::artifact::upsert(&cat, &art).unwrap();
        upsert(&cat, &aug("a2")).unwrap();
        let got = get(&cat, "a2").unwrap().unwrap();
        assert!(!got.append_mode);
        assert_eq!(got.history_cap, None);
    }
    #[test]
    fn entry_collection_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        crate::librarian::catalog::artifact::upsert(&cat, &sample_art("ec-art")).unwrap();
        upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ec-art".into(),
                prompt: "p".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-05-28T00:00:00.000Z".into(),
                updated_at: "2026-05-28T00:00:00.000Z".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("failures".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let got = get(&cat, "ec-art").unwrap().unwrap();
        assert_eq!(got.entry_collection.as_deref(), Some("failures"));
    }

    #[test]
    fn append_entry_assigns_first_id_to_empty_collection() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id =
            append_entry(&mut cat, "art1", "failures", "F", json!({"status": "fail"})).unwrap();

        assert_eq!(id, "F-1");
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["failures"][0]["id"], "F-1");
        assert_eq!(params["failures"][0]["status"], "fail");
    }

    #[test]
    fn append_entry_computes_max_plus_one_across_non_contiguous_ids() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-1"},{"id":"F-3"},{"id":"F-9"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({})).unwrap();
        assert_eq!(id, "F-10");
    }

    #[test]
    fn append_entry_rejects_unknown_entry_collection() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let err = append_entry(&mut cat, "art1", "bugs", "B", json!({})).unwrap_err();
        assert!(err.to_string().contains("failures"));
    }

    #[test]
    fn append_entry_rejects_missing_augmentation() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();

        let err = append_entry(&mut cat, "art1", "failures", "F", json!({})).unwrap_err();
        assert!(err.to_string().contains("no augmentation"));
    }

    #[test]
    fn append_entry_rejects_schema_violation_without_writing() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-1","status":"fail"}]}"#.to_string();
        a.params_schema = Some(
            json!({
                "type": "object",
                "properties": {
                    "failures": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["id", "status"],
                            "properties": {
                                "status": {"type": "string", "enum": ["fail", "pass"]}
                            }
                        }
                    }
                }
            })
            .to_string(),
        );
        upsert(&cat, &a).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({"status": "bogus"}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("params_schema"));

        // Rolled back: still exactly the one original entry.
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["failures"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn append_entry_rejects_malformed_glob_without_writing() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("rules".to_string());
        a.params = r#"{"rules":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "rules",
            "C",
            json!({"paths": ["[invalid"], "rule": "R", "status": "active"}),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("[invalid"),
            "error should name the offending glob; got: {err}"
        );

        // Rolled back: still no rules persisted.
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["rules"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn append_entry_accepts_valid_glob() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("rules".to_string());
        a.params = r#"{"rules":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id = append_entry(
            &mut cat,
            "art1",
            "rules",
            "C",
            json!({"paths": ["src/**/*.rs"], "rule": "R", "status": "active"}),
        )
        .unwrap();
        assert_eq!(id, "C-1");
    }

    #[test]
    fn append_entry_ignores_glob_check_for_other_collections() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        // "paths" isn't a rules-glob field here — an unrelated collection
        // must never be glob-validated, valid-looking or not.
        let id = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({"paths": ["[invalid"], "status": "fail"}),
        )
        .unwrap();
        assert_eq!(id, "F-1");
    }

    #[test]
    fn merge_params_rejects_malformed_glob_without_writing() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("rules".to_string());
        a.params = r#"{"rules":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let patch = json!({"rules": [{"id": "C-1", "paths": ["[invalid"], "status": "active"}]});
        let err = merge_params(&cat, "art1", &patch).unwrap_err();
        assert!(
            err.to_string().contains("[invalid"),
            "error should name the offending glob; got: {err}"
        );

        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["rules"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn append_entry_serializes_across_independent_connections_to_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cat.sqlite");

        {
            let cat = Catalog::open(&path).unwrap();
            art_upsert(&cat, &sample_art("art1")).unwrap();
            let mut a = aug("art1");
            a.entry_collection = Some("failures".to_string());
            a.params = r#"{"failures":[]}"#.to_string();
            upsert(&cat, &a).unwrap();
        }

        let path1 = path.clone();
        let path2 = path.clone();
        let h1 = std::thread::spawn(move || {
            let mut cat = Catalog::open(&path1).unwrap();
            append_entry(&mut cat, "art1", "failures", "F", json!({"who": "one"})).unwrap()
        });
        let h2 = std::thread::spawn(move || {
            let mut cat = Catalog::open(&path2).unwrap();
            append_entry(&mut cat, "art1", "failures", "F", json!({"who": "two"})).unwrap()
        });

        let id1 = h1.join().unwrap();
        let id2 = h2.join().unwrap();

        assert_ne!(
            id1, id2,
            "concurrent appends from independent connections must not collide"
        );
        let mut ids = vec![id1, id2];
        ids.sort();
        assert_eq!(ids, vec!["F-1".to_string(), "F-2".to_string()]);

        let cat = Catalog::open(&path).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["failures"].as_array().unwrap().len(), 2);
    }
}
