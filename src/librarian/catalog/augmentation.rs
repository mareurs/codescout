use crate::librarian::catalog::Catalog;
use crate::librarian::tools::{schema_validate, RecoverableError};
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
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
/// docs/issues/archive/2026-06-13-artifact-update-body-applies-before-params-validation.md.
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

pub fn merge_params(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<ParamsMergeOutcome> {
    // Sample the entry collection BEFORE the write. RFC 7396 replaces arrays
    // wholesale, and the catalog is not in git, so this count is the only signal
    // a wipe will ever produce.
    let entries_before = entry_count(cat, artifact_id)?;
    let Some(new_params) = merge_params_dry(cat, artifact_id, patch)? else {
        return Ok(ParamsMergeOutcome::default());
    };
    cat.conn.execute(
        "UPDATE artifact_augmentation SET params = ?1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?2",
        rusqlite::params![new_params, artifact_id],
    )?;
    let entries_after = entry_count(cat, artifact_id)?;
    Ok(ParamsMergeOutcome {
        found: true,
        entries_before,
        entries_after,
    })
}

/// Rows in the artifact's declared `entry_collection`.
///
/// `None` means the artifact declares no entry collection — there is nothing to
/// count, and reporting `0` would read as "the collection was emptied". A
/// declared-but-absent key counts as `0`: the collection exists conceptually and
/// holds no rows.
fn entry_count(cat: &Catalog, artifact_id: &str) -> Result<Option<usize>> {
    let Some(existing) = get(cat, artifact_id)? else {
        return Ok(None);
    };
    let Some(collection) = existing.entry_collection.as_deref() else {
        return Ok(None);
    };
    let params: Value = serde_json::from_str(&existing.params).unwrap_or_else(|_| json!({}));
    Ok(Some(
        params
            .get(collection)
            .and_then(|v| v.as_array())
            .map_or(0, |a| a.len()),
    ))
}

/// What a params merge did to the artifact's declared entry collection.
///
/// `merge_params` applies RFC 7396 semantics, which replace an array **wholesale**.
/// That is still allowed — a bulk rewrite is a legitimate operation — but it must
/// not be silent: a caller sending one row to flip one row's status deletes every
/// other row, and the catalog is not in git, so nothing else can notice.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParamsMergeOutcome {
    /// False when the artifact has no augmentation to merge into.
    pub found: bool,
    /// Length of the declared `entry_collection` array before/after the merge.
    /// `None` when the artifact declares no entry collection — there is no array
    /// to count, and inventing one would be worse than saying nothing.
    pub entries_before: Option<usize>,
    pub entries_after: Option<usize>,
}

/// What an entry-grain update changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEntryOutcome {
    pub entry_id: String,
    /// Keys the patch actually touched, in patch order.
    pub changed_fields: Vec<String>,
    /// Row count after the write. An entry update never changes it — reported so
    /// a caller can assert that cheaply.
    pub entries_total: usize,
}

