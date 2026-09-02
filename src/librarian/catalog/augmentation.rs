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
    /// refresh runs with a resolvable HEAD. Surfaced by doc(get) as
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
    get_by_conn(&cat.conn, artifact_id)
}

/// `get` against a bare connection, for callers that hold one rather than a `Catalog` —
/// notably `doctor`'s scan family, whose functions all take `&rusqlite::Connection`.
pub fn get_by_conn(
    conn: &rusqlite::Connection,
    artifact_id: &str,
) -> Result<Option<AugmentationRow>> {
    let mut stmt = conn.prepare(
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
    /// Set when this tracker keeps a rendered snapshot in its body and that
    /// snapshot is now behind for the patched row. `None` for a prose-only
    /// tracker, whose rows deliberately live in `params` alone.
    ///
    /// This is the harder half of the drift: `append_entry` can name a missing
    /// ID, but a patched row is usually *present* in the body showing its
    /// previous values — no id comparison can see that, so the signal is
    /// "you changed fields the body still renders the old way".
    pub snapshot_stale: Option<String>,
    /// Set when the entry has no `## <ID> — <title>` heading, so nothing can cite it.
    ///
    /// Orthogonal to `snapshot_stale`, and both can be `Some` at once: that one asks
    /// whether the body *carries* the row (an index row satisfies it), this asks
    /// whether anything can *cite* the entry (an index row does not). Unlike
    /// `snapshot_stale` it is NOT gated on `body_keeps_snapshot` — see
    /// `undefined_in_body_note` for why gating it would silence the larger half of the
    /// defect.
    pub undefined_in_body: Option<String>,
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
    // docs/issues/archive/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
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

    // Unlike `append_entry`, this path never read the body — so the third
    // instance of the drift (a status flipped in params while the committed
    // table still shows the old value) had nothing that could have noticed.
    // One read, after the write is committed: the signal is advisory and must
    // never be able to fail the mutation the caller asked for.
    // docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
    let claimed_indices: std::collections::BTreeSet<u64> = params
        .get(entry_collection)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()))
        .filter_map(|i| i.rsplit_once('-'))
        .filter_map(|(_, n)| n.parse::<u64>().ok())
        .collect();
    let snapshot_stale = snapshot_stale_note(cat, artifact_id, entry_id, &claimed_indices);
    let undefined_in_body = undefined_in_body_note(cat, artifact_id, entry_id);

    Ok(UpdateEntryOutcome {
        entry_id: entry_id.to_string(),
        changed_fields,
        entries_total,
        snapshot_stale,
        undefined_in_body,
    })
}

/// Whether `entry_id`'s tracker keeps a body snapshot that is now behind.
///
/// Gated on [`body_keeps_snapshot`] — the body must line-anchor a MAJORITY of
/// the ids in `claimed`, not merely one of them. `render_template` is the wrong
/// test in the other direction: its documented job is to project params into
/// `librarian(context)` precisely SO the body can stay prose-only, and 26 of 28
/// augmented trackers here declare one, so it would fire almost always and mean
/// almost nothing.
///
/// Best-effort throughout: an unreadable file or an unparseable id yields
/// `None`. A missing advisory is a far smaller harm than a failed update, and
/// this runs after the transaction has already committed.
fn snapshot_stale_note(
    cat: &Catalog,
    artifact_id: &str,
    entry_id: &str,
    claimed: &std::collections::BTreeSet<u64>,
) -> Option<String> {
    let (prefix, num) = entry_id.rsplit_once('-')?;
    let num: u64 = num.parse().ok()?;
    let abs_path: String = cat
        .conn
        .query_row(
            "SELECT abs_path FROM artifact WHERE id = ?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()?;
    let body = std::fs::read_to_string(&abs_path).ok()?;
    // ROW anchors only. Every message below speaks of "the row" and "the
    // committed table", so a body whose ids live in section headings has no
    // snapshot for this note to be about. See `body_snapshot_row_indices`.
    let in_body = body_snapshot_row_indices(&body, prefix);
    // Majority coverage, not mere presence: a params-canonical tracker mentions
    // a few ids in unrelated tables without maintaining a snapshot, and telling
    // it that its rows are missing on every write is noise, not a signal.
    if !body_keeps_snapshot(claimed, &in_body) {
        return None;
    }
    Some(if in_body.contains(&num) {
        // The hard half: the row IS in the body, showing its previous values.
        // No id comparison can see this, which is why `append_entry`'s
        // missing-id check would have reported nothing here.
        format!(
            "This tracker renders a snapshot in its body, and its `{entry_id}` row still shows \
             the PREVIOUS field values — params changed, the file did not. Update the row via \
             doc(action=\"update\", patch={{body_edits: [...]}}), or the committed table \
             disagrees with the catalog."
        )
    } else {
        format!(
            "This tracker renders a snapshot in its body, but `{entry_id}` is not in it at all — \
             the row exists only in the catalog, which is machine-local and git-ignored. Add it \
             via doc(action=\"update\", patch={{body_edits: [...]}})."
        )
    })
}

/// Whether `entry_id` is missing the `## <ID> — <title>` heading that makes it
/// citable, and if so which of the two failures it is.
///
/// The twin of [`snapshot_stale_note`], and the reason both are needed: that one asks
/// whether the body *carries* the row, which an index row satisfies. This asks whether
/// anything can *cite* the entry, which an index row does not satisfy at all —
/// `link_scan`'s resolver binds a token to a defining heading. An entry can therefore
/// pass every existing check and be permanently unreachable, which is exactly the bug
/// this closes (`docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`).
///
/// **Deliberately NOT gated on [`body_keeps_snapshot`].** That gate exists so a
/// params-canonical tracker is not nagged about rows it never intended to keep, and it
/// is right for the row question. Applying it here would silence precisely the larger
/// half of the defect — the ledger that keeps no definitions *at all*, whose entries
/// are the ones already broken (measured 2026-08-18: zero `BL-N` definitions repo-wide
/// against 117 cross-file citations). An advisory that goes quiet where citations break
/// is the failure being fixed, not a design to copy.
///
/// Best-effort throughout, matching `snapshot_stale_note`: an unreadable file or an
/// unparseable id yields `None`. This runs after the transaction has committed, so a
/// missing advisory is a far smaller harm than a failed write.
fn undefined_in_body_note(cat: &Catalog, artifact_id: &str, entry_id: &str) -> Option<String> {
    let (prefix, num) = entry_id.rsplit_once('-')?;
    let num: u64 = num.parse().ok()?;
    let abs_path: String = cat
        .conn
        .query_row(
            "SELECT abs_path FROM artifact WHERE id = ?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten()?;
    let body = std::fs::read_to_string(&abs_path).ok()?;
    match definition_gap(&body_defined_indices(&body, prefix), num) {
        DefinitionGap::Defined => None,
        DefinitionGap::EntryUndefined => Some(format!(
            "`{entry_id}` has no `## {entry_id} — <title>` heading in the body, so any citation \
             of it would resolve to nothing — an index row does not define a token. This ledger \
             defines its other entries, so this one is most likely an omission: add the heading \
             via doc(action=\"update\", patch={{body_edits: [...]}}). If instead this ledger \
             defines an entry only once something cites it, that is a valid convention and \
             nothing is owed here yet."
        )),
        DefinitionGap::LedgerDefinesNothing => Some(format!(
            "This ledger defines NO `{prefix}-N` heading anywhere in its body, so `{entry_id}` and \
             every other entry in it are uncitable — `link_scan` binds a token to a \
             `## {prefix}-N — <title>` heading, and index rows define nothing. This is a \
             whole-ledger format issue, not something one row can fix: see \
             docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md."
        )),
    }
}

/// Result of a successful [`append_entry`].
#[derive(Debug)]
pub struct AppendOutcome {
    /// The id assigned to the new entry.
    pub id: String,
    /// Set when the body claimed ids the params array does not carry — the
    /// append itself succeeded, but the structured index is incomplete.
    pub warning: Option<String>,
    /// The mirror of `warning`: ids `params` carry that the body line-anchors
    /// nowhere, so they live only in the machine-local, git-ignored catalog.
    /// Always includes the id just assigned. Empty for a prose-only tracker
    /// (one whose body claims no ids at all) — see `body_claimed_indices`.
    pub snapshot_missing: Vec<String>,
    /// Set when the id just assigned has no `## <ID> — <title>` heading, so nothing
    /// can cite it. Distinct from `snapshot_missing`, which an index row satisfies;
    /// see `undefined_in_body_note`.
    pub undefined_in_body: Option<String>,
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
    let body_text = abs_path.and_then(|p| std::fs::read_to_string(p).ok());
    // One read, both directions. The set answers the id-allocation question
    // (its max) AND the durability question (what the body is missing); reading
    // only the max is what left the second one unanswerable for a month.
    let body_claimed = body_text
        .as_deref()
        .map(|b| body_claimed_indices(b, id_prefix))
        .unwrap_or_default();
    let body_max = body_claimed.iter().next_back().copied();
    // A THIRD question, and it needs a narrower set than the other two: whether
    // the body renders a snapshot table. Headings must keep counting for
    // allocation above — a heading claiming F-33 has to block reissuing F-33 —
    // but they are not a snapshot, and folding them in here is what told
    // `tool-usage-patterns` (0 table rows) to update a table it does not have.
    let body_rows = body_text
        .as_deref()
        .map(|b| body_snapshot_row_indices(b, id_prefix))
        .unwrap_or_default();

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

    // The mirror direction, and the common one. The warning above fires when the
    // BODY runs ahead of params; this fires when params run ahead of the body —
    // i.e. rows that exist only in the catalog, which is machine-local and
    // git-ignored, so they are in no repo at all.
    //
    // Gated on the body already claiming at least one id: that, not the
    // augmentation config, is what distinguishes a tracker keeping a rendered
    // snapshot from a prose-only one whose rows deliberately live in params
    // alone. `render_template` is the wrong test — its documented job is to
    // project params into `librarian(context)` precisely SO the body can stay
    // prose-only, and 26 of 28 augmented trackers declare one.
    //
    // The id just assigned is included on purpose: at this moment the body does
    // not carry it, and naming it is the reminder to write the row while the
    // caller still has the context to do it.
    // docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
    let snapshot_missing: Vec<String> = {
        let mut claimed: std::collections::BTreeSet<u64> = existing_ids
            .iter()
            .filter_map(|i| i.rsplit_once('-'))
            .filter(|(p, _)| *p == id_prefix)
            .filter_map(|(_, n)| n.parse::<u64>().ok())
            .collect();
        claimed.insert(next);
        // Majority coverage, not mere presence — a params-canonical tracker can
        // line-anchor a few ids incidentally without maintaining a snapshot.
        if body_keeps_snapshot(&claimed, &body_rows) {
            claimed
                .into_iter()
                .filter(|n| !body_rows.contains(n))
                .map(|n| format!("{id_prefix}-{n}"))
                .collect()
        } else {
            Vec::new()
        }
    };

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
    // After the commit, deliberately: this reads the body off disk and must never be
    // able to fail a write that already succeeded.
    let undefined_in_body = undefined_in_body_note(cat, artifact_id, &new_id);
    Ok(AppendOutcome {
        id: new_id,
        warning,
        snapshot_missing,
        undefined_in_body,
    })
}

/// What a prose-ledger allocation assigned, and what it was derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocateOutcome {
    /// The assigned `<id_prefix>-N`.
    pub id: String,
    /// Highest index the markdown body already claims, if any.
    pub body_max: Option<u64>,
    /// Highest index a previous allocation reserved, if any. Non-`None` with a
    /// value at or above `body_max` means an id was handed out and the body has
    /// not caught up — see the leak note below.
    pub reserved_max: Option<u64>,
    /// Highest index the ledger's COMMITTED frontmatter mark carried on entry.
    /// The only one of the three that survives a clone, a move, or compaction.
    pub frontmatter_max: Option<u64>,
    /// Heading level this ledger's existing entry sections use, when it has any.
    ///
    /// Carried so the caller can phrase its "now write the section" hint in the
    /// ledger's own shape instead of asserting one. `None` means the body headed
    /// nothing — and the caller must then say the level is a default rather than an
    /// observation. See `body_entry_heading_level`, and U-40 in
    /// `docs/trackers/codescout-usage-frictions.md` for what the assertion cost.
    ///
    /// Deliberately still an OBSERVATION even when a `PendingSection` was written:
    /// the allocator formats that section at `heading_level.unwrap_or(2)`, but it does
    /// not overwrite this field with the level it chose. A caller cannot otherwise
    /// tell "the ledger uses H3" from "nothing was headed, so I picked H2", and
    /// asserting the latter as the former is U-40 itself.
    pub heading_level: Option<usize>,
    /// Whether a `PendingSection` was supplied and written in the same file write.
    ///
    /// `false` is the reserve-only path: the id is durable, the entry is the caller's
    /// to write, and the returned hint must say so. `true` means the entry is already
    /// on disk with a `def_re`-conformant heading and the caller must NOT write it
    /// again.
    pub section_written: bool,
}

/// Frontmatter key by which an artifact declares itself a ledger owning an id
/// namespace. Lives in **frontmatter**, not in the augmentation, because a
/// ledger's identity has to travel with the repo: the catalog is machine-local
/// and git-ignored, so an augmentation-based declaration is absent in a fresh
/// clone (HY-10).
pub const ENTRY_PREFIX_KEY: &str = "entry_prefix";

/// Frontmatter key prefix for a ledger's committed high-water mark, one key per
/// declared namespace: `entry_high_water_HY: 11`.
///
/// **Per-prefix scalar keys rather than one nested map**, because the surgical
/// frontmatter writers operate on single `key: value` lines
/// (`frontmatter::upsert_int_line`), and the alternative — re-emitting the whole
/// block to hold a map — reformats hand-authored files (BL-34). A ledger owning two
/// namespaces gets two independent keys, which is also what keeps the two counters
/// independently updatable without a read-modify-write over a shared value.
pub const ENTRY_HIGH_WATER_PREFIX: &str = "entry_high_water_";

/// The frontmatter key holding `id_prefix`'s committed high-water mark.
pub fn entry_high_water_key(id_prefix: &str) -> String {
    format!("{ENTRY_HIGH_WATER_PREFIX}{id_prefix}")
}

/// A body section for [`allocate_entry_id`] to write in the same file write that
/// records the high-water mark.
///
/// **Why the allocator writes it rather than the caller.** `allocate_entry_id`
/// already writes the file — it splices `entry_high_water_<PREFIX>` into frontmatter
/// inside its `IMMEDIATE` transaction. A caller that wrote the section afterwards
/// would do a second read-modify-write outside that transaction, so a peer session
/// allocating on the same file in between gets clobbered — and what gets clobbered is
/// the peer's committed mark, walking the counter BACKWARDS. That is the reissue
/// defect `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`
/// closed, reintroduced by the back door. One file write, one transaction.
///
/// The already-accepted failure mode is unchanged: a crash before the write leaks an
/// integer, which every ledger convention here tolerates. A clobbered peer mark is
/// not tolerable, and that is the difference.
#[derive(Debug, Clone)]
pub struct PendingSection {
    /// Entry title. The allocator formats the heading as `<level> <ID> — <title>`,
    /// which is exactly `link_scan`'s `def_re` shape — so an entry written this way
    /// can never be born undefined. Callers never format the heading themselves;
    /// that is the whole point (CAP-5 defect class 2).
    pub title: String,
    /// Section prose, written beneath the heading — but not always verbatim: the
    /// allocator prepends a default `**Valid:** dated <today>` line when `body`
    /// declares no class, leaves `body` untouched when it already declares one,
    /// and refuses the whole call (no id allocated, nothing written) when the
    /// declaration it does carry fails to parse.
    pub body: String,
    /// Existing heading to insert BEFORE. Required rather than optional: a wrong
    /// guess about placement on a WRITE needs manual repair, and this project's
    /// input-handling law is that writes accept an explicit target and never infer
    /// one (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`).
    pub anchor_heading: String,
}

/// The id namespaces a parsed frontmatter block declares via `entry_prefix`.
///
/// Extracted from [`allocate_entry_id`]'s body so it can be driven by
/// `both_entry_prefix_readers_agree_on_every_yaml_form` alongside the guard's
/// independent text-level reader
/// (`crate::util::librarian_guard::declared_entry_prefixes`). Returns owned
/// `String`s rather than borrowed `&str` so the two readers' outputs compare
/// directly in that test.
///
/// Scalar or sequence: a session log legitimately owns two namespaces (F-N
/// frictions and W-N wins), so `entry_prefix: [F, W]` must be as valid as
/// `entry_prefix: R`. Reservations are keyed per (artifact, prefix), so the
/// counters stay independent either way.
pub(crate) fn declared_prefixes_from_frontmatter(
    fm: Option<&crate::librarian::frontmatter::Frontmatter>,
) -> Vec<String> {
    match fm.and_then(|f| f.extra.get(ENTRY_PREFIX_KEY)) {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Allocate the next `<id_prefix>-N` for a **ledger**: an artifact that declares
/// `entry_prefix: <PREFIX>` in its frontmatter and keeps entries as
/// `## PREFIX-N` body sections and/or `| PREFIX-N |` index rows.
///
/// This is the allocator's second caller, alongside `append_entry`. The split is
/// the point: id *identity* is a ledger-wide concern, while pushing a row into
/// `params.<entry_collection>` is one particular storage choice. Nine of the ten
/// numeric prefixes in `docs/TAXONOMY.md` keep entries in prose, so gating
/// identity on params left them allocating by hand — which is how the R-N ledger
/// came to reuse nine ids for unrelated lessons.
///
/// **The counter is derived from three inputs, and committed to one.** `entry_prefix`
/// and `entry_high_water_<PREFIX>` both live in committed frontmatter, so a fresh
/// clone knows what the ledger is *and* how far it has counted. `entry_reservation`
/// is machine-local and now serves only as the within-machine race guard it is
/// actually good at, and `body_max` is a cross-check. `next` is the max of all three.
///
/// An earlier version of this function derived the counter from the reservation and
/// the body alone, and argued the machine-local table was safe to lose because it was
/// "re-derivable from the committed body". That premise held only while the live body
/// contained every id ever issued — and compaction, which moves entries out to an
/// archive companion, lowers `body_max` by design, while `doc(move)`'s graft
/// cascade-deletes the reservation. With both understating, the `.max(1)` floor
/// reissued `HY-1`, and because the resolver binds a token to its sole ACTIVE definer,
/// every historical citation silently re-pointed with no dangling or ambiguous count
/// moving. Measured 2026-08-17;
/// `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`.
///
/// **Why this is race-free without writing the entry.** The reservation is
/// recorded inside the same `IMMEDIATE` transaction that reads the maximum, so a
/// concurrent caller observes it and receives N+1. Handing back an id without
/// recording it would only move the race — that is the defect in a bare "next
/// free index" lookup, measured 2026-08-17 with a four-minute margin between two
/// sessions computing the same `R-97` (R-98). Because the reservation is durable,
/// the body write may safely follow in a separate call, which keeps entry prose
/// unconstrained by any server-side template.
///
/// A reserved-but-never-written id leaks an integer. Deliberate: integers are
/// cheap, and every ledger convention in this repo already forbids reuse.
pub fn allocate_entry_id(
    cat: &mut Catalog,
    artifact_id: &str,
    id_prefix: &str,
    section: Option<&PendingSection>,
) -> Result<AllocateOutcome> {
    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let abs_path: Option<String> = tx
        .query_row(
            "SELECT abs_path FROM artifact WHERE id = ?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(abs_path) = abs_path else {
        return Err(RecoverableError::new(format!(
            "allocate_entry_id: unknown artifact `{artifact_id}`"
        )));
    };
    let doc = std::fs::read_to_string(&abs_path).map_err(|e| {
        RecoverableError::new(format!(
            "allocate_entry_id: cannot read `{abs_path}`: {e} — the ledger's own body is \
             where the id maximum is derived from"
        ))
    })?;

    // One read, three answers: the declaration, the body maximum, and (via the
    // body) the durability check the caller needs.
    let (fm, body) = crate::librarian::frontmatter::parse(&doc)
        .map_err(|e| RecoverableError::new(format!("allocate_entry_id: {e}")))?;
    // Scalar or sequence: a session log legitimately owns two namespaces (F-N
    // frictions and W-N wins), so `entry_prefix: [F, W]` must be as valid as
    // `entry_prefix: R`. Reservations are keyed per (artifact, prefix), so the
    // counters stay independent either way.
    let declared = declared_prefixes_from_frontmatter(fm.as_ref());

    if declared.is_empty() {
        return Err(RecoverableError::with_hint(
            format!("allocate_entry_id: `{abs_path}` does not declare an entry_prefix"),
            format!(
                "A ledger declares its id namespace in FRONTMATTER, so the declaration is \
                 committed and survives a fresh clone: doc(action=\"update\", id=\"{artifact_id}\", \
                 patch={{extra: {{\"{ENTRY_PREFIX_KEY}\": \"{id_prefix}\"}}}}). Pass a list for a \
                 ledger owning two namespaces. No augmentation and no entry_collection are needed."
            ),
        ));
    }
    if !declared.iter().any(|d| d == id_prefix) {
        return Err(RecoverableError::with_hint(
            format!("allocate_entry_id: `{id_prefix}` is not declared by this ledger"),
            format!(
                "`{abs_path}` declares `{ENTRY_PREFIX_KEY}: {}` — pass one of those, or add \
                 `{id_prefix}` to the list.",
                declared.join(", ")
            ),
        ));
    }

    // Line-anchored headings and index-row leading cells only, so a prose aside
    // cannot blow a hole in the numbering. Scanned over the BODY, so a prefix
    // mentioned in frontmatter is not mistaken for an entry.
    let body_claimed = body_claimed_indices(body, id_prefix);
    let body_max = body_claimed.iter().next_back().copied();

    // The COMMITTED high-water mark, and the only input that survives the three
    // operations `body_max` and `reserved_max` cannot: a fresh clone, an
    // `doc(move)` (whose graft cascade-deletes the reservation), and
    // compaction (which lowers `body_max` BY DESIGN when entries move to an
    // archive companion). Accepts a number or a string, because a hand-written
    // or previously-quoted value should still be honoured rather than silently
    // read as absent — reading it as absent is exactly the reissue.
    let hw_key = entry_high_water_key(id_prefix);
    let frontmatter_max: Option<u64> =
        fm.as_ref()
            .and_then(|f| f.extra.get(&hw_key))
            .and_then(|v| match v {
                Value::Number(n) => n.as_u64(),
                Value::String(s) => s.trim().parse().ok(),
                _ => None,
            });

    // SQLite has no unsigned integers and rusqlite does not implement FromSql for
    // u64, so the column round-trips as i64. Clamping at 0 keeps a hand-edited
    // negative row from wrapping into a colossal id.
    let reserved_max: Option<u64> = tx
        .query_row(
            "SELECT max_allocated FROM entry_reservation WHERE artifact_id = ?1 AND prefix = ?2",
            rusqlite::params![artifact_id, id_prefix],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .map(|v| v.max(0) as u64);

    // Max of all THREE, so no single input can walk the counter backwards. That is
    // the whole property: each source is unreliable in a different way, and none of
    // them is ever wrong in the *high* direction.
    let next = body_max
        .map_or(0, |m| m + 1)
        .max(reserved_max.map_or(0, |m| m + 1))
        .max(frontmatter_max.map_or(0, |m| m + 1))
        .max(1);

    tx.execute(
        "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(artifact_id, prefix) DO UPDATE SET
           max_allocated = excluded.max_allocated,
           updated_at    = excluded.updated_at",
        rusqlite::params![artifact_id, id_prefix, next as i64],
    )?;

    // Persist the mark BEFORE committing, and fail the whole allocation if it cannot
    // be written. The ordering is the durability argument:
    //
    // * write fails → `?` returns, the transaction rolls back, no id was handed out.
    //   Refusing is right: an id whose committed mark did not advance is precisely
    //   the reissue this exists to prevent, so a silent fallback would reintroduce it.
    // * write succeeds, commit fails → frontmatter runs ahead of the database. Safe,
    //   because `next` takes the max: the following call reads the higher mark and
    //   simply skips an integer.
    //
    // Concurrency on one machine is handled by the enclosing IMMEDIATE transaction —
    // a second session blocks on the write lock, so it cannot interleave here.
    let updated =
        crate::librarian::frontmatter::upsert_int_line(&doc, &hw_key, next).ok_or_else(|| {
            RecoverableError::with_hint(
                format!(
                    "allocate_entry_id: `{abs_path}` has no frontmatter block to record \
                     `{hw_key}` in"
                ),
                "A ledger's high-water mark is COMMITTED state, so the file needs a \
                 frontmatter block to hold it. Add one — the same block already carries \
                 the `entry_prefix` declaration that made this a ledger."
                    .to_string(),
            )
        })?;
    // The entry section, spliced into the SAME string the mark went into, so one
    // `fs::write` carries both. See `PendingSection` for why this cannot be a second
    // write by the caller.
    //
    // The heading is formatted here and nowhere else: `<level> <ID> — <title>` is
    // exactly `link_scan`'s `def_re` (`^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`), so an entry
    // written through this path cannot be born undefined. Callers used to receive the
    // id plus a hint asking them to format it, and a heading missing its
    // dash-and-title defines no token — the mechanism behind ~30 of 39 sampled
    // dangling tokens in this repo. CAP-5 defect class 2.
    // Kept as two values on purpose. `observed_level` is an OBSERVATION and stays
    // `None` when the body heads nothing, because the caller phrases its hint
    // differently for a default than for a convention — collapsing them is U-40 in
    // `docs/trackers/codescout-usage-frictions.md`, where a hard-coded `##` told a
    // `###` ledger the wrong level while claiming it was the ledger's own.
    // `level` is only the formatting choice made here.
    let observed_level = body_entry_heading_level(body, id_prefix);
    let level = observed_level.unwrap_or(2);
    let id = format!("{id_prefix}-{next}");
    let updated = match section {
        None => updated,
        Some(s) => {
            let heading = "#".repeat(level);
            let prose = s.body.trim_end();
            // Every section the server writes is born with a declared decay class, the
            // same way it is born with a def_re-conformant heading: by construction, not
            // by convention. A caller that already declared one is left alone —
            // double-stamping would make the parser's first-match rule pick between the
            // caller's class and this one arbitrarily. A malformed declaration (e.g. an
            // unparsable date) propagates the parser's own error via `?` — before any
            // write happens — rather than being joined by a second `**Valid:**` line,
            // which would make the malformed one authoritative under first-wins and
            // leave the entry permanently unparseable. Repair only when exactly one
            // interpretation is correct
            // (docs/adrs/2026-07-10-repair-and-continue-input-handling.md); a malformed
            // date has none.
            let stamped = match crate::librarian::statements::parse_validity(prose)? {
                Some(_) => prose.to_string(),
                None => format!("**Valid:** dated {}\n\n{prose}", today_iso()),
            };
            // Trailing blank line so the anchor heading that follows is not glued to
            // this section's last prose line. Caught by reading a mutation test's
            // failure output, which printed `the prose\n## Template for new entries`.
            let section_text = format!("{heading} {id} — {}\n\n{stamped}\n\n", s.title);
            crate::tools::markdown::edit_markdown::perform_section_edit_ext(
                &updated,
                &s.anchor_heading,
                "insert_before",
                Some(&section_text),
                None,
                false,
            )
            .map_err(|e| {
                // The document is in memory right here, so the recovery can be
                // CONCRETE instead of a referral. Naming the last top-level headings
                // specifically, because a ledger's append anchor is conventionally its
                // final stanza and `doc(action="get")`'s heading window fills from
                // the top — which is exactly why the surface this hint used to name
                // could not answer on a long ledger.
                // docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
                let tail: Vec<String> = crate::librarian::preview::headings::parse(&updated)
                    .into_iter()
                    .filter(|h| h.level <= 2)
                    .rev()
                    .take(3)
                    .map(|h| format!("`{} {}`", "#".repeat(h.level as usize), h.text))
                    .collect();
                let tail_hint = if tail.is_empty() {
                    "This ledger has no top-level heading to anchor against, so the \
                     section must be added by hand."
                        .to_string()
                } else {
                    format!(
                        "Its last top-level headings, closest to the end first, are: {}. \
                         A ledger's append anchor is conventionally the final one.",
                        tail.join(", ")
                    )
                };
                RecoverableError::with_hint(
                    format!(
                        "allocate_entry_id: cannot place {id} before `{}`: {e} — no id was \
                         allocated and nothing was written",
                        s.anchor_heading
                    ),
                    format!(
                        "`anchor_heading` must name a heading that exists in the ledger \
                         verbatim, including its `#` prefix. {tail_hint}"
                    ),
                )
            })?
        }
    };
    std::fs::write(&abs_path, &updated).map_err(|e| {
        RecoverableError::new(format!(
            "allocate_entry_id: cannot record the high-water mark in `{abs_path}`: {e} — no \
             id was allocated"
        ))
    })?;

    tx.commit()?;

    Ok(AllocateOutcome {
        id,
        body_max,
        reserved_max,
        frontmatter_max,
        heading_level: observed_level,
        section_written: section.is_some(),
    })
}

/// Today as `YYYY-MM-DD`, UTC. `chrono` is already a workspace dependency and this
/// exact format is already used elsewhere in this module tree (`tools/update.rs`,
/// `tools/legibility_scan/mod.rs`) — a hand-rolled civil-date formatter here would be
/// exactly the duplication that drifts.
fn today_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
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

/// Every `<id_prefix>-N` index an artifact's markdown body line-anchors.
///
/// Only line-anchored occurrences count: a markdown heading (`## F-12`) or the
/// leading cell of an index-table row (`| F-12 | ... |`), optionally wrapped in
/// backticks/bold/link brackets. Those are exactly the two surfaces the
/// documented 3-step tracker flow writes. Prose mentions are deliberately
/// ignored — an aside like "planned F-999" must not blow a hole in the
/// numbering, and over-allocating is only safe when the trigger is precise.
///
/// An **empty** set is itself the signal that this artifact keeps no body
/// snapshot at all — a prose-only tracker whose rows live purely in `params`.
/// That is a legitimate design (5 of 28 augmented trackers here), so callers
/// checking for snapshot drift must treat empty as "nothing to reconcile"
/// rather than "everything is missing".
pub(crate) fn body_claimed_indices(body: &str, id_prefix: &str) -> std::collections::BTreeSet<u64> {
    let esc = regex::escape(id_prefix);
    let Ok(re) = regex::Regex::new(&format!(
        r"(?m)^(?:#{{1,6}}[ \t]+|\|[ \t]*)[`*\[]*{esc}-(\d+)\b"
    )) else {
        return Default::default();
    };
    re.captures_iter(body)
        .filter_map(|c| c[1].parse::<u64>().ok())
        .collect()
}

/// The subset of [`body_claimed_indices`] anchored in an **index-table row**
/// (`| F-12 | … |`), excluding headings.
///
/// This is the set that answers *"does this body render a snapshot of `params`?"*.
/// [`body_claimed_indices`] deliberately answers a different question — *"which
/// ids does the body claim?"* — for which a heading counts every bit as much as a
/// row, because it is what stops [`append_entry`] reissuing an id the body already
/// uses. Merging the two is what made [`body_keeps_snapshot`] unable to tell a
/// table from a set of section headings.
///
/// The distinction matters in both directions, and it is asymmetric:
///
/// - **Headings-only body** (`tool-usage-patterns`: 32 defining headings, 0 rows)
///   — heading coverage cleared the majority gate, and the caller was told to add
///   a row to a table that does not exist. There is no snapshot to be behind.
/// - **Headings masking a lagging table** (`prompt-hamsa-audit-log`: every params
///   id has a heading, the table has fewer rows) — the gate passed, but
///   `claimed.difference(in_body)` came out *empty* because the headings filled
///   the holes the rows left, so the check went silent on a genuine lag. This is
///   the worse half: a false negative that looks like health.
///
/// Nothing is lost by excluding headings here. Whether an entry has a citable
/// `## <ID> — <title>` heading is `undefined_in_body_note`'s question, it is not
/// gated on [`body_keeps_snapshot`], and it fires independently.
pub(crate) fn body_snapshot_row_indices(
    body: &str,
    id_prefix: &str,
) -> std::collections::BTreeSet<u64> {
    let esc = regex::escape(id_prefix);
    // Same anchors and same wrapper tolerance as `body_claimed_indices`, minus
    // the `#{1,6}` heading alternation. Keep the two regexes in step.
    let Ok(re) = regex::Regex::new(&format!(r"(?m)^\|[ \t]*[`*\[]*{esc}-(\d+)\b")) else {
        return Default::default();
    };
    re.captures_iter(body)
        .filter_map(|c| c[1].parse::<u64>().ok())
        .collect()
}

/// Every `<id_prefix>-N` index an artifact's markdown body **defines as a citable
/// token** — a heading of the shape `## <ID> — <title>`, and nothing else.
///
/// The narrower twin of [`body_claimed_indices`], and the two are *supposed* to
/// disagree. `body_claimed_indices` answers "is this number taken", where an index
/// row is valid evidence and over-counting is safe. This answers "can anything cite
/// this entry", where a row is worth nothing: `link_scan`'s resolver binds a token to
/// a defining heading, so a row-only entry is uncitable no matter how visible it is
/// in the rendered table.
///
/// Conflating the two is the defect this exists to close. The drift advisories built
/// on `body_claimed_indices` are satisfied by a row, so they fall silent at exactly
/// the point where citations break — an entry reads as fully written while every
/// reference to it resolves to nothing, forever, with nothing reporting it.
/// Measured 2026-08-18: ten row-only `A-N` entries with 25 dead cross-file citations,
/// and a params-rendered ledger with **zero** `BL-N` definitions against 117.
/// See `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`.
///
/// **Delegates to `link_scan::extract` on purpose — do not inline a regex here.**
/// One definition rule in the codebase is the whole point. A second hand-copied
/// approximation is the mechanism behind this bug, U-22 and U-44 alike: the rule
/// changed in one place and the copy kept answering the old way. Delegating also
/// buys the cmark-accurate cases a line regex gets wrong — fenced blocks, code-first
/// headings, setext headings, frontmatter.
///
/// An **empty** set does not mean the body is broken: a params-canonical ledger that
/// renders its index from `params` defines no token by construction, and that is a
/// legitimate design. Callers must treat empty as "this ledger defines nothing" and
/// not report per-entry breakage on every write.
pub(crate) fn body_defined_indices(body: &str, id_prefix: &str) -> std::collections::BTreeSet<u64> {
    let want = format!("{id_prefix}-");
    crate::librarian::tools::link_scan::extract::extract(body)
        .definitions
        .into_iter()
        .filter_map(|d| d.token.strip_prefix(&want)?.parse::<u64>().ok())
        .collect()
}

/// What a ledger's body fails to say about one entry, in citation terms.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DefinitionGap {
    /// A `## <ID> — <title>` heading defines it. Citable; nothing to report.
    Defined,
    /// The body defines other entries of this prefix but not this one.
    EntryUndefined,
    /// The body defines no entry of this prefix at all.
    LedgerDefinesNothing,
}

/// Classify what a ledger's body fails to say about entry `num`, given the set of
/// indices its body actually **defines** (from [`body_defined_indices`]).
///
/// Three outcomes rather than a bool, because the two failures need different
/// remedies and telling them apart is the whole value of the check:
///
/// - `EntryUndefined` — the ledger demonstrably writes definitions and this entry
///   missed one. The author writes one heading and it is fixed.
/// - `LedgerDefinesNothing` — no entry of this prefix is defined anywhere, so nothing
///   done to this one row helps: the ledger's entry format has to change. Reporting
///   this as a per-entry omission would tell a queue maintainer to "add a heading for
///   BL-39" while the other 38 stay equally uncitable.
///
/// `claimed` is deliberately not a parameter. Whether the number is *taken* has no
/// bearing on whether the entry is *citable*, and conflating those two questions is
/// the defect this whole path exists to close.
pub(crate) fn definition_gap(defined: &std::collections::BTreeSet<u64>, num: u64) -> DefinitionGap {
    if defined.contains(&num) {
        DefinitionGap::Defined
    } else if defined.is_empty() {
        DefinitionGap::LedgerDefinesNothing
    } else {
        DefinitionGap::EntryUndefined
    }
}

/// The heading level this ledger already uses for its `<id_prefix>-N` entry sections.
///
/// `None` when the body line-anchors no such heading: a params-only tracker, an index of
/// rows with no sections, or a ledger's very first entry. That distinction is the whole
/// point. Callers use this to phrase a hint, and a hint that ASSERTS a level it never
/// read is how `append_entry` came to tell the `###` U-N ledger to write `##` — against
/// its 36 siblings, its own augmentation prompt, and `docs/TAXONOMY.md`, all three of
/// which say `###` (U-40 in `docs/trackers/codescout-usage-frictions.md`).
///
/// The **mode**, not the max or the first match: a ledger can carry a stray heading at
/// another depth — a compacted archive section, a hand-written aside — and the level a
/// new entry should match is the one its siblings overwhelmingly use. Ties break to the
/// shallowest, matching markdown's own nesting sense.
///
/// Deliberately NOT folded into `body_claimed_indices`: that function answers "which ids
/// does the body claim", and its answer counts index rows, which have no heading level at
/// all. Two questions, two scanners, one shared shape.
pub(crate) fn body_entry_heading_level(body: &str, id_prefix: &str) -> Option<usize> {
    let esc = regex::escape(id_prefix);
    let re = regex::Regex::new(&format!(r"(?m)^(#{{1,6}})[ \t]+[`*\[]*{esc}-\d+\b")).ok()?;
    let mut counts: std::collections::BTreeMap<usize, usize> = Default::default();
    for c in re.captures_iter(body) {
        *counts.entry(c[1].len()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(level, n)| (n, std::cmp::Reverse(level)))
        .map(|(level, _)| level)
}

/// Does this body actually MAINTAIN a snapshot of `params`, or does it merely
/// mention a few ids in passing?
///
/// A non-empty [`body_claimed_indices`] is not enough on its own. A tracker can
/// be **params-canonical by design** — rows live in `params`, the body carries
/// narrative only for the entries that need more than a row — and still
/// line-anchor a handful of ids incidentally: the first cell of an unrelated
/// table (`| PV-25 | rule … |`), or a `### PV-2 —` write-up. Treating those as a
/// lagging snapshot nags a tracker whose author deliberately kept the rows out
/// of the file.
///
/// The discriminator is **majority coverage**: a snapshot that fell behind still
/// carries most of its rows, because it is appended to and lags at the tail; a
/// document that mentions ids carries a small minority, scattered.
///
/// Measured across the 13 snapshot-bearing trackers on the authoring machine,
/// the two populations are bimodal and do not overlap:
///
/// | coverage | shape | what it was |
/// |---|---|---|
/// | 100% | contiguous prefix | 11 maintained snapshots, in sync |
/// | 61% | contiguous prefix `1..14` | `prompt-hamsa-audit-log.md` — a real lag, caught |
/// | 21% | scattered, holes throughout | `provenance-subsystem.md` — params-canonical, a false positive |
///
/// Fails safe: a genuinely maintained snapshot that has fallen more than half
/// behind goes unreported, which is a smaller harm than telling every
/// params-canonical tracker it is broken on every write.
///
/// # The input narrowed on 2026-08-28 — read the table above as history
///
/// Those three coverage figures were measured against [`body_claimed_indices`],
/// which counts **headings and index rows alike**. Every caller now passes
/// [`body_snapshot_row_indices`] instead, so the percentages a live run produces
/// are not the ones tabulated above. The row is kept because it is what justifies
/// the majority threshold, and that reasoning still holds — but do not re-derive
/// today's numbers from it.
///
/// The threshold was never the defect. Coverage counted from the wide set is
/// simply the wrong *question*, and it failed in both directions at once:
///
/// - **False positive** — `tool-usage-patterns`, 32 defining headings and **0**
///   table rows, scored 100% and was told on every append that its rendered
///   table had fallen behind. It has no table.
/// - **False negative, the worse one** — `prompt-hamsa-audit-log`, where every
///   params id has a heading and the table has fewer rows. Coverage was 100%, the
///   gate passed, and then `claimed.difference(in_body)` came out *empty* because
///   the headings filled the holes the rows left. A real lag reported as health.
///
/// Note that those two sit on **opposite sides of every threshold**, which is the
/// evidence that no tuning could have separated them.
/// See `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`.
pub(crate) fn body_keeps_snapshot(
    claimed: &std::collections::BTreeSet<u64>,
    in_body: &std::collections::BTreeSet<u64>,
) -> bool {
    if claimed.is_empty() || in_body.is_empty() {
        return false;
    }
    claimed.intersection(in_body).count() * 2 > claimed.len()
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

/// RFC 7396 JSON Merge Patch applied in place to `target`. `null` keys in the
/// patch delete; when both the existing value and the patch value at a key are
/// objects, they merge recursively (this function calls itself); otherwise the
/// patch value replaces the target's value for that key entirely.
///
/// **Arrays are replaced wholesale, and params are no longer flat.** RFC 7396
/// does not merge arrays element-wise — a patch value that is an array (or any
/// other non-object) always replaces the corresponding target value outright.
/// `entry_collection` makes params the home for arrays of entry rows, and two
/// of the archetypes `tracker_design` recommends are built on exactly that
/// shape. So a patch carrying one row of a collection still deletes every
/// other row — legitimate for a bulk rewrite, catastrophic for the one-row
/// edit it looks like.
///
/// Two things stand between a caller and that outcome: [`update_entry`] gives
/// a one-row edit its own path, and [`merge_params`] reports
/// `entries_before`/`entries_after` so a wholesale replace is visible rather
/// than silent.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
///
/// Non-object patches are silent no-ops. Callers MUST reject them at their own
/// input boundary rather than relying on the tool schema: the schema's
/// `"type": "object"` covers only the inline `params` argument, and
/// `params_path` reads a file that never passes through it. That gap let a bare
/// top-level array report success while discarding the whole payload
/// (docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md).
pub fn apply_merge_patch(target: &mut Value, patch: &Value) {
    let (Value::Object(t), Value::Object(p)) = (target, patch) else {
        return;
    };
    for (k, v) in p {
        if v.is_null() {
            t.remove(k);
        } else if v.is_object() {
            let entry = t
                .entry(k.clone())
                .or_insert(Value::Object(Default::default()));
            if !entry.is_object() {
                *entry = Value::Object(Default::default());
            }
            apply_merge_patch(entry, v);
        } else {
            t.insert(k.clone(), v.clone());
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

    /// RFC 7396 merge-patch is recursive: a patch naming one branch of a nested
    /// object must leave untouched sibling branches under the same top-level key
    /// alone. docs/trackers/bug-artifact-augment-shallow-merge.md (changelog-reader).
    #[test]
    fn merge_params_recursively_merges_nested_objects() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.params = json!({"a": {"x": 1, "y": 2}, "b": "top-level-value"}).to_string();
        upsert(&cat, &a).unwrap();
        let patch = json!({"a": {"x": 99}});
        merge_params(&cat, "art1", &patch).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["a"]["x"], 99);
        assert_eq!(
            params["a"]["y"], 2,
            "sibling field under the patched key must survive"
        );
        assert_eq!(params["b"], "top-level-value");
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
    /// docs/issues/archive/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
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

    /// A tracker whose entries are `## F-N — title` SECTIONS and nothing else
    /// keeps no rendered snapshot, so `snapshot_missing` must stay empty. The
    /// heading question is `undefined_in_body`'s, and it is not gated — so
    /// nothing is lost by declining to answer it here twice, in the wrong words.
    ///
    /// Live false positive, 2026-08-28: appending to `tool-usage-patterns`
    /// (32 defining headings, **0** table rows) returned a hint saying *"This
    /// tracker keeps a rendered snapshot in its body"* and telling the caller to
    /// add a row to a table that does not exist. `body_claimed_indices` reads
    /// headings and index rows into one set, so heading coverage alone cleared
    /// `body_keeps_snapshot`.
    ///
    /// See `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`.
    #[test]
    fn append_entry_does_not_claim_a_snapshot_when_the_body_is_headings_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(
            &path,
            "# Tracker\n\n## F-1 — one\nprose\n\n## F-2 — two\nprose\n\n## F-3 — three\nprose\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = path.clone();
        art_upsert(&cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-1"},{"id":"F-2"},{"id":"F-3"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[]).unwrap();

        assert_eq!(out.id, "F-4");
        assert!(
            out.snapshot_missing.is_empty(),
            "no table exists, so no row can be missing from one; got {:?}",
            out.snapshot_missing
        );
        // The heading advisory is the correct one for this shape, and it must
        // still fire — otherwise this fix trades a wrong signal for no signal.
        assert!(
            out.undefined_in_body.is_some(),
            "F-4 has no `## F-4 — title` heading yet; that is the real advisory"
        );
    }

    /// The guard against over-correcting: a body that really does render a table
    /// must still be told when the table is behind. Same params as the test
    /// above, but the body carries index ROWS, so the snapshot is real.
    #[test]
    fn append_entry_still_reports_a_missing_row_when_the_body_renders_a_table() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(
            &path,
            "# Tracker\n\n| ID | T |\n|----|---|\n| F-1 | a |\n| F-2 | b |\n| F-3 | c |\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = path.clone();
        art_upsert(&cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-1"},{"id":"F-2"},{"id":"F-3"}]}"#.to_string();
        upsert(&cat, &a).unwrap();

        let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[]).unwrap();

        assert_eq!(out.id, "F-4");
        assert_eq!(
            out.snapshot_missing,
            vec!["F-4".to_string()],
            "this body DOES render a table, and it is now one row behind"
        );
    }

    #[test]
    fn body_claimed_indices_reads_headings_and_index_rows() {
        let body =
            "## F-3 — a\n\n| ID | x |\n| `F-7` | y |\n###### **F-5** z\n| [F-4](#f-4) | w |\n";
        // The whole set, not just its max: snapshot-drift detection asks WHICH
        // ids the body carries, and a max-only assertion cannot tell a
        // contiguous run from one with holes in it.
        assert_eq!(
            body_claimed_indices(body, "F"),
            [3, 4, 5, 7].into_iter().collect()
        );
    }

    /// The split that `body_claimed_indices` above deliberately does not make.
    /// Same body, same wrapper tolerance, but headings drop out — because a
    /// heading is an entry, not a row in a rendered snapshot.
    #[test]
    fn body_snapshot_row_indices_reads_rows_and_ignores_headings() {
        let body =
            "## F-3 — a\n\n| ID | x |\n| `F-7` | y |\n###### **F-5** z\n| [F-4](#f-4) | w |\n";
        assert_eq!(
            body_claimed_indices(body, "F"),
            [3, 4, 5, 7].into_iter().collect(),
            "precondition: the wide reading still sees all four"
        );
        assert_eq!(
            body_snapshot_row_indices(body, "F"),
            [4, 7].into_iter().collect(),
            "only the two `|`-anchored rows are snapshot rows"
        );
    }

    /// A body of nothing but entry sections renders no snapshot at all, and
    /// the empty set is what makes `body_keeps_snapshot` return false for it.
    /// This is the `tool-usage-patterns` shape: 32 headings, 0 rows.
    #[test]
    fn body_snapshot_row_indices_is_empty_for_a_headings_only_body() {
        let body = "# T\n\n## F-1 — a\nprose\n\n### F-2 — b\nprose\n\n#### F-3 — c\nprose\n";
        assert!(!body_claimed_indices(body, "F").is_empty());
        assert!(
            body_snapshot_row_indices(body, "F").is_empty(),
            "no `|` row anchors, so there is no table to fall behind"
        );
    }

    #[test]
    fn an_index_row_without_a_heading_is_claimed_but_not_defined() {
        // The bug, on the exact fixture the test above pins. Four ids are CLAIMED,
        // so the drift advisory stays quiet about all four; only ONE is DEFINED, so
        // citations of the other three resolve to nothing, forever, with nothing
        // reporting it. `F-7` and `F-4` are table rows; `F-5` is a heading with no
        // ` — title`, which link_scan reads as a section ABOUT F-5 rather than a
        // definition of it.
        //
        // Mutation check: make body_defined_indices accept rows and this goes red
        // while body_claimed_indices_reads_headings_and_index_rows stays green.
        // That pair is the whole point — the two predicates answer different
        // questions and must be allowed to disagree.
        let body =
            "## F-3 — a\n\n| ID | x |\n| `F-7` | y |\n###### **F-5** z\n| [F-4](#f-4) | w |\n";
        assert_eq!(
            body_claimed_indices(body, "F"),
            [3, 4, 5, 7].into_iter().collect()
        );
        assert_eq!(body_defined_indices(body, "F"), [3].into_iter().collect());
    }

    #[test]
    fn defined_indices_delegate_to_link_scans_own_definition_rule() {
        // NOT a re-implementation: body_defined_indices calls link_scan's `extract`,
        // so every case below is already pinned by that module's own tests
        // (heading_without_dash_separator_does_not_define, code_first_heading_does_not_define,
        // fenced_blocks_are_skipped_inline_code_is_scanned).
        //
        // This test exists so that a later "optimisation" swapping the call for a
        // local regex has to reproduce all of them. Re-approximating the rule in a
        // second place is exactly how the two predicates drifted apart to begin
        // with — a hand-copied predicate is the mechanism behind U-22 and U-44 too.
        let body = concat!(
            "## A-1 — defined\n",
            "## A-2\n",
            "### `A-3` — code-first\n",
            "```\n## A-4 — inside a fence\n```\n",
            "| A-5 | table row |\n",
        );
        assert_eq!(body_defined_indices(body, "A"), [1].into_iter().collect());
    }

    #[test]
    fn body_defined_indices_is_empty_when_the_body_defines_nothing() {
        // A params-rendered index defines NO token — measured 2026-08-18: zero BL-N
        // definitions repo-wide against 117 cross-file citations. So "0 defined
        // alongside N claimed" is a whole legitimate ledger shape, not per-entry
        // breakage, and any advisory built on this predicate must not fire blanket
        // on every write to such a tracker.
        let body = "# Queue\n\n| ID | task |\n|---|---|\n| BL-1 | a |\n| BL-2 | b |\n";
        assert!(body_defined_indices(body, "BL").is_empty());
        assert_eq!(
            body_claimed_indices(body, "BL"),
            [1, 2].into_iter().collect()
        );
    }

    #[test]
    fn definition_gap_is_defined_when_the_heading_exists() {
        let defined = [1u64, 2].into_iter().collect();
        assert_eq!(definition_gap(&defined, 2), DefinitionGap::Defined);
    }

    #[test]
    fn definition_gap_blames_the_entry_when_siblings_are_defined() {
        // The hybrid failure: the ledger clearly writes definitions, and this one
        // entry missed. Remedy is one heading, and the author is the right person
        // to write it — measured on the hamsa audit log, `## A-1`..`## A-14`
        // present and A-15..A-24 row-only.
        let defined = [1u64, 2].into_iter().collect();
        assert_eq!(definition_gap(&defined, 7), DefinitionGap::EntryUndefined);
    }

    #[test]
    fn definition_gap_blames_the_ledger_when_nothing_is_defined() {
        // The by-design failure, and it needs a DIFFERENT message: no entry of
        // this prefix is defined anywhere, so nothing the author does to one row
        // fixes it — the ledger's whole entry format has to change. Measured on
        // the BL-N queue: zero definitions repo-wide, 117 dead citations.
        //
        // Distinguishing the two is the point of this function. Collapsing them
        // would tell a queue maintainer to "add a heading for BL-39" when every
        // one of the other 38 is equally uncitable.
        let defined = std::collections::BTreeSet::new();
        assert_eq!(
            definition_gap(&defined, 39),
            DefinitionGap::LedgerDefinesNothing
        );
    }

    #[test]
    fn body_claimed_indices_ignores_prose_mentions() {
        // A speculative aside must not blow a hole in the numbering — nor count
        // as the body "carrying" that row for snapshot-drift purposes.
        let body = "## F-3 — a\n\nWe should file F-999 for this later. See also F-500.\n";
        assert_eq!(body_claimed_indices(body, "F"), [3].into_iter().collect());
    }

    #[test]
    fn body_claimed_indices_respects_prefix_boundaries() {
        // `F` must not match `FX-9`, and `F-12x` is not `F-12`.
        let body = "## FX-900 — other tracker\n## F-12x — malformed\n## F-2 — real\n";
        assert_eq!(body_claimed_indices(body, "F"), [2].into_iter().collect());
    }

    #[test]
    fn body_entry_heading_level_reads_the_level_the_siblings_use() {
        // The U-N ledger's real shape: H3 sections under an H1 title. Asserting H2
        // here — which the hint used to do unconditionally — is the defect (U-40).
        let body = "# Ledger\n\n### U-38 — a\n\n### U-39 — b\n";
        assert_eq!(body_entry_heading_level(body, "U"), Some(3));
    }

    #[test]
    fn body_entry_heading_level_takes_the_mode_not_the_first_or_the_max() {
        // A stray heading at another depth — a compacted archive section, an aside —
        // must not decide the level for every future entry. Two H3s outvote one H2.
        let body = "## R-1 — old, hand-written\n### R-2 — a\n### R-3 — b\n";
        assert_eq!(body_entry_heading_level(body, "R"), Some(3));
    }

    #[test]
    fn body_entry_heading_level_is_none_when_nothing_is_headed() {
        // Index rows carry no heading level, and a first entry has no sibling to
        // match. Both must read as "unknown" rather than as a default, or the caller
        // cannot tell an asserted level from an observed one.
        let body = "# Ledger\n\n| ID | Title |\n|----|-------|\n| F-7 | a row |\n";
        assert_eq!(body_entry_heading_level(body, "F"), None);
        assert_eq!(body_entry_heading_level("# Empty\n", "F"), None);
    }

    #[test]
    fn body_entry_heading_level_respects_prefix_boundaries() {
        // Same boundary rule as body_claimed_indices: `F` must not match `FX-9`.
        let body = "###### FX-900 — other ledger\n## F-2 — real\n";
        assert_eq!(body_entry_heading_level(body, "F"), Some(2));
    }

    #[test]
    fn body_claimed_indices_is_empty_when_body_claims_nothing() {
        // Empty is load-bearing: it is how a prose-only tracker is recognised,
        // so snapshot-drift checks stay silent instead of reporting every row
        // as missing. See `body_claimed_indices`.
        assert!(body_claimed_indices("# Tracker\n\nno entries yet\n", "F").is_empty());
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

    /// A ledger declares its namespace in FRONTMATTER — committed, so the
    /// declaration survives a fresh clone — and needs no augmentation and no
    /// entry_collection at all. Nine of the ten numeric prefixes in
    /// `docs/TAXONOMY.md` are this shape.
    #[test]
    fn allocate_entry_id_reads_the_body_max_from_a_frontmatter_declared_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: R\n---\n\n\
                 # Ledger\n\n| ID | Pattern |\n|----|---------|\n| R-9 | a row-only entry |\n\n\
                 ## R-5 — a body entry\n\nprose mentioning R-400, which must not count\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let out = allocate_entry_id(&mut cat, "art1", "R", None).unwrap();
        assert_eq!(out.id, "R-10", "max across BOTH body formats, plus one");
        assert_eq!(out.body_max, Some(9));
        assert_eq!(out.reserved_max, None);
    }

    /// The reason the allocation is *recorded* rather than merely returned.
    /// Between these two calls the body does not change, so a bare "next free
    /// index" lookup would hand out the same id twice — which is the measured
    /// R-97 collision (R-98) reproduced in miniature.
    #[test]
    fn allocate_entry_id_does_not_reissue_when_the_body_has_not_caught_up() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: R\n---\n\n## R-41 — an entry\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let first = allocate_entry_id(&mut cat, "art1", "R", None).unwrap();
        let second = allocate_entry_id(&mut cat, "art1", "R", None).unwrap();

        assert_eq!(first.id, "R-42");
        assert_eq!(second.id, "R-43", "the reservation must survive the read");
        assert_eq!(second.reserved_max, Some(42));
    }

    /// The regression guard for
    /// `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`.
    ///
    /// The counterpart to the test above. There the body lags the reservation and the
    /// reservation saves us. Here BOTH lag the ledger's real history — the case the
    /// old two-input derivation could not survive:
    ///
    /// * compaction moves entries OUT of the live body into an archive companion (the
    ///   ladder `get_guide("tracker-conventions")` mandates), so `body_max` is not
    ///   monotonic;
    /// * `graft_rows` cascade-deleted the reservation, so `doc(move)` reset the
    ///   counter — and archiving IS a move. A fresh clone or a second machine has the
    ///   same effect.
    ///
    /// With both understating, the `.max(1)` floor handed back `HY-1`, which the
    /// archived companion still defines. Worse than a plain collision: the resolver
    /// binds a token to its sole ACTIVE definer, so every historical citation of
    /// `HY-1` silently re-pointed while `dangling` and `ambiguous` both stayed flat.
    #[test]
    fn allocate_entry_id_never_reissues_an_id_the_archive_still_defines() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("ledger.md");
        let archive = dir.path().join("ledger-archived-entries.md");
        let entries: String = (1..=10)
            .map(|n| format!("## HY-{n} — entry {n}\n\n"))
            .collect();

        std::fs::write(
            &live,
            format!("---\nkind: tracker\nentry_prefix: HY\n---\n\n# Ledger\n\n{entries}"),
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("live");
        art.abs_path = live.clone();
        art_upsert(&cat, &art).unwrap();

        let first = allocate_entry_id(&mut cat, "live", "HY", None).unwrap();
        assert_eq!(first.id, "HY-11", "precondition: history runs to HY-10");
        assert_eq!(first.frontmatter_max, None, "no mark existed yet");

        // The load-bearing new behaviour: the mark is COMMITTED state now, in the file.
        let after = std::fs::read_to_string(&live).unwrap();
        assert!(
            after.contains("entry_high_water_HY: 11"),
            "the allocation must record its mark in frontmatter, got:\n{after}"
        );

        // Compaction. Entries keep their headings in the companion — reducing them to
        // bare rows would destroy their definitions, which is a different defect. The
        // archive artifact is seeded so the fixture states WHY a low id would be wrong.
        std::fs::write(
            &archive,
            format!(
                "---\nkind: tracker\nstatus: archived\nentry_prefix: HY\n---\n\n# Archived\n\n{entries}"
            ),
        )
        .unwrap();
        let mut arch = sample_art("archived");
        arch.abs_path = archive.clone();
        arch.status = "archived".to_string();
        art_upsert(&cat, &arch).unwrap();

        // Compact THROUGH `replace_body`, not by hand-writing the file. That is the
        // fixture decision the whole test turns on: real compaction is a body edit, and
        // a body edit preserves the frontmatter block byte for byte. Hand-writing the
        // compacted file would erase the mark and quietly test nothing.
        let compacted = crate::librarian::frontmatter::replace_body(
            &after,
            "\n# Ledger\n\nEntries archived.\n",
        )
        .expect("the ledger has a frontmatter block");
        std::fs::write(&live, &compacted).unwrap();

        // Counter loss — what the move's graft cascade, a fresh clone, or a second
        // machine each produce. Only the committed mark is left to carry the history.
        cat.conn
            .execute(
                "DELETE FROM entry_reservation WHERE artifact_id = 'live'",
                [],
            )
            .unwrap();

        let out = allocate_entry_id(&mut cat, "live", "HY", None).unwrap();
        assert_eq!(out.body_max, None, "the live body claims no id any more");
        assert_eq!(out.reserved_max, None, "the reservation is gone");
        assert_eq!(
            out.frontmatter_max,
            Some(11),
            "the committed mark is the only surviving input"
        );
        assert_eq!(
            out.id, "HY-12",
            "must not reissue an id the archived companion still defines"
        );
    }

    /// One ledger can host several namespaces — a session log carries both F-N
    /// frictions and W-N wins — so `entry_prefix` accepts a sequence, and the
    /// high-water marks must not bleed into each other. Reservations are keyed
    /// per (artifact, prefix) precisely for this.
    #[test]
    fn allocate_entry_id_keeps_prefixes_independent() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("session-log.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# Session log\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        assert_eq!(
            allocate_entry_id(&mut cat, "art1", "F", None).unwrap().id,
            "F-1"
        );
        assert_eq!(
            allocate_entry_id(&mut cat, "art1", "F", None).unwrap().id,
            "F-2"
        );
        assert_eq!(
            allocate_entry_id(&mut cat, "art1", "W", None).unwrap().id,
            "W-1",
            "a second prefix must start at 1, not inherit F's high-water mark"
        );
    }

    /// The template's own `## Index` / `## Wins Index` example rows used to be
    /// digit-shaped (`| F-1 | ... |`, `| W-1 | ... |`) — structurally
    /// indistinguishable from a real claimed entry to `body_claimed_indices`, so
    /// bootstrapping fresh from the template and allocating the first real F and
    /// W entry returned `F-2`/`W-2`, never `F-1`/`W-1`. Fixed by using a
    /// non-digit placeholder (`F-<n>`/`W-<n>`) that the allocator's `(\d+)`
    /// capture cannot match.
    /// docs/issues/archive/2026-08-21-session-log-template-example-row-burns-id.md
    #[test]
    fn fresh_session_log_template_bootstrap_allocates_f1_and_w1() {
        let template_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/templates/session-log.md");
        let template_body = std::fs::read_to_string(&template_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", template_path.display()));

        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("session-log.md");
        std::fs::write(
            &md,
            format!("---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n{template_body}"),
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        assert_eq!(
            allocate_entry_id(&mut cat, "art1", "F", None).unwrap().id,
            "F-1",
            "a fresh copy of the template must not pre-burn F-1 via its own example row"
        );
        assert_eq!(
            allocate_entry_id(&mut cat, "art1", "W", None).unwrap().id,
            "W-1",
            "a fresh copy of the template must not pre-burn W-1 via its own example row"
        );
    }

    #[test]
    fn allocate_entry_id_requires_a_declared_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("not-a-ledger.md");
        // Note the fixture: the body DOES carry a `## R-4` heading. A ledger is
        // declared, never inferred from content — inferring would turn every design
        // doc that quotes an id into an allocatable namespace. Measured 2026-08-17:
        // 27 unaugmented trackers in this repo, only THREE of them ledgers.
        std::fs::write(
            &md,
            "---\nkind: tracker\ntitle: A design doc, not a ledger\n---\n\n## R-4 — looks like one\n",
        )
        .unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let err = allocate_entry_id(&mut cat, "art1", "R", None).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("does not declare an entry_prefix"),
            "expected a declare-the-ledger error, got: {text}"
        );
        // A hint that names an impossible call is worse than no hint. An earlier
        // draft of this error prescribed `artifact_augment(merge=true, …)`, which
        // refuses with "call artifact_augment first" exactly when no augmentation
        // exists — the state the error reported. Caught by running it, not by
        // re-reading it. Assert the hint names the remedy that works from THIS
        // state: the frontmatter declaration.
        assert!(
            text.contains("extra") && text.contains("entry_prefix"),
            "the hint must name the frontmatter declaration: {text}"
        );
        assert!(
            // Anchor on the CALL SHAPE, not the word: `!contains("augmentation")`
            // fails on the correct hint, which mentions augmentation only to say
            // none is needed. Fourth time in two days that a keyword check counted
            // a document's discussion of a token as a use of it.
            !text.contains("artifact_augment("),
            "the declaration is frontmatter, not an augmentation — a hint pointing at \
             the catalog would recreate the portability defect HY-10 names: {text}"
        );
    }

    /// The claim the whole prototype exists to test: two OS-level writers on
    /// one catalog file, against a ledger with no params collection at all,
    /// must not receive the same id. Mirrors the params-path test above.
    #[test]
    fn allocate_entry_id_serializes_across_independent_connections() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("cat.sqlite");
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: R\n---\n\n# Ledger\n\n## R-41 — an entry\n",
        )
        .unwrap();

        {
            let cat = Catalog::open(&db).unwrap();
            let mut art = sample_art("art1");
            art.abs_path = md.clone();
            art_upsert(&cat, &art).unwrap();
        }

        let (p1, p2) = (db.clone(), db.clone());
        let h1 = std::thread::spawn(move || {
            let mut cat = Catalog::open(&p1).unwrap();
            allocate_entry_id(&mut cat, "art1", "R", None).unwrap().id
        });
        let h2 = std::thread::spawn(move || {
            let mut cat = Catalog::open(&p2).unwrap();
            allocate_entry_id(&mut cat, "art1", "R", None).unwrap().id
        });

        let mut ids = vec![h1.join().unwrap(), h2.join().unwrap()];
        ids.sort();
        assert_eq!(
            ids,
            vec!["R-42".to_string(), "R-43".to_string()],
            "concurrent allocations against a PROSE ledger must not collide"
        );
    }

    /// The guard and the allocator each read `entry_prefix`, by different mechanisms,
    /// and they must agree — a disagreement is silent in the dangerous direction: the
    /// allocator honours a form the guard is blind to, so entries in that ledger can
    /// be hand-written past the allocator with no error anywhere.
    ///
    /// Two readers is forced, not sloppy. `src/util/librarian_guard.rs` compiles under
    /// `--no-default-features` where `serde_yml` does not exist, so it hand-parses;
    /// this side already parses `fm` for `entry_high_water_<PREFIX>` and would be made
    /// worse, not better, by reading one frontmatter block two ways. What is NOT
    /// acceptable is holding the agreement in a doc comment — this project has paid
    /// for prose-enforced co-change contracts before
    /// (`docs/adrs/2026-07-25-embedding-transport-boundary.md`, where a duplicated
    /// `reqwest` client carried "Mirrors the codescout-embed RemoteEmbedder guard" and
    /// cost 48 needlessly-compiled crates). This test is the mechanism that comment
    /// stood in for.
    #[test]
    fn both_entry_prefix_readers_agree_on_every_yaml_form() {
        for (label, doc) in [
            ("scalar", "---\nkind: tracker\nentry_prefix: R\n---\n\n# L\n"),
            (
                "quoted scalar",
                "---\nkind: tracker\nentry_prefix: 'HY'\n---\n\n# L\n",
            ),
            (
                "double-quoted scalar",
                "---\nkind: tracker\nentry_prefix: \"HY\"\n---\n\n# L\n",
            ),
            (
                "inline flow",
                "---\nkind: tracker\nentry_prefix: [F, W]\n---\n\n# L\n",
            ),
            (
                "block sequence",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# L\n",
            ),
            (
                "sequence then sibling key",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\nentry_high_water_F: 3\n---\n\n# L\n",
            ),
            ("absent", "---\nkind: tracker\n---\n\n# L\n"),
            ("bare key", "---\nkind: tracker\nentry_prefix:\n---\n\n# L\n"),
            (
                "empty string",
                "---\nkind: tracker\nentry_prefix: ''\n---\n\n# L\n",
            ),
            (
                "empty flow list",
                "---\nkind: tracker\nentry_prefix: []\n---\n\n# L\n",
            ),
            ("no frontmatter at all", "# L\n\nentry_prefix: R\n"),
        ] {
            let (fm, _body) = crate::librarian::frontmatter::parse(doc).unwrap();
            let librarian_side = declared_prefixes_from_frontmatter(fm.as_ref());
            let guard_side = crate::util::librarian_guard::declared_entry_prefixes(doc);
            assert_eq!(
                librarian_side, guard_side,
                "{label}: the allocator and the guard must read entry_prefix identically — \
                 a form only one of them honours is a silent hole in the guard"
            );
        }
    }

    /// The point of the whole path: the SERVER formats the heading, so an entry cannot
    /// be born undefined. `link_scan`'s `def_re` is
    /// `^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`, and a heading missing its dash-and-title
    /// defines no token — the mechanism behind ~30 of 39 sampled dangling tokens in
    /// this repo. Reserve-and-let-the-agent-write leaves that failure available;
    /// this closes it. CAP-5 defect class 2.
    #[test]
    fn a_written_section_gets_a_def_re_conformant_heading_at_the_ledgers_own_level() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        // Existing entries are H3, so the new one must be H3 — not a hard-coded H2.
        // U-40 in docs/trackers/codescout-usage-frictions.md is what happens otherwise.
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n# Ledger\n\n### U-38 — first\n\nbody\n\n## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "server wrote this".to_string(),
            body: "the prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        let out = allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

        assert_eq!(out.id, "U-39");
        assert!(out.section_written, "the section must be reported written");
        assert_eq!(
            out.heading_level,
            Some(3),
            "heading_level stays an OBSERVATION of the ledger, not the level we chose"
        );

        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("### U-39 — server wrote this"),
            "heading must be `<level> <ID> — <title>` at the ledger's own level: {text}"
        );
        assert!(
            text.contains("the prose"),
            "the section body must be written: {text}"
        );
        // The anchor still follows it — inserted BEFORE, not appended at the end.
        let entry_at = text.find("### U-39").unwrap();
        let anchor_at = text.find("## Template for new entries").unwrap();
        assert!(
            entry_at < anchor_at,
            "the entry must be inserted before the anchor, not after it: {text}"
        );
        assert!(
            text.contains("the prose\n\n## Template for new entries"),
            "a blank line must separate the section from the anchor heading that \
             follows, or the heading is glued to the last prose line: {text}"
        );
        // And the mark advanced in the SAME write.
        assert!(
            text.contains("entry_high_water_U: 39"),
            "the committed mark must advance in the same file write: {text}"
        );
    }

    /// One file write, or the peer race comes back. `allocate_entry_id` writes the
    /// frontmatter mark inside its `IMMEDIATE` transaction; a caller that wrote the
    /// section afterwards would read-modify-write the file a second time outside that
    /// transaction, and a peer allocating in between would have its committed mark
    /// clobbered — walking the counter BACKWARDS, which is the reissue defect
    /// `2026-08-17-ledger-id-reissue-silently-repoints-citations.md` closed.
    ///
    /// Asserting on the OUTCOME of a failed placement is how that is pinned: a bad
    /// anchor must leave the file byte-identical, which is only possible if the mark
    /// and the section share one write.
    #[test]
    fn a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let before = "---\nkind: tracker\nentry_prefix: R\n---\n\n# Ledger\n\n## R-7 — an entry\n";
        std::fs::write(&md, before).unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "never lands".to_string(),
            body: "x".to_string(),
            anchor_heading: "## No Such Heading".to_string(),
        };
        let err = allocate_entry_id(&mut cat, "art1", "R", Some(&section)).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("No Such Heading"),
            "the error must name the anchor it could not find: {text}"
        );

        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "a failed placement must leave the file byte-identical — the mark and the \
             section share one write, so neither lands"
        );

        // And the id was not consumed: the next successful call still gets R-8.
        let ok = allocate_entry_id(&mut cat, "art1", "R", None).unwrap();
        assert_eq!(
            ok.id, "R-8",
            "a refused placement must not burn the id it would have used"
        );
    }

    /// docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
    ///
    /// The old hint sent the caller to `doc(action="get")` to discover the
    /// anchor. On the artifact class this feature exists for, that surface cannot
    /// answer: its heading window fills from the TOP, and a ledger's append anchor
    /// is its LAST heading (`append_entry` inserts *before* it), so on a long ledger
    /// the needed heading was dropped every single time. Measured on
    /// `reconnaissance-patterns.md`: 20 of 92 headings returned, all from lines
    /// 1–607, with the anchor at line 4038 of 4100.
    ///
    /// The document is already in memory at the failure, so the recovery belongs
    /// here rather than in a referral to a surface that truncates.
    #[test]
    fn a_bad_anchor_names_the_anchors_that_do_exist() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: R\n---\n\n# Ledger\n\n## R-7 — an entry\n\n## Template for new entries\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "never lands".to_string(),
            body: "x".to_string(),
            anchor_heading: "## No Such Heading".to_string(),
        };
        let err = allocate_entry_id(&mut cat, "art1", "R", Some(&section)).unwrap_err();
        let text = err.to_string();

        assert!(
            text.contains("## Template for new entries"),
            "the hint must name the anchor that actually exists, with its `#` prefix, \
             so the retry can be composed from the error alone: {text}"
        );
        assert!(
            !text.contains("doc(action=\"get\""),
            "the hint must NOT prescribe the surface that cannot answer — that \
             referral is the defect this closes: {text}"
        );
    }

    /// New entries are born with a declared decay class the same way they are born
    /// with a def_re-conformant heading: by construction, not by convention.
    #[test]
    fn allocator_stamps_a_default_validity_into_the_section_it_writes() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "server wrote this".to_string(),
            body: "the prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

        let written = std::fs::read_to_string(&md).unwrap();
        // Pins the exact stamped form: today's date (not some other format), a blank
        // line separating the stamp from the prose (or the anchor heading glues to
        // it), and the stamp landing BEFORE the prose, not after.
        let today = Utc::now().format("%Y-%m-%d").to_string();
        assert!(
            written.contains(&format!("**Valid:** dated {today}\n\nthe prose")),
            "a section the server writes must be born with today's date, blank-line \
             separated, immediately ahead of the caller's prose:\n{written}"
        );
    }

    /// `s.body.trim_end()` matters beyond cosmetics: the section must still end with
    /// exactly the `\n\n` this file's own trailing-blank-line comment promises before
    /// the anchor heading. A body that already carries trailing blank lines would
    /// otherwise stack past that, gluing extra blank lines in front of the anchor
    /// instead of exactly one.
    #[test]
    fn allocator_trims_trailing_whitespace_from_the_body_before_stamping() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "trailing whitespace".to_string(),
            body: "the prose\n\n\n".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

        let written = std::fs::read_to_string(&md).unwrap();
        assert!(
            written.contains("the prose\n\n## Template for new entries"),
            "trailing whitespace in the caller's body must be trimmed, leaving \
             exactly one blank line before the anchor heading:\n{written}"
        );
    }

    /// The `None` branch depends on `parse_validity` skipping fenced code blocks —
    /// a body whose ONLY `**Valid:**` line sits inside a worked-example fence must
    /// still read as `Ok(None)` and get stamped, not be treated as a caller
    /// declaration. Pinned from this side of the module boundary (this task may not
    /// touch `statements.rs`): a future change there that stops skipping fences
    /// would flip this test red without anything else here noticing, and the entry
    /// would be born with the fenced example as its FIRST (and so authoritative,
    /// under first-wins) `**Valid:**` line.
    #[test]
    fn allocator_stamps_when_the_only_valid_line_is_inside_a_fence() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "worked example".to_string(),
            body: "Example syntax:\n\n```\n**Valid:** invariant\n```\n\nthe prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

        let written = std::fs::read_to_string(&md).unwrap();
        let today = Utc::now().format("%Y-%m-%d").to_string();
        assert!(
            written.contains(&format!("**Valid:** dated {today}")),
            "a fenced **Valid:** line is a worked example, not a declaration — the \
             section must still be stamped with a real one:\n{written}"
        );
        assert_eq!(
            written.matches("**Valid:**").count(),
            2,
            "the fenced example line plus exactly one stamped declaration — never a \
             second REAL declaration:\n{written}"
        );
    }

    /// An explicit class is left alone — double-stamping would make the parser's
    /// first-match rule pick between the caller's class and the stamped default
    /// arbitrarily.
    #[test]
    fn allocator_does_not_double_stamp_a_caller_declared_class() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "a law".to_string(),
            body: "**Valid:** invariant\n\nthe prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

        let written = std::fs::read_to_string(&md).unwrap();
        assert_eq!(
            written.matches("**Valid:**").count(),
            1,
            "an explicit class must not be joined by a stamped default:\n{written}"
        );
        assert!(written.contains("**Valid:** invariant"));
    }

    /// A malformed `**Valid:**` (e.g. an unparsable date) has no correct repair:
    /// joining it with a stamped default would make the malformed line authoritative
    /// under first-wins (`parse_validity` takes the FIRST declaration), and
    /// permanently unparseable. Refusing outright is the only interpretation that has
    /// exactly one reading (docs/adrs/2026-07-10-repair-and-continue-input-handling.md).
    /// Asserting the file is byte-identical pins "no id allocated, nothing written" —
    /// same shape as `a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`.
    #[test]
    fn allocator_refuses_a_malformed_caller_declared_class_rather_than_double_stamping() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let before = "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n";
        std::fs::write(&md, before).unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "a broken law".to_string(),
            body: "**Valid:** dated notadate\n\nthe prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
        };
        let err = allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("not an ISO date"),
            "the parser's own error must propagate, not be swallowed: {text}"
        );

        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "a refused stamp must leave the file byte-identical — no second `**Valid:**` line"
        );

        // And the id was not consumed: the next successful call still gets U-2.
        let ok = allocate_entry_id(&mut cat, "art1", "U", None).unwrap();
        assert_eq!(
            ok.id, "U-2",
            "a refused stamp must not burn the id it would have used"
        );
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