/// Patch the fields of ONE entry in `params.<entry_collection>`, leaving every
/// other row untouched.
///
/// The counterpart `append_entry` never had. Without it, flipping one row's
/// status — the most common maintenance action on a task tracker — had no choice
/// but to go through `merge_params`, whose RFC 7396 array semantics replace the
/// whole collection. `append_entry` exists precisely so nobody hand-rolls that
/// read-then-write; this closes the other half.
///
/// `fields` is merged shallowly onto the matched row: a `null` value deletes the
/// key, matching the params merge-patch semantics callers already know. `id` is
/// rejected — entry ids key `entry_cite` rows (`<slug>:<local>`), so re-keying a
/// row through a field patch would strand every citation of it.
///
/// Runs in a single `IMMEDIATE` transaction, like `append_entry`.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
pub fn update_entry(
    cat: &mut Catalog,
    artifact_id: &str,
    entry_collection: &str,
    entry_id: &str,
    fields: Value,
) -> Result<UpdateEntryOutcome> {
    let Some(patch) = fields.as_object() else {
        return Err(RecoverableError::new(
            "update_entry: `fields` must be a JSON object",
        ));
    };
    // An empty patch passes every guard below and completes having touched
    // nothing, reporting success with `changed_fields: []` — which reads as "this
    // changed nothing" rather than "your patch never arrived". That is how a
    // typo'd param name became a silent no-op: an undeclared key is dropped before
    // it reaches here, so `fields` arrives as `{}`. This action exists because the
    // path it replaced was silent; it must not be silent in a narrower way.
    // docs/issues/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
    if patch.is_empty() {
        return Err(RecoverableError::with_hint(
            "update_entry: `fields` is empty — there is nothing to patch".to_string(),
            "Pass at least one field, e.g. fields={\"status\": \"done\"}; a null value deletes a \
             key. If you passed the patch under a different parameter name, note that `entry` \
             belongs to append_entry — this action takes `fields`."
                .to_string(),
        ));
    }
    if patch.contains_key("id") {
        return Err(RecoverableError::with_hint(
            "update_entry: `id` cannot be changed through a field patch".to_string(),
            "Entry ids key entry_cite rows (`<slug>:<local>`), so re-keying one would strand \
             every citation of it with nothing to repair them. Append a new entry and mark this \
             one superseded instead."
                .to_string(),
        ));
    }

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
            "update_entry: artifact `{artifact_id}` has no augmentation — augment it with an entry_collection first"
        )));
    };

    if declared_collection.as_deref() != Some(entry_collection) {
        return Err(RecoverableError::with_hint(
            format!("update_entry: `{entry_collection}` is not this artifact's entry_collection"),
            match declared_collection {
                Some(c) => format!("This artifact's entry_collection is `{c}` — pass that instead."),
                None => "This artifact has no entry_collection declared — set one via artifact_augment first.".to_string(),
            },
        ));
    }

    let mut params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));

    // Locate the row before mutating, so the immutable borrow ends before the
    // schema re-validation needs `params` whole again.
    let Some(arr) = params.get(entry_collection).and_then(|v| v.as_array()) else {
        return Err(RecoverableError::new(format!(
            "update_entry: `{entry_collection}` holds no entry array on artifact `{artifact_id}`"
        )));
    };
    let entries_total = arr.len();
    let position = arr
        .iter()
        .position(|e| e.get("id").and_then(|v| v.as_str()) == Some(entry_id));

    let Some(position) = position else {
        // Name what IS there. A bare "not found" makes the caller re-read the
        // whole collection just to discover it typed the id — which is the
        // read-then-write this path exists to remove.
        const KNOWN_IDS_SHOWN: usize = 12;
        let known: Vec<&str> = arr
            .iter()
            .filter_map(|e| e.get("id").and_then(|v| v.as_str()))
            .take(KNOWN_IDS_SHOWN)
            .collect();
        let elided = entries_total.saturating_sub(known.len());
        let suffix = if elided > 0 {
            format!(" (+{elided} more)")
        } else {
            String::new()
        };
        return Err(RecoverableError::with_hint(
            format!("update_entry: no entry `{entry_id}` in `{entry_collection}`"),
            format!("Known ids: {}{}", known.join(", "), suffix),
        ));
    };

    let Some(obj) = params[entry_collection][position].as_object_mut() else {
        return Err(RecoverableError::new(format!(
            "update_entry: entry `{entry_id}` is not a JSON object"
        )));
    };

    let mut changed_fields = Vec::with_capacity(patch.len());
    for (k, v) in patch {
        changed_fields.push(k.clone());
        if v.is_null() {
            obj.remove(k);
        } else {
            obj.insert(k.clone(), v.clone());
        }
    }

    if let Some(schema_text) = params_schema.as_deref() {
        schema_validate::validate_against_stored(schema_text, &params).map_err(|e| {
            RecoverableError::new(format!(
                "update_entry: patched entry violates params_schema: {e}"
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

    Ok(UpdateEntryOutcome {
        entry_id: entry_id.to_string(),
        changed_fields,
        entries_total,
    })
}

/// Result of a successful [`append_entry`].
#[derive(Debug)]
pub struct AppendOutcome {
    /// The id assigned to the new entry.
    pub id: String,
    /// Set when the body claimed ids the params array does not carry — the
    /// append itself succeeded, but the structured index is incomplete.
    pub warning: Option<String>,
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
    cites: &[String],
) -> Result<AppendOutcome> {
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
    // The params array is NOT the only surface that claims ids. For the common
    // tracker shape the markdown body (index table + `## PREFIX-N` sections) is
    // the canonical human-readable surface, and the documented 3-step flow
    // (body section -> index row -> append_entry) lets params legitimately lag
    // the body when a session skips step 3. Folding the body's max in makes the
    // reissue impossible instead of silent.
    // See docs/issues/archive/2026-07-20-append-entry-id-drift-params-vs-body.md
    let abs_path: Option<String> = tx
        .query_row(
            "SELECT abs_path FROM artifact WHERE id = ?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()?;
    let body_max = abs_path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|body| body_max_index(&body, id_prefix));

    let params_next = next_index(&existing_ids, id_prefix);
    let next = params_next.max(body_max.map_or(0, |m| m + 1));
    let new_id = format!("{id_prefix}-{next}");

    // Skipping the collision is only half the repair — params is still missing
    // the rows the body already documents, and nothing else would ever say so.
    let warning = body_max.filter(|m| m + 1 > params_next).map(|m| {
        let params_max = match params_next {
            1 => "none".to_string(),
            n => format!("{id_prefix}-{}", n - 1),
        };
        format!(
            "params lags body: the body already claims {id_prefix}-{m} but params' highest is \
             {params_max}. Assigned {new_id} to avoid a collision — backfill the missing params \
             rows from the body so the structured index matches."
        )
    });

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

    if !cites.is_empty() {
        let slug = crate::librarian::catalog::artifact::ensure_slug(&tx, artifact_id)?;
        let now = chrono::Utc::now().timestamp_millis();
        for raw in cites {
            let dst_ref = resolve_cite_ref(&tx, raw)?;
            crate::librarian::catalog::entry_cite::insert_with(
                &tx,
                &crate::librarian::catalog::entry_cite::EntryCiteRow {
                    src_slug: slug.clone(),
                    src_local: new_id.clone(),
                    dst_ref,
                    rel: "cites".to_string(),
                    origin: "write".to_string(),
                    created_at: now,
                },
            )?;
        }
    }
    tx.commit()?;
    Ok(AppendOutcome {
        id: new_id,
        warning,
    })
}

/// Resolve a user-supplied cite ref to a stable `entry_cite.dst_ref`.
/// Accepts: a 16-hex artifact id that exists; a `<slug>:<local>` whose slug is a
/// known artifact and whose local exists in that artifact's entry_collection; or a
/// rel_path (suffix of exactly one artifact's abs_path). Rejects anything else.
fn resolve_cite_ref(conn: &rusqlite::Connection, raw: &str) -> Result<String> {
    // 1. artifact id (16 lowercase hex chars).
    let is_hex16 = raw.len() == 16 && raw.bytes().all(|b| b.is_ascii_hexdigit());
    if is_hex16 {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM artifact WHERE id=?1",
                rusqlite::params![raw],
                |_| Ok(true),
            )
            .optional()?
            .is_some();
        if exists {
            return Ok(raw.to_string());
        }
    }
    // 2. <slug>:<local>
    if let Some((slug, local)) = raw.split_once(':') {
        let coll: Option<Option<String>> = conn
            .query_row(
                "SELECT au.entry_collection
                   FROM artifact a JOIN artifact_augmentation au ON au.artifact_id = a.id
                  WHERE a.slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(Some(collection)) = coll {
            let params_text: String = conn.query_row(
                "SELECT au.params FROM artifact a
                   JOIN artifact_augmentation au ON au.artifact_id = a.id
                  WHERE a.slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )?;
            let params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));
            let local_exists = params
                .get(&collection)
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .any(|e| e.get("id").and_then(|v| v.as_str()) == Some(local));
            if local_exists {
                return Ok(raw.to_string());
            }
        }
        return Err(RecoverableError::new(format!(
            "append_entry: cite `{raw}` — no such entry (slug or local id not found)"
        )));
    }
    // 3. rel_path suffix match — must resolve to exactly one artifact.
    let escaped = crate::librarian::util::escape_like_pattern(raw);
    let like = format!("%/{escaped}");
    let mut stmt = conn
        .prepare("SELECT id FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2 ESCAPE '\\'")?;
    let ids: Vec<String> = stmt
        .query_map(rusqlite::params![raw, like], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match ids.len() {
        1 => Ok(ids.into_iter().next().unwrap()),
        0 => Err(RecoverableError::with_hint(
            format!("append_entry: cite `{raw}` did not resolve"),
            "Use a 16-hex artifact id, a `<slug>:<local>` entry id, or a unique rel_path."
                .to_string(),
        )),
        _ => Err(RecoverableError::new(format!(
            "append_entry: cite `{raw}` is ambiguous ({} artifacts match)",
            ids.len()
        ))),
    }
}

/// Highest `<id_prefix>-N` index already claimed by an artifact's markdown body.
///
/// Only line-anchored occurrences count: a markdown heading (`## F-12`) or the
/// leading cell of an index-table row (`| F-12 | ... |`), optionally wrapped in
/// backticks/bold/link brackets. Those are exactly the two surfaces the
/// documented 3-step tracker flow writes. Prose mentions are deliberately
/// ignored — an aside like "planned F-999" must not blow a hole in the
/// numbering, and over-allocating is only safe when the trigger is precise.
pub(crate) fn body_max_index(body: &str, id_prefix: &str) -> Option<u64> {
    let esc = regex::escape(id_prefix);
    let re = regex::Regex::new(&format!(
        r"(?m)^(?:#{{1,6}}[ \t]+|\|[ \t]*)[`*\[]*{esc}-(\d+)\b"
    ))
    .ok()?;
    re.captures_iter(body)
        .filter_map(|c| c[1].parse::<u64>().ok())
        .max()
}

pub(crate) fn next_index(existing_ids: &[String], id_prefix: &str) -> u64 {
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
/// Nested objects are overwritten in full (not recursively merged).
///
/// **Arrays are replaced wholesale, and params are no longer flat.** This comment
/// used to justify the shallow merge with "artifact params are expected to be flat
/// key-value objects". That was true when it was written and is not any more:
/// `entry_collection` makes params the home for arrays of entry rows, and two of
/// the archetypes `tracker_design` recommends are built on exactly that shape. So
/// a patch carrying one row of a collection deletes every other row — legitimate
/// for a bulk rewrite, catastrophic for the one-row edit it looks like.
///
/// Two things now stand between a caller and that outcome, because the semantics
/// here deliberately did not change: [`update_entry`] gives a one-row edit its own
/// path, and [`merge_params`] reports `entries_before`/`entries_after` so a
/// wholesale replace is visible rather than silent.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
///
/// Non-object patches are silent no-ops. Callers MUST reject them at their own
/// input boundary rather than relying on the tool schema: the schema's
/// `"type": "object"` covers only the inline `params` argument, and
/// `params_path` reads a file that never passes through it. That gap let a bare
/// top-level array report success while discarding the whole payload
/// (docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md).
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
    use crate::librarian::catalog::artifact::{
        upsert as art_upsert, ArtifactRow, TestArtifactRowBuilder,
    };
    use chrono::Utc;

    fn sample_art(id: &str) -> ArtifactRow {
        let now = Utc::now().timestamp_millis();
        TestArtifactRowBuilder::new(id)
            .with_kind("tracker")
            .with_title("T")
            .with_created_at(now)
            .with_updated_at(now)
            .with_file_mtime(now)
            .with_file_sha256("abc")
            .build()
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
        let found = merge_params(&cat, "art1", &patch).unwrap().found;
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

    /// Seeds an artifact whose params hold a three-row `tasks` entry collection.
    fn seed_task_list(cat: &Catalog, id: &str) {
        art_upsert(cat, &sample_art(id)).unwrap();
        let mut a = aug(id);
        a.entry_collection = Some("tasks".into());
        a.params = r#"{"tasks":[
                {"id":"T-1","status":"open","note":"first"},
                {"id":"T-2","status":"open"},
                {"id":"T-3","status":"open"}
            ]}"#
        .to_string();
        upsert(cat, &a).unwrap();
    }

    /// RFC 7396 replaces an array wholesale, so sending one row to flip one
    /// row's status deletes the rest. That stays *allowed* — a bulk rewrite is
    /// legitimate — but it must never again be **silent**. This is the call
    /// that took a live tracker from 19 entries to 1.
    /// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
    #[test]
    fn merge_params_reports_entry_counts_across_a_wholesale_replace() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let out = merge_params(&cat, "art1", &json!({"tasks":[{"id":"T-1"}]})).unwrap();

        assert!(out.found);
        assert_eq!(out.entries_before, Some(3));
        assert_eq!(
            out.entries_after,
            Some(1),
            "the caller must be told the collection shrank, even though the write is permitted"
        );
    }

    /// An artifact with no declared entry_collection has no array to count, and
    /// must not invent one.
    #[test]
    fn merge_params_reports_no_entry_counts_without_an_entry_collection() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();

        let out = merge_params(&cat, "art1", &json!({"format": "table"})).unwrap();

        assert!(out.found);
        assert_eq!(out.entries_before, None);
        assert_eq!(out.entries_after, None);
    }

    /// The real fix: flipping one row's status is the most common maintenance
    /// action on a task tracker, and until now it had to go through the
    /// wholesale replace because `append_entry` only appends.
    #[test]
    fn update_entry_patches_one_row_and_leaves_the_others() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let out = update_entry(&mut cat, "art1", "tasks", "T-2", json!({"status":"done"})).unwrap();

        assert_eq!(out.entry_id, "T-2");
        assert_eq!(
            out.entries_total, 3,
            "an entry update must never change the row count"
        );
        assert_eq!(out.changed_fields, vec!["status".to_string()]);

        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        let tasks = params["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0]["status"], "open");
        assert_eq!(tasks[1]["status"], "done");
        assert_eq!(tasks[2]["status"], "open");
        assert_eq!(
            tasks[0]["note"], "first",
            "fields the patch did not name must survive"
        );
    }

    /// Null deletes a field, matching the params merge-patch semantics the
    /// caller already knows from `patch={params:…}`.
    #[test]
    fn update_entry_null_deletes_a_field() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        update_entry(&mut cat, "art1", "tasks", "T-1", json!({"note": null})).unwrap();

        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert!(params["tasks"][0].get("note").is_none());
        assert_eq!(params["tasks"][0]["status"], "open");
    }

    /// A typo'd id must not silently write nothing — the whole point of this
    /// path is that the caller stops hand-building the array, so a no-op that
    /// reports success would be worse than the bug it replaces.
    #[test]
    fn update_entry_rejects_an_unknown_entry_id() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let err =
            update_entry(&mut cat, "art1", "tasks", "T-9", json!({"status":"done"})).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("T-9"),
            "the error must name the missing id: {msg}"
        );

        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["tasks"].as_array().unwrap().len(), 3);
    }

    /// An empty patch passes every other guard and completes having touched
    /// nothing, reporting success. That is how a typo'd param name became a
    /// silent no-op: the MCP layer drops an undeclared key, `fields` defaults
    /// to `{}`, and `changed_fields: []` reads as "this changed nothing"
    /// rather than "your patch never arrived".
    ///
    /// This action exists because the path it replaced was silent. It must not
    /// be silent in a narrower way.
    /// docs/issues/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
    #[test]
    fn update_entry_rejects_an_empty_patch() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let err = update_entry(&mut cat, "art1", "tasks", "T-1", json!({})).unwrap_err();
        assert!(
            err.to_string().contains("empty"),
            "the error must say the patch was empty, not report success: {err}"
        );
    }

    /// Entry ids are a citation surface — `entry_cite` rows key on
    /// `<slug>:<local>` — so re-keying a row through a field patch would
    /// strand every citation of it with no cascade to repair them.
    #[test]
    fn update_entry_refuses_to_rewrite_the_entry_id() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let err = update_entry(&mut cat, "art1", "tasks", "T-1", json!({"id":"T-99"})).unwrap_err();
        assert!(
            err.to_string().contains("id"),
            "the error must say the id field is off limits: {err}"
        );
    }

    /// Same guard `append_entry` carries: the collection named must be the one
    /// the artifact declared, or the caller is writing to a key nothing reads.
    #[test]
    fn update_entry_rejects_a_collection_the_artifact_did_not_declare() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");

        let err = update_entry(
            &mut cat,
            "art1",
            "findings",
            "T-1",
            json!({"status":"done"}),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("findings"),
            "the error must name the collection that was passed: {err}"
        );
    }

    #[test]
    fn merge_params_missing_artifact_returns_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let found = merge_params(&cat, "nope", &json!({"x": 1})).unwrap().found;
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
        let found = merge_params(&cat, "art1", &patch).unwrap().found;
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

        let id = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({"status": "fail"}),
            &[],
        )
        .unwrap()
        .id;

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

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[])
            .unwrap()
            .id;
        assert_eq!(id, "F-10");
    }

    #[test]
    fn append_entry_skips_ids_already_claimed_by_the_body() {
        // Regression: docs/issues/archive/2026-07-20-append-entry-id-drift-params-vs-body.md
        // The documented 3-step tracker flow (body section -> index row ->
        // append_entry) lets params lag the body whenever a session skips step 3.
        // Next-id must be max(params_max, body_max) + 1, not params_max + 1.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(
            &path,
            "# Tracker\n\n| ID | Title |\n|----|-------|\n| F-33 | body-only entry |\n\n## F-33 — body-only entry\nprose\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = path.clone();
        art_upsert(&cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-32"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[])
            .unwrap()
            .id;
        assert_eq!(id, "F-34");
    }

    #[test]
    fn body_max_index_reads_headings_and_index_rows() {
        let body =
            "## F-3 — a\n\n| ID | x |\n| `F-7` | y |\n###### **F-5** z\n| [F-4](#f-4) | w |\n";
        assert_eq!(body_max_index(body, "F"), Some(7));
    }

    #[test]
    fn body_max_index_ignores_prose_mentions() {
        // A speculative aside must not blow a hole in the numbering.
        let body = "## F-3 — a\n\nWe should file F-999 for this later. See also F-500.\n";
        assert_eq!(body_max_index(body, "F"), Some(3));
    }

    #[test]
    fn body_max_index_respects_prefix_boundaries() {
        // `F` must not match `FX-9`, and `F-12x` is not `F-12`.
        let body = "## FX-900 — other tracker\n## F-12x — malformed\n## F-2 — real\n";
        assert_eq!(body_max_index(body, "F"), Some(2));
    }

    #[test]
    fn body_max_index_returns_none_when_body_claims_nothing() {
        assert_eq!(body_max_index("# Tracker\n\nno entries yet\n", "F"), None);
    }

    #[test]
    fn append_entry_ignores_body_when_params_is_ahead() {
        // The body lagging params (the normal steady state mid-flow) must not
        // pull the next id backwards.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(&path, "## F-2 — stale body\n").unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = path.clone();
        art_upsert(&cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-9"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[])
            .unwrap()
            .id;
        assert_eq!(id, "F-10");
    }

    #[test]
    fn append_entry_tolerates_a_body_missing_from_disk() {
        // sample_art points at /test/r/<id>.md, which does not exist. A missing
        // or unreadable file must degrade to params-only, never fail the append.
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-4"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[])
            .unwrap()
            .id;
        assert_eq!(id, "F-5");
    }

    #[test]
    fn append_entry_rejects_unknown_entry_collection() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let err = append_entry(&mut cat, "art1", "bugs", "B", json!({}), &[]).unwrap_err();
        assert!(err.to_string().contains("failures"));
    }

    #[test]
    fn append_entry_rejects_missing_augmentation() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();

        let err = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[]).unwrap_err();
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
            &[],
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
            &[],
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
            &[],
        )
        .unwrap()
        .id;
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
            &[],
        )
        .unwrap()
        .id;
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
            append_entry(
                &mut cat,
                "art1",
                "failures",
                "F",
                json!({"who": "one"}),
                &[],
            )
            .unwrap()
        });
        let h2 = std::thread::spawn(move || {
            let mut cat = Catalog::open(&path2).unwrap();
            append_entry(
                &mut cat,
                "art1",
                "failures",
                "F",
                json!({"who": "two"}),
                &[],
            )
            .unwrap()
        });

        let id1 = h1.join().unwrap().id;
        let id2 = h2.join().unwrap().id;

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

    #[test]
    fn resolve_cite_ref_resolves_existing_hex_id() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("0123456789abcdef")).unwrap();

        let resolved = resolve_cite_ref(&cat.conn, "0123456789abcdef").unwrap();
        assert_eq!(resolved, "0123456789abcdef");
    }

    #[test]
    fn resolve_cite_ref_resolves_slug_local() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='trk' WHERE id='art1'", [])
            .unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("items".to_string());
        a.params = r#"{"items":[{"id":"F-1"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let resolved = resolve_cite_ref(&cat.conn, "trk:F-1").unwrap();
        assert_eq!(resolved, "trk:F-1");
    }

    #[test]
    fn resolve_cite_ref_rejects_unknown_local() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='trk' WHERE id='art1'", [])
            .unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("items".to_string());
        a.params = r#"{"items":[{"id":"F-1"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let err = resolve_cite_ref(&cat.conn, "trk:F-99").unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[test]
    fn resolve_cite_ref_rejects_ambiguous_rel_path() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("dup-a")
                .with_abs_path("/a/dup.md")
                .build(),
        )
        .unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("dup-b")
                .with_abs_path("/b/dup.md")
                .build(),
        )
        .unwrap();

        let err = resolve_cite_ref(&cat.conn, "dup.md").unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[test]
    fn resolve_cite_ref_escapes_like_wildcards_in_rel_path() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("underscore")
                .with_abs_path("/x/foo_bar.md")
                .build(),
        )
        .unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("letterx")
                .with_abs_path("/x/fooXbar.md")
                .build(),
        )
        .unwrap();

        let resolved = resolve_cite_ref(&cat.conn, "foo_bar.md").unwrap();
        assert_eq!(resolved, "underscore");
    }
}
