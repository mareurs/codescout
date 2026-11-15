use crate::librarian::catalog::cite_text::{grammar_form_of, with_cites};
use crate::librarian::catalog::dual_write::{
    commit_after_file_write, find_unrecorded_section, frontmatter_mark, record_written_file,
    write_entry_into_ledger, FileWrite, RetryRule,
};
use crate::librarian::catalog::Catalog;
use crate::librarian::derived::diff::{scope_of, Scope};
use crate::librarian::identity::key::key_of;
use crate::librarian::tools::{schema_validate, LibrarianRecoverableError};
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

/// The columns of `artifact_augmentation`, in the order [`row_from_sql`] reads them.
pub(crate) const AUGMENTATION_COLUMNS: &str = "artifact_id, prompt, params, last_refreshed_at, \
     refresh_count, created_at, updated_at, render_template, params_schema, append_mode, \
     history_cap, entry_collection, refreshed_at_commit";

/// `get` against a bare connection, for callers that hold one rather than a `Catalog` —
/// notably `doctor`'s scan family, whose functions all take `&rusqlite::Connection`.
pub fn get_by_conn(
    conn: &rusqlite::Connection,
    artifact_id: &str,
) -> Result<Option<AugmentationRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {AUGMENTATION_COLUMNS} FROM artifact_augmentation WHERE artifact_id = ?1"
    ))?;
    let mut rows = stmt.query_map([artifact_id], row_from_sql)?;
    Ok(rows.next().transpose()?)
}

pub(crate) fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<AugmentationRow> {
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
                return Err(LibrarianRecoverableError::new(format!(
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
            LibrarianRecoverableError::new(format!(
                "merge_params: patch violates params_schema: {e}"
            ))
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
pub(crate) fn entry_count(cat: &Catalog, artifact_id: &str) -> Result<Option<usize>> {
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
    /// True when this call re-rendered the patched row into the committed snapshot
    /// table, so the body and `params` already agree and `snapshot_stale` will be
    /// `None` for a reason other than "this tracker keeps no snapshot".
    ///
    /// Surfaced because the two silences are otherwise indistinguishable to a caller,
    /// and they license opposite next actions: nothing to do, versus update the table
    /// by hand. That is the same trap `a1ca3baa` fixed one field over, where
    /// `snapshot_missing` kept naming the id whose row the call had just written.
    pub row_resynced: bool,
    /// True when the committed row was COMPARED to the render and found byte-identical,
    /// so nothing needed writing.
    ///
    /// A third silence, added 2026-09-14 for the reason the second one exists. Folding it
    /// into `row_resynced == false` would put it back beside "this tracker keeps no
    /// snapshot" — and those license opposite next actions again, one step further out:
    /// *the body is verified current* versus *nobody looked*. The field above already paid
    /// for that lesson; widening its blind spot rather than repeating its fix would be the
    /// same defect with a new member.
    ///
    /// Mutually exclusive with `row_resynced` by construction — both come from one
    /// [`SnapshotRow`] — and both being `false` means `Undetermined`, which is exactly when
    /// `snapshot_stale` may be `Some`.
    pub row_already_current: bool,
}

/// The augmentation columns [`update_entry`] reads in one query:
/// `(params, params_schema, entry_collection, render_template)`.
///
/// An alias rather than a bare tuple because `clippy::type_complexity` denies the
/// four-element form under the gate's long clippy invocation — and only there, not
/// under a bare `cargo clippy`, which is why it is easy to add locally and discover
/// from someone else's blocked gate.
type EntryUpdateColumns = (String, Option<String>, Option<String>, Option<String>);

/// The handle a row is addressed by in `update_entry`: its `id`, else its `key`, with a flag
/// that is `true` for the `key` case. A row that has both is addressed by `id` only, so its
/// `key` is an ordinary field there. `None` for a row that carries neither.
///
/// Collections whose rows carry `key` and no `id` exist — the legibility backlog's
/// `candidates` are `CandidateRow { key, .. }` — and before this the action could not reach
/// any of them. `id` stays the citable handle: `append_entry` allocates it and `link_scan`
/// binds it. A `key` is only an address and never becomes citable.
/// docs/issues/archive/2026-10-06-update-entry-cannot-address-rows-keyed-by-key-and-prints-an-empty-known-ids-list.md
fn entry_handle(e: &Value) -> Option<(&str, bool)> {
    if let Some(id) = e.get("id").and_then(Value::as_str) {
        return Some((id, false));
    }
    e.get("key").and_then(Value::as_str).map(|key| (key, true))
}

/// What `entry_id` names in an entry array.
enum EntryMatch {
    /// Exactly one row; `by_key` says whether it was reached through its `key`.
    One {
        position: usize,
        by_key: bool,
    },
    /// More than one candidate: never guessed, refused by name. The text says which.
    Ambiguous(String),
    None,
}

/// Find the row `entry_id` addresses.
///
/// An `id` match keeps the original behaviour exactly: the first row with that `id` (so a
/// collection that already has duplicate ids behaves as before). A `key` match is only
/// taken on a row with no `id`, and only when it is unique. The two kinds never mix
/// silently: a value that is one row's `id` and also another row's `key` is ambiguous, and so
/// is a `key` shared by two rows.
fn locate_entry(arr: &[Value], entry_id: &str) -> EntryMatch {
    let id_hit = arr
        .iter()
        .position(|e| e.get("id").and_then(Value::as_str) == Some(entry_id));
    let key_hits: Vec<usize> = arr
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(entry_handle(e), Some((h, true)) if h == entry_id))
        .map(|(i, _)| i)
        .collect();
    match (id_hit, key_hits.len()) {
        (Some(position), 0) => EntryMatch::One {
            position,
            by_key: false,
        },
        (Some(_), n) => EntryMatch::Ambiguous(format!(
            "it is the `id` of one row and the `key` of {n} other row(s)"
        )),
        (None, 1) => EntryMatch::One {
            position: key_hits[0],
            by_key: true,
        },
        (None, 0) => EntryMatch::None,
        (None, n) => EntryMatch::Ambiguous(format!("it is the `key` of {n} rows")),
    }
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
/// A row is addressed by its `id`, or by its `key` when it has no `id` (see
/// [`locate_entry`]); a patch may not rewrite the `key` that is a row's only address.
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
        return Err(LibrarianRecoverableError::new(
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
        return Err(LibrarianRecoverableError::with_hint(
            "update_entry: `fields` is empty — there is nothing to patch".to_string(),
            "Pass at least one field, e.g. fields={\"status\": \"done\"}; a null value deletes a \
             key. If you passed the patch under a different parameter name, note that `entry` \
             belongs to append_entry — this action takes `fields`."
                .to_string(),
        ));
    }
    if patch.contains_key("id") {
        return Err(LibrarianRecoverableError::with_hint(
            "update_entry: `id` cannot be changed through a field patch".to_string(),
            "Entry ids key entry_cite rows (`<slug>:<local>`), so re-keying one HERE would \
             strand every citation of it. To move a whole PREFIX-N namespace — ids, schema \
             pattern, headings and citation rows together — use \
             doc(action=\"rekey_prefix\", id=…, from=\"T\", to=\"SRI\"), which is dry-run by \
             default. To retire one entry, append a new one and mark this superseded."
                .to_string(),
        ));
    }

    let roots = cat.write_roots();
    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let row: Option<EntryUpdateColumns> = tx
        .query_row(
            "SELECT params, params_schema, entry_collection, render_template
             FROM artifact_augmentation WHERE artifact_id = ?1",
            [artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    let Some((params_text, params_schema, declared_collection, render_template)) = row else {
        return Err(LibrarianRecoverableError::new(format!(
            "update_entry: artifact `{artifact_id}` has no augmentation — augment it with an entry_collection first"
        )));
    };

    if declared_collection.as_deref() != Some(entry_collection) {
        return Err(LibrarianRecoverableError::with_hint(
            format!("update_entry: `{entry_collection}` is not this artifact's entry_collection"),
            match declared_collection {
                Some(c) => format!("This artifact's entry_collection is `{c}` — pass that instead."),
                None => "This artifact has no entry_collection declared — set one via doc(action=\"augment\") first.".to_string(),
            },
        ));
    }

    let mut params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));

    // Locate the row before mutating, so the immutable borrow ends before the
    // schema re-validation needs `params` whole again.
    let Some(arr) = params.get(entry_collection).and_then(|v| v.as_array()) else {
        return Err(LibrarianRecoverableError::new(format!(
            "update_entry: `{entry_collection}` holds no entry array on artifact `{artifact_id}`"
        )));
    };
    let entries_total = arr.len();
    // Addressing: a row's `id`, else its `key`. See `locate_entry` for the rules and why the
    // two never mix silently.
    let (position, by_key) = match locate_entry(arr, entry_id) {
        EntryMatch::One { position, by_key } => (Some(position), by_key),
        EntryMatch::Ambiguous(detail) => {
            return Err(LibrarianRecoverableError::with_hint(
                format!(
                    "update_entry: `{entry_id}` is ambiguous in `{entry_collection}`: {detail}"
                ),
                "update_entry will not guess between rows. It cannot add an `id` either, so give \
                 the rows distinct addresses by rewriting the collection through \
                 doc(action=\"augment\", merge=true) with the full array, then retry."
                    .to_string(),
            ));
        }
        EntryMatch::None => (None, false),
    };

    let Some(position) = position else {
        // Name what IS there. A bare "not found" makes the caller re-read the
        // whole collection just to discover it typed the id — which is the
        // read-then-write this path exists to remove. A row is shown by the handle
        // `locate_entry` would accept for it: its `id`, else its `key`.
        const KNOWN_IDS_SHOWN: usize = 12;
        let handles: Vec<(&str, bool)> = arr.iter().filter_map(entry_handle).collect();
        let known: Vec<&str> = handles
            .iter()
            .map(|(h, _)| *h)
            .take(KNOWN_IDS_SHOWN)
            .collect();
        let shown_keys = handles
            .iter()
            .take(KNOWN_IDS_SHOWN)
            .filter(|(_, is_key)| *is_key)
            .count();
        let label = match (shown_keys, known.len()) {
            (0, _) => "Known ids",
            (k, n) if k == n => "Known keys",
            _ => "Known ids and keys",
        };
        let elided = handles.len().saturating_sub(known.len());
        let mut suffix = if elided > 0 {
            format!(" (+{elided} more)")
        } else {
            String::new()
        };
        let unaddressable = entries_total.saturating_sub(handles.len());
        if unaddressable > 0 {
            suffix.push_str(&format!(
                " ({unaddressable} row(s) carry neither an `id` nor a `key` and cannot be addressed)"
            ));
        }
        return Err(LibrarianRecoverableError::with_hint(
            format!("update_entry: no entry `{entry_id}` in `{entry_collection}`"),
            format!("{label}: {}{}", known.join(", "), suffix),
        ));
    };

    // A row addressed by `key` has no other handle, so a patch that rewrites `key` would
    // re-address it. Unlike `id`, a `key` keys no `entry_cite` rows, so nothing is stranded;
    // it is refused because the caller's next `entry_id` would silently stop matching.
    if by_key && patch.contains_key("key") {
        return Err(LibrarianRecoverableError::with_hint(
            format!("update_entry: `key` cannot be changed through a field patch on `{entry_id}`"),
            "This row has no `id`, so its `key` is its only address in the collection. To \
             re-address it, rewrite the collection through doc(action=\"augment\", \
             merge=true) with the full array."
                .to_string(),
        ));
    }

    let Some(obj) = params[entry_collection][position].as_object_mut() else {
        return Err(LibrarianRecoverableError::new(format!(
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
            LibrarianRecoverableError::new(format!(
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
    // Re-render this row in the committed table BEFORE the commit, so a failed splice
    // rolls the params write back rather than creating a new disagreement. A no-op for
    // any artifact that declares no `snapshot_anchor` — see `resync_snapshot_row`.
    let snapshot_row = resync_snapshot_row(
        &tx,
        &roots,
        artifact_id,
        entry_id,
        &params,
        render_template.as_deref(),
    )?;
    tx.commit()?;

    // The advisory below is now the FALLBACK path, not the only one. An artifact that
    // declares a `snapshot_anchor` had its row re-rendered above, inside the
    // transaction; everything else still only gets told. Kept because that "everything
    // else" is the majority — most trackers keep no body snapshot at all — and because
    // an artifact whose row exists in params but not yet in the table is reported here
    // rather than having a row invented for it by guess.
    //
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
    // Both silences below are EARNED, and by different observations: `Rewritten` means
    // this call just wrote the row, `AlreadyCurrent` means the rendered row and the body
    // row were compared and found byte-identical. Only `Undetermined` reaches the
    // advisory, and that is the case where nothing about the cells was observed at all.
    //
    // Until 2026-09-14 this was `if row_resynced`, and `AlreadyCurrent` fell through to
    // the note — which then inferred from id PRESENCE that the row "still shows the
    // PREVIOUS field values" and said so as fact, on a body the comparison three lines
    // earlier had just proven current. Patching a field the template does not render hit
    // it every time, because the rendered row cannot change and the note never sees which
    // fields moved.
    // docs/issues/archive/2026-09-13-snapshot-stale-tests-row-presence-and-reports-a-content-claim.md
    // Both notes below reason about CITABLE entry ids (`PREFIX-N` headings and rows). A row
    // addressed by `key` has no such id and `link_scan` never binds its `key`, so neither note
    // has a premise to check: say nothing rather than parse a `key` as if it were an id.
    let snapshot_stale = match snapshot_row {
        SnapshotRow::Rewritten | SnapshotRow::AlreadyCurrent => None,
        SnapshotRow::Undetermined if by_key => None,
        SnapshotRow::Undetermined => {
            snapshot_stale_note(cat, artifact_id, entry_id, &claimed_indices)
        }
    };
    let undefined_in_body = if by_key {
        None
    } else {
        undefined_in_body_note(cat, artifact_id, entry_id)
    };

    Ok(UpdateEntryOutcome {
        entry_id: entry_id.to_string(),
        changed_fields,
        entries_total,
        snapshot_stale,
        undefined_in_body,
        row_resynced: matches!(snapshot_row, SnapshotRow::Rewritten),
        row_already_current: matches!(snapshot_row, SnapshotRow::AlreadyCurrent),
    })
}

/// Whether `entry_id`'s tracker keeps a body snapshot that MAY now be behind.
///
/// Gated on [`body_keeps_snapshot`] — the body must line-anchor a MAJORITY of
/// the ids in `claimed`, not merely one of them. `render_template` is the wrong
/// test in the other direction: its documented job is to project params into
/// `librarian(context)` precisely SO the body can stay prose-only, and 26 of 28
/// augmented trackers here declare one, so it would fire almost always and mean
/// almost nothing.
///
/// **This function reads IDS and never CELLS, and every message it emits must stay inside
/// that.** It receives `claimed` — a set of integers — and not the patched fields, so it
/// cannot know whether a present row's values moved; it could not compare them if it
/// wanted to. Until 2026-09-14 the present-row branch nonetheless asserted *"still shows
/// the PREVIOUS field values — params changed, the file did not"*, a claim about content
/// derived from a check on presence. It fired on bodies that were provably current: patch
/// a field the template does not render and the rendered row cannot change, yet the note
/// reported it stale every time. The wording now states the limit, because the limit is
/// real and permanent here rather than a gap to close.
///
/// **The case where the answer IS knowable does not reach this function at all.** When the
/// artifact declares a `snapshot_anchor`, `resync_snapshot_row` compares the rendered row
/// to the body row and the caller suppresses this note on both
/// [`SnapshotRow::Rewritten`] and [`SnapshotRow::AlreadyCurrent`]. So arriving here means
/// nothing was observed — which is why the remedy text names declaring an anchor as the
/// way to get a real answer rather than a warning.
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
    let body = crate::librarian::read_door::for_catalog(cat)
        .text(std::path::Path::new(&abs_path))
        .ok()?;
    // ROW anchors only, and only rows inside the DECLARED block. Every message
    // below speaks of "the row" and "the committed table", so a body whose ids
    // live in section headings has no snapshot for this note to be about — and a
    // row in an unrelated table is not the table being spoken of either. Falls
    // back to the whole document when nothing is declared; see
    // `snapshot_rows_in_declared_block`.
    let in_body = snapshot_rows_in_declared_block(&body, prefix);
    // Majority coverage, not mere presence: a params-canonical tracker mentions
    // a few ids in unrelated tables without maintaining a snapshot, and telling
    // it that its rows are missing on every write is noise, not a signal.
    if !body_keeps_snapshot(claimed, &in_body) {
        return None;
    }
    Some(if in_body.contains(&num) {
        // PRESENCE is the whole of what this branch established. Reaching it means
        // `resync_snapshot_row` returned `Undetermined`, so not one cell was read.
        format!(
            "This tracker renders a snapshot in its body and its `{entry_id}` row is there, \
             but this check reads IDS ONLY — it has not compared a single cell, so whether \
             the row still shows the previous values is UNVERIFIED. If the fields you \
             patched appear in the rendered columns, update the row via \
             doc(action=\"update\", patch={{body_edits: [...]}}). To stop being guessed at: \
             declare `snapshot_anchor` in this artifact's frontmatter, and the next write \
             re-renders the row and answers this instead of warning about it."
        )
    } else {
        // ABSENCE is a genuine id-level observation, so this branch keeps its
        // assertion — the row is not in the body, and no cell reading is needed to
        // know it.
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
    let body = crate::librarian::read_door::for_catalog(cat)
        .text(std::path::Path::new(&abs_path))
        .ok()?;
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
    /// Whether a [`PendingSection`] was supplied and written in the same call.
    ///
    /// The mirror of [`AllocateOutcome::section_written`], and load-bearing for the same
    /// reason: `false` means the entry is the caller's to write and the hint must say so,
    /// `true` means it is already on disk with a `def_re`-conformant heading and the caller
    /// must NOT write it again. Before the params path could take a section, this was
    /// unconditionally the second case's opposite — the caller was told to write a section
    /// whether or not they had asked the server to.
    pub section_written: bool,
    /// Whether the section this call carried was ALREADY in the file (a failed earlier call
    /// wrote it and persisted no id), so this call recorded that section's id and wrote no
    /// second one. `section_written` is `true` then: the section is on disk and the caller
    /// must not write it again. See [`dual_write::find_unrecorded_section`].
    pub adopted: bool,
    /// The citations written into the entry's section, as grammar text (`[[key]]`,
    /// `[[key:TOKEN]]`, `[[repo/key…]]`), in input order with repeats dropped. Empty when the
    /// call carried no `cites` or wrote no section.
    pub cites: Vec<String>,
    /// Set when the ledger's own file name is not a canonical key: the entry is written and its
    /// bare token resolves inside the ledger, but `[[key:TOKEN]]` cannot name it from another
    /// document until the file is renamed.
    pub key_note: Option<String>,
}

/// Atomically assigns the next `<id_prefix>-N` id and appends `entry` to
/// `params.<entry_collection>`. Runs inside a single `IMMEDIATE` transaction
/// so the read-max-write is safe under both intra-process and cross-process
/// concurrency (paired with `busy_timeout` set on the connection).
///
/// With a `section` the entry is also written into the markdown: the number is
/// `max(params, body, committed mark) + 1`, the `## <ID> — <title>` section and the ledger's
/// high-water mark go to disk in one write (`write_mark: true`), and a ledger with no
/// frontmatter block to hold the mark is refused with nothing written. Without one the call
/// writes params only; the tool refuses that shape, so only tests and Plan D's export reach it.
///
/// With a section, each of `cites` is also written into it as grammar text, one `**Cites:**`
/// line at the end of the section ([`resolve_cites`]); a cite the grammar cannot name (no git
/// root, outside the key space, a key two files share, a repository with no committed name)
/// refuses the whole call before an id is chosen. The `entry_cite` `origin='write'` rows are
/// written either way, and an adoption compares against the section as rendered with that line.
pub fn append_entry(
    cat: &mut Catalog,
    artifact_id: &str,
    entry_collection: &str,
    id_prefix: &str,
    mut entry: Value,
    cites: &[String],
    section: Option<&PendingSection>,
) -> Result<AppendOutcome> {
    let roots = cat.write_roots();
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
        return Err(LibrarianRecoverableError::new(format!(
            "append_entry: artifact `{artifact_id}` has no augmentation — augment it with an entry_collection first"
        )));
    };

    if declared_collection.as_deref() != Some(entry_collection) {
        return Err(LibrarianRecoverableError::with_hint(
            format!("append_entry: `{entry_collection}` is not this artifact's entry_collection"),
            match declared_collection {
                Some(c) => format!("This artifact's entry_collection is `{c}` — pass that instead."),
                None => "This artifact has no entry_collection declared — set one via doc(action=\"augment\") first.".to_string(),
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
    // `as_deref` rather than a move: `abs_path` is needed again below to write the
    // section, and consuming it here is what made a section write impossible to add
    // without restructuring.
    let body_text = abs_path.as_deref().and_then(|p| {
        crate::librarian::read_door::ReadDoor::new(roots.clone())
            .text(std::path::Path::new(p))
            .ok()
    });
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
        .map(|b| snapshot_rows_in_declared_block(b, id_prefix))
        .unwrap_or_default();

    let params_next = next_index(&existing_ids, id_prefix);
    // A call that carries a section reads the ledger through the WRITE door now, inside the
    // transaction and before an id is chosen, because a retry of a call that failed after its
    // section reached disk must find that section instead of numbering a second one. The same
    // text is the one the section is spliced into below, so the refusal is the door's own
    // (a link, a path outside the roots) and fails the call with nothing written.
    let section_doc: Option<String> = match (section, abs_path.as_deref()) {
        (Some(_), Some(path)) => Some(
            crate::librarian::read_door::ReadDoor::new(roots.clone())
                .text_for_write(std::path::Path::new(path))
                .map_err(|e| match e {
                    crate::librarian::read_door::ReadError::Refused(r) => r,
                    crate::librarian::read_door::ReadError::Io(e) => {
                        LibrarianRecoverableError::new(format!(
                            "append_entry: cannot read `{path}` to write the section: {e} — no id was \
                             allocated and nothing was written"
                        ))
                    }
                })?,
        ),
        _ => None,
    };
    // Every cite is resolved, and rendered as grammar text, BEFORE an id is chosen and before
    // anything is written: a cite the grammar cannot name refuses the whole call with the file and
    // the catalog untouched (C1-R6, C1-R15). The section the call writes is the caller's with its
    // `**Cites:**` line, so that rendered section is also what a retry is matched against.
    let resolved = resolve_cites(&tx, abs_path.as_deref(), cites, section.is_some())?;
    let rendered: Option<PendingSection> = section.map(|s| with_cites(s, &resolved.forms));
    // "Recorded" on this path is exact: params hold `<P>-n`. A section the file has and params
    // lack is the orphan of a failed call (C1-R3, C1-R4).
    let adopted_n: Option<u64> = match (rendered.as_ref(), section_doc.as_deref()) {
        (Some(s), Some(doc)) => find_unrecorded_section(doc, id_prefix, &s.title, &s.body, &|n| {
            let token = format!("{id_prefix}-{n}");
            existing_ids.contains(&token)
        }),
        _ => None,
    };
    let adopted = adopted_n.is_some();
    // The COMMITTED mark counts only for a call that writes a section: that call also writes the
    // mark (`write_mark: true` below), so the number it takes must lie above every number the
    // committed text already holds. A call with no section (library callers, Plan D's export)
    // writes no file and takes the params and body numbers as before.
    let committed_mark: Option<u64> = section_doc
        .as_deref()
        .and_then(|doc| frontmatter_mark(doc, id_prefix));
    let next = adopted_n.unwrap_or_else(|| {
        params_next
            .max(body_max.map_or(0, |m| m + 1))
            .max(committed_mark.map_or(0, |m| m + 1))
    });
    let new_id = format!("{id_prefix}-{next}");

    // Skipping the collision is only half the repair — params is still missing
    // the rows the body already documents, and nothing else would ever say so.
    // Not for an adopted section: nothing was renumbered, so there is no collision to name.
    let warning = body_max
        .filter(|m| m + 1 > params_next && !adopted)
        .map(|m| {
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
            LibrarianRecoverableError::new(format!(
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

    if !resolved.refs.is_empty() {
        let slug = crate::librarian::catalog::artifact::ensure_slug(&tx, artifact_id)?;
        let now = chrono::Utc::now().timestamp_millis();
        for dst_ref in resolved.refs {
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
    // The section and its index row, written BEFORE the commit and in that order for the
    // reason `allocate_entry_id` writes before committing: if the splice fails, the
    // transaction rolls back, the id is not consumed, and the refusal's "nothing was
    // written" is true. This is deliberately the ONE body read on this path that may fail
    // the call — `undefined_in_body_note` below still may not, because it runs after a
    // write that already succeeded, and the two are different obligations.
    //
    // Before this existed the five section fields were accepted here and never read: the
    // code that honours them lived inside the prose branch that excluded this one
    // (`docs/issues/archive/2026-09-12-append-entry-drops-section-and-index-row-on-the-params-path.md`).
    let mut snapshot_missing = snapshot_missing;
    // What the section write put on disk, kept so a FAILED COMMIT can be compensated: the
    // transaction rolls back and the id is never persisted, but these bytes are already
    // written.
    let mut written_section: Option<FileWrite> = None;
    let section_written = match rendered.as_ref() {
        None => false,
        Some(s) => {
            let (Some(path), Some(doc)) = (abs_path.as_deref(), section_doc.as_deref()) else {
                return Err(LibrarianRecoverableError::new(format!(
                    "append_entry: artifact `{artifact_id}` has no file on disk, so the \
                     section cannot be written — no id was allocated"
                )));
            };
            // The params path folds the committed mark into its write like the prose path
            // does (`write_mark: true`), so the entry is in the markdown AND under the
            // committed high-water mark, which is what survives a clone. A document with no
            // frontmatter block is refused by the write with nothing on disk and the
            // transaction still open.
            //
            // An adopted section is already in the file, so only the mark can be behind it: a
            // mark-only write when it is below the adopted id (a failed commit then says the
            // retry is safe, no section having been written by this call), and no write when
            // the mark already covers it.
            if !adopted || committed_mark.is_none_or(|m| m < next) {
                written_section = Some(write_entry_into_ledger(
                    &tx,
                    &roots,
                    artifact_id,
                    std::path::Path::new(path),
                    doc,
                    id_prefix,
                    next,
                    if adopted { None } else { Some(s) },
                    true,
                    "append_entry",
                )?);
            }
            // `snapshot_missing` was derived from a body read taken BEFORE this write, so
            // it still lists the id whose row was just added. Reporting it would ask the
            // caller to do by hand the exact thing this call just did — the defect this
            // change exists to remove, re-introduced one field over. An adopted section added
            // no row in this call, so the body read already says whether it has one.
            if s.index_row.is_some() && !adopted {
                snapshot_missing.retain(|m| m != &new_id);
            }
            true
        }
    };

    // NOT a bare `tx.commit()?`: see `commit_after_file_write` for what the commit does when it
    // fails after the section is already on disk. `Adopts`: params' "recorded" test is exact
    // (params hold the id), so a retry finds the section a failed commit left behind.
    commit_after_file_write(
        tx,
        written_section.as_ref(),
        &new_id,
        "append_entry",
        RetryRule::Adopts,
    )?;
    // After the commit, deliberately: this reads the body off disk and must never be
    // able to fail a write that already succeeded.
    let undefined_in_body = undefined_in_body_note(cat, artifact_id, &new_id);
    let key_note = abs_path
        .as_deref()
        .filter(|_| section_written)
        .and_then(|path| ledger_key_note(path, &new_id));
    Ok(AppendOutcome {
        id: new_id,
        warning,
        snapshot_missing,
        undefined_in_body,
        section_written,
        adopted,
        cites: resolved.forms,
        key_note,
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
    /// Whether the section this call carried was ALREADY in the file (a failed earlier call
    /// wrote it and persisted no id), so this call recorded that section's id and wrote no
    /// second one. `section_written` is `true` then: the section is on disk and the caller
    /// must not write it again. See [`dual_write::find_unrecorded_section`].
    pub adopted: bool,
    /// Each cite of the call as the grammar text written into the section, in input order with
    /// exact repeats dropped. Empty when the call carried no cites.
    pub cites: Vec<String>,
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
/// A row to write into the ledger's index table, in the SAME file write as the
/// section. Direction (3) of
/// `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`:
/// the two-call protocol guarantees an interval in which the ledger holds an entry
/// no row names, and no discipline available to the caller closes it — writing the
/// row first is forbidden, because the allocator counts a row as a claimed id.
///
/// One struct rather than two `Option`s on [`PendingSection`], so "a row with no
/// anchor" is unrepresentable instead of refused at runtime.
#[derive(Debug, Clone)]
pub struct PendingIndexRow {
    /// The row text, with `{id}` substituted for the allocated id. A template
    /// rather than a literal because the caller cannot know the id before the
    /// call — that is the same reason the heading is formatted here and not by
    /// the caller.
    pub row: String,
    /// Existing line to insert the row immediately AFTER. Explicit, never
    /// inferred: for a newest-first table this is the separator (`|---|---|`).
    /// Same law as [`PendingSection::anchor_heading`]
    /// (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`) \u2014 a wrong
    /// guess about placement on a WRITE needs manual repair.
    pub after_line: RowAnchor,
}

/// Where a [`PendingIndexRow`] is to be placed.
///
/// **Both variants name an EXPLICIT target; neither infers one.** That distinction is
/// the whole reason this is an enum rather than `Option<String>`:
/// `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` § *The boundary*
/// permits accepting an explicit write target and forbids guessing one, and
/// `None` would have re-admitted exactly the state [`PendingIndexRow`]'s own doc
/// says must stay unrepresentable — a row with no anchor at all.
#[derive(Debug, Clone)]
pub enum RowAnchor {
    /// The caller named the line verbatim, per call.
    Explicit(String),
    /// The caller deferred to the ARTIFACT's own [`SNAPSHOT_ANCHOR_KEY`] frontmatter
    /// declaration — still an explicit target, named once by the author in a file
    /// that travels with the repo instead of once per call.
    ///
    /// Resolution happens in [`splice_pending_section`], which already holds the whole
    /// document (frontmatter included) inside the caller's transaction, so the anchor
    /// is read from the same bytes the row is spliced into. Resolving it earlier, at
    /// the tool boundary, would read the file a second time and could observe a
    /// different version than the one written.
    ///
    /// Refuses rather than falling back when the artifact declares no anchor, or when
    /// the declaration matches no line or several: a row placed in a guessed table is
    /// the defect this mechanism exists to prevent, and a wrong guess on a write needs
    /// manual repair.
    Declared,
}

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
    /// Optional index-table row, written in the same `fs::write` as the section.
    /// `None` leaves the ledger's table alone, which is correct for the 28 of 49
    /// guarded ledgers that keep no row table (measured 2026-09-07).
    pub index_row: Option<PendingIndexRow>,
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
///
/// Only CITABLE prefixes count ([`crate::util::librarian_guard::is_citable_entry_prefix`]):
/// a declaration the token grammar cannot express declares nothing, here exactly as in the
/// guard. Every write path refuses to create one ([`refuse_taken_prefixes`] reads the raw
/// value for that), so a surviving one was hand-written, and `append_entry` names the bound
/// when it meets it.
pub(crate) fn declared_prefixes_from_frontmatter(
    fm: Option<&crate::librarian::frontmatter::Frontmatter>,
) -> Vec<String> {
    raw_declared_prefixes(fm)
        .into_iter()
        .filter(|p| crate::util::librarian_guard::is_citable_entry_prefix(p))
        .collect()
}

/// Every non-empty `entry_prefix` value as written, citable or not — what a declaration
/// SAYS, for the paths that must see a bad one in order to refuse or report it.
pub(crate) fn raw_declared_prefixes(
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

/// Frontmatter key declaring an id namespace owned OUTSIDE the markdown corpus — a
/// benchmark suite, a JSON fixture, a code constant — as a map from prefix to the
/// repo-relative file that owns it: `external_prefix: {TC: scripts/tc-suites/x.json}`.
///
/// Read by `doctor`'s `cited_prefix_with_no_definer`, which stays silent for a declared
/// prefix only while that file still holds a `PREFIX-<n>` id, and by
/// [`refuse_taken_prefixes`], for which an external namespace is as owned as a ledger's.
/// Deliberately NOT read by `link_scan`: a resolver that learned the prefix would report
/// every citation of it dangling.
/// docs/issues/archive/2026-09-04-a-namespace-owned-outside-the-corpus-cannot-declare-itself.md
pub const EXTERNAL_PREFIX_KEY: &str = "external_prefix";

/// The `external_prefix` declarations in a parsed frontmatter block, as `(prefix,
/// authority)` pairs. A non-map value, or an entry whose value is not a string, yields
/// nothing for that entry — which leaves every consumer in its conservative state
/// (doctor keeps firing; the uniqueness check simply sees no external owner).
pub(crate) fn declared_external_prefixes(
    fm: Option<&crate::librarian::frontmatter::Frontmatter>,
) -> Vec<(String, String)> {
    let Some(Value::Object(map)) = fm.and_then(|f| f.extra.get(EXTERNAL_PREFIX_KEY)) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(prefix, v)| Some((prefix.trim().to_string(), v.as_str()?.trim().to_string())))
        .filter(|(p, a)| !p.is_empty() && !a.is_empty())
        .collect()
}

/// Prefixes MANY ledgers may declare at once, and the only ones.
///
/// `docs/templates/session-log.md` gives every per-work-stream log its own `F-N` frictions
/// and `W-N` wins, and those logs are cited QUALIFIED by file stem
/// (`bug-fix-session-log:F-97`), which is what keeps fifteen `F-7`s apart. Refusing a second
/// declaration of `F` would break the template every reconnaissance run copies. Every other
/// prefix has exactly one owner per repository — see [`refuse_taken_prefixes`].
///
/// A list rather than a rule derived from the corpus on purpose: the exemption is a decision
/// about ONE family, and a derived rule ("any prefix several ledgers already share") would
/// bless the next accidental collision the moment it happened twice.
pub const SHARED_ENTRY_PREFIXES: &[&str] = &["F", "W"];

/// Every prefix an `extra` frontmatter map claims — `entry_prefix` and `external_prefix`
/// keys alike. The single definition of "a claim", shared by [`prefix_owners_under`] (reading
/// files) and the write paths (reading a caller's `extra` before anything is written), so the
/// guard and the scan it consults cannot disagree about what a declaration says.
///
/// RAW `entry_prefix` values, uncitable ones included: a claim of `DCTX` must reach
/// [`refuse_taken_prefixes`] to be refused, and filtering it here would wave it through.
pub(crate) fn claimed_prefixes(extra: &std::collections::BTreeMap<String, Value>) -> Vec<String> {
    let fm = crate::librarian::frontmatter::Frontmatter {
        extra: extra.clone(),
        ..Default::default()
    };
    raw_declared_prefixes(Some(&fm))
        .into_iter()
        .chain(
            declared_external_prefixes(Some(&fm))
                .into_iter()
                .map(|(p, _)| p),
        )
        .collect()
}

/// Every prefix declared by an artifact under `root`, mapped to the declaring paths.
///
/// Both declarations count as ownership — `entry_prefix` (a ledger allocating ids) and
/// `external_prefix` (a namespace whose ids live outside markdown). Status is ignored, and
/// that is load-bearing: an ARCHIVED ledger still owns its namespace, because citations of
/// its entries still resolve to it, and a new ledger reusing the prefix would capture them
/// as silent wrong edges — `T-1`…`T-12` bound ~65 citations of three retired ledgers into a
/// new one that way (docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md).
pub(crate) fn prefix_owners_under(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, Vec<String>>> {
    let mut stmt = conn.prepare("SELECT abs_path FROM artifact ORDER BY abs_path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut owners: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let door = crate::librarian::read_door::ReadDoor::new(vec![root.to_path_buf()]);
    for path in paths {
        if !std::path::Path::new(&path).starts_with(root) {
            continue;
        }
        let Ok(text) = door.text(std::path::Path::new(&path)) else {
            continue;
        };
        // Cheap pre-filter: nearly every artifact declares neither key, and a YAML parse per
        // file is the whole cost of this scan.
        if !text.contains(ENTRY_PREFIX_KEY) && !text.contains(EXTERNAL_PREFIX_KEY) {
            continue;
        }
        let fm = crate::librarian::frontmatter::parse(&text)
            .ok()
            .and_then(|(fm, _)| fm);
        let Some(fm) = fm else { continue };
        for prefix in claimed_prefixes(&fm.extra) {
            let list = owners.entry(prefix).or_default();
            if !list.contains(&path) {
                list.push(path.clone());
            }
        }
    }
    Ok(owners)
}

/// Refuse to let `declaring` claim any of `prefixes` that another artifact in the same
/// repository already owns, naming the owner and offering free alternatives.
///
/// The collision this closes is not a small-namespace problem — 37 prefixes are in use out
/// of ~18,000 expressible — it is two authors picking the same mnemonic (`T` for tool, test
/// and task), which nothing checked. Called at every WRITE path that can declare a prefix:
/// `doc(create)`, `doc(update)` and `rekey_prefix`. A hand-edited frontmatter bypasses all
/// three, which is what `doctor`'s `entry_prefix_declared_twice` is for.
///
/// Scoped to `declaring`'s git root, falling back to its directory outside a repository. A
/// catalog that has not indexed a sibling ledger yet cannot see it — a false ACCEPT, the
/// permissive direction, which `doctor` catches after the next reindex. A linked WORKTREE is
/// its own git root here and cannot see the main checkout's ledgers either; same direction,
/// caught by `doctor` once `merge_worktree` brings the ledger home.
pub(crate) fn refuse_taken_prefixes(
    conn: &rusqlite::Connection,
    declaring: &std::path::Path,
    prefixes: &[String],
) -> Result<()> {
    let uncitable: Vec<&String> = prefixes
        .iter()
        .filter(|p| !crate::util::librarian_guard::is_citable_entry_prefix(p))
        .collect();
    let claimed: Vec<&String> = prefixes
        .iter()
        .filter(|p| !SHARED_ENTRY_PREFIXES.contains(&p.as_str()))
        .collect();
    // `claimed` only drops the shared family, which is citable, so an empty `claimed` already
    // means an empty `uncitable` — one test covers both.
    if claimed.is_empty() {
        return Ok(());
    }
    let dir = declaring.parent().unwrap_or(std::path::Path::new("."));
    let root = crate::librarian::current_project::lookup_git_root(dir)
        .unwrap_or_else(|| dir.to_path_buf());
    let owners = prefix_owners_under(conn, &root)?;
    // The catalog's WRITE normalization, so any spelling a caller passes compares equal to
    // the stored row. Raw `to_string_lossy` was native on Windows (`C:\…\docs/x.md`)
    // against an all-forward-slash `abs_path`, and refused an owner as a conflict with
    // itself. docs/issues/archive/2026-09-24-prefix-owner-check-compares-path-spellings-as-text.md
    let me = crate::util::fs::RepoPath::from(declaring);

    // Refused before ownership, because a free-but-uncitable prefix is the worse outcome: a
    // taken one at least collides where a scan can see it, while this one allocates, commits
    // and is invisible to every citation reader in both directions.
    if !uncitable.is_empty() {
        let described = uncitable
            .iter()
            .map(|p| format!("`{p}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let suggestions = uncitable
            .iter()
            .map(|p| {
                let letters: String = p
                    .chars()
                    .filter(char::is_ascii_alphabetic)
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
                let free = free_prefix_suggestions(&letters, &owners);
                format!(
                    "instead of `{p}`: {}",
                    free.iter()
                        .map(|s| format!("`{s}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(
            crate::librarian::tools::LibrarianRecoverableError::with_hint(
                format!(
                    "entry prefix {described} cannot be cited — an entry token is \
                     `[A-Z]{{1,3}}-<n>`, so an id under a longer or non-uppercase prefix \
                     allocates and is written, but no citation can address it and no scan \
                     reports it missing."
                ),
                format!(
                    "Pick one to three uppercase letters — {suggestions} (all free in this \
                     repository). Nothing has been written."
                ),
            ),
        );
    }

    let taken: Vec<(&String, Vec<&String>)> = claimed
        .into_iter()
        .filter_map(|p| {
            let others: Vec<&String> = owners
                .get(p)?
                .iter()
                .filter(|o| o.as_str() != me.as_str())
                .collect();
            (!others.is_empty()).then_some((p, others))
        })
        .collect();
    if taken.is_empty() {
        return Ok(());
    }

    let described = taken
        .iter()
        .map(|(p, others)| {
            let others = others
                .iter()
                .map(|o| o.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("`{p}` is owned by {others}")
        })
        .collect::<Vec<_>>()
        .join("; ");
    let suggestions = taken
        .iter()
        .map(|(p, _)| {
            let free = free_prefix_suggestions(p, &owners);
            format!(
                "instead of `{p}`: {}",
                free.iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(
        crate::librarian::tools::LibrarianRecoverableError::with_hint(
            format!(
                "entry prefix already taken in this repository — {described}. A prefix has one \
             owner per repository, because a bare `PREFIX-N` citation resolves to whichever \
             ledger defines it, and two owners bind each other's citations silently."
            ),
            format!(
                "Pick a free prefix — {suggestions} (checked against every declaration here, \
             archived ledgers included, since their citations still resolve). If this is the \
             SAME namespace continued, extend the existing ledger instead of declaring a \
             second one. `F` and `W` are the only shared family (session logs, cited \
             qualified by file stem). Nothing has been written."
            ),
        ),
    )
}

/// Up to three free prefixes near `requested`, for a refusal to hand the caller a next move
/// it can take rather than a bare "taken". Longer candidates first extend the prefix
/// (`R` -> `RA`, `RB`, ...), a three-letter one varies its last letter; the grammar is
/// `[A-Z]{1,3}`, so nothing longer is ever offered, and the shared family is never offered.
fn free_prefix_suggestions(
    requested: &str,
    owners: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let stem = if requested.len() < 3 {
        requested.to_string()
    } else {
        requested[..2].to_string()
    };
    ('A'..='Z')
        .map(|c| format!("{stem}{c}"))
        .filter(|c| {
            c != requested
                && !owners.contains_key(c)
                && !SHARED_ENTRY_PREFIXES.contains(&c.as_str())
        })
        .take(3)
        .collect()
}

/// Frontmatter key by which a ledger declares WHERE its rendered snapshot block
/// lives in the body: that block's header line, verbatim.
///
/// **Lives in frontmatter for exactly [`ENTRY_PREFIX_KEY`]'s reason** — the catalog
/// is machine-local and git-ignored, so an augmentation-column declaration is absent
/// in a fresh clone (HY-10), and the block would silently stop being locatable on
/// every machine but the authoring one. The cost signal was concrete: putting it on
/// [`AugmentationRow`] meant editing 58 construction sites across 21 files to add one
/// optional declaration, which is the struct saying the fact does not belong to it.
///
/// **Declared, never derived, and that is the whole point.** Locating the block by
/// matching `render_template`'s static header instead would be auto-*guessing* a
/// write target, which `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`
/// § *The boundary* forbids: an absent `index_after_line` is absent input, not
/// malformed input, and that ADR reserves the teaching error for absent. The
/// derivation is also only safe because the header happens to be unique in today's
/// bodies — a property of the corpus rather than of the scheme, and the very
/// collision
/// `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`
/// reports.
///
/// A **scalar** key rather than a nested map, for [`ENTRY_HIGH_WATER_PREFIX`]'s
/// reason: the surgical frontmatter writers operate on single `key: value` lines,
/// and re-emitting the whole block to hold a map reformats hand-authored files
/// (BL-34).
pub const SNAPSHOT_ANCHOR_KEY: &str = "snapshot_anchor";

/// The snapshot block's header line as the artifact itself declares it, or `None`
/// when it declares none.
///
/// `None` is the CORRECT state for most trackers, not a gap to fill. Measured
/// 2026-09-13: of the 15 codescout artifacts carrying both a `render_template` and
/// an `entry_collection`, only **3** render a table into the body at all
/// (`open-issue-work-queue`, `legibility-backlog`,
/// `2026-08-16-iron-law-gate-firing-audit`). The other 12 project their template
/// into `librarian(action="context")` and hold no block to anchor — writing one into
/// them would inject a table their author never had.
///
/// Mirrors [`declared_prefixes_from_frontmatter`] deliberately, down to reading
/// `extra` rather than a typed field, so a caller declares this with the same
/// `doc(action="update", patch={extra: …})` surface that already exists.
pub(crate) fn declared_snapshot_anchor(
    fm: Option<&crate::librarian::frontmatter::Frontmatter>,
) -> Option<String> {
    match fm.and_then(|f| f.extra.get(SNAPSHOT_ANCHOR_KEY)) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

/// The inclusive line range `(header, last)` of the contiguous `|`-anchored block
/// headed by `anchor`.
///
/// **Refuses rather than guesses, in both directions.** `None` when the anchor
/// matches no line (the declaration has drifted from the body) and `None` when it
/// matches more than one (the declaration does not identify a block). Silently
/// picking the first match is the defect this whole mechanism exists to avoid: it is
/// what `body_snapshot_row_indices` does on its own, and why a row in an unrelated
/// table counts as a snapshot row.
///
/// Walking **down** from a known header is the mirror of `doctor`'s
/// `table_is_about_status`, which walks **up** from a known data row to find its
/// header. Same contiguity rule, opposite direction.
///
/// **Returns the range because two consumers want different halves of one walk**, and
/// deriving either from the other reintroduces the ambiguity this module exists to
/// end: [`snapshot_block_last_line`] wants the tail line's TEXT, since
/// `insert_index_row` addresses by line text; [`snapshot_rows_in_declared_block`]
/// wants the bounds to scan between. Recovering bounds from the text means
/// `rposition`-ing a line that a second table may repeat verbatim.
pub(crate) fn snapshot_block_range(doc: &str, anchor: &str) -> Option<(usize, usize)> {
    let needle = anchor.trim();
    if needle.is_empty() {
        return None;
    }
    let lines: Vec<&str> = doc.lines().collect();
    let mut header: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == needle {
            if header.is_some() {
                // Ambiguous: two lines answer to this anchor, so it names no single
                // block. Refusing is the contract — see the doc comment.
                return None;
            }
            header = Some(i);
        }
    }
    let start = header?;
    // Contiguous `|` run below the header: separator row, then data rows. Stops at
    // the first non-table line, so a second table further down the file is out of
    // reach by construction rather than by luck.
    let mut last = start;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim_start().starts_with('|') {
            last = i;
        } else {
            break;
        }
    }
    Some((start, last))
}

/// The last line of the contiguous `|`-anchored block headed by `anchor` — the line
/// a new row must be inserted AFTER to land at the block's tail.
///
/// Returns the line VERBATIM (minus its newline, as `str::lines` yields it) because
/// `insert_index_row` compares trimmed — a caller that reconstructed the text would
/// risk a whitespace mismatch against the very line it just read.
///
/// A projection of [`snapshot_block_range`]; every refusal case is that function's,
/// and the tests below exercise the walk through this wrapper.
pub(crate) fn snapshot_block_last_line(doc: &str, anchor: &str) -> Option<String> {
    let (_, last) = snapshot_block_range(doc, anchor)?;
    doc.lines().nth(last).map(str::to_string)
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
///
/// **`cites`** are written only with a `section`: each is resolved ([`resolve_cites`], the params
/// path's resolver) inside the transaction before the reservation is staged, and the section is
/// written with one `**Cites:**` line at its end; the `entry_cite` `origin='write'` rows are
/// inserted beside the reservation. `cites` without a section are refused with nothing reserved,
/// and a cite the grammar cannot name refuses the whole call. A retry adopts an orphan section
/// only when its title, body and cites all match: the adoption key is the section as rendered.
pub fn allocate_entry_id_with_cites(
    cat: &mut Catalog,
    artifact_id: &str,
    id_prefix: &str,
    section: Option<&PendingSection>,
    cites: &[String],
) -> Result<AllocateOutcome> {
    // `cites` have no place to land without a section: the entry's text is the section, and a
    // cite with no text would exist only as a catalog row (spec requirement 3). Refused before the
    // transaction opens, so nothing is read, reserved or written.
    if !cites.is_empty() && section.is_none() {
        return Err(LibrarianRecoverableError::with_hint(
            "allocate_entry_id: `cites` needs a section to be written into".to_string(),
            "Pass `title`, `body` and `anchor_heading` together with `cites`: the server writes the \
             citations as a `**Cites:**` line at the end of the section. Nothing has been \
             reserved."
                .to_string(),
        ));
    }
    let roots = cat.write_roots();
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
        return Err(LibrarianRecoverableError::new(format!(
            "allocate_entry_id: unknown artifact `{artifact_id}`"
        )));
    };
    let doc = crate::librarian::read_door::ReadDoor::new(roots.clone())
        .text_for_write(std::path::Path::new(&abs_path))
        .map_err(|e| match e {
            crate::librarian::read_door::ReadError::Refused(r) => r,
            crate::librarian::read_door::ReadError::Io(e) => {
                LibrarianRecoverableError::new(format!(
                    "allocate_entry_id: cannot read `{abs_path}`: {e} — the ledger's own body is \
                 where the id maximum is derived from"
                ))
            }
        })?;

    // One read, three answers: the declaration, the body maximum, and (via the
    // body) the durability check the caller needs.
    let (fm, body) = crate::librarian::frontmatter::parse(&doc)
        .map_err(|e| LibrarianRecoverableError::new(format!("allocate_entry_id: {e}")))?;
    // Scalar or sequence: a session log legitimately owns two namespaces (F-N
    // frictions and W-N wins), so `entry_prefix: [F, W]` must be as valid as
    // `entry_prefix: R`. Reservations are keyed per (artifact, prefix), so the
    // counters stay independent either way.
    let declared = declared_prefixes_from_frontmatter(fm.as_ref());

    if declared.is_empty() {
        return Err(LibrarianRecoverableError::with_hint(
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
        return Err(LibrarianRecoverableError::with_hint(
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
    let frontmatter_max: Option<u64> =
        crate::librarian::catalog::dual_write::frontmatter_mark(&doc, id_prefix);

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

    // A section already in the file under this title, body and cites, whose id the reservation table
    // does not cover, is the orphan of a failed earlier call: that call wrote the file, then lost
    // the catalog COMMIT, so its id was never persisted. Adopt it instead of numbering a second
    // section (C1-R3). "Recorded" here is ONE per-prefix maximum, so a peer that reserved a
    // higher id after our failed commit makes the orphan look recorded and it is not adopted;
    // the commit below keeps `RetryRule::DeleteFirst` for exactly that case (C1-R22).
    //
    // Every cite is resolved and rendered as grammar text BEFORE the reservation is staged and
    // before anything is written ([`resolve_cites`], the params path's own resolver): a cite the
    // grammar cannot name refuses the call, the transaction drops, and no reservation, section,
    // mark or `entry_cite` row exists. The adoption key is the section AS RENDERED with its
    // `**Cites:**` line, so a retry with the same title, body and cites adopts, and different
    // cites are a new entry.
    let resolved = resolve_cites(&tx, Some(&abs_path), cites, section.is_some())?;
    let rendered: Option<PendingSection> = section.map(|s| with_cites(s, &resolved.forms));
    let adopted_n: Option<u64> = rendered.as_ref().and_then(|s| {
        find_unrecorded_section(&doc, id_prefix, &s.title, &s.body, &|n| {
            reserved_max.is_some_and(|r| n <= r)
        })
    });
    let adopted = adopted_n.is_some();

    // Max of all THREE, so no single input can walk the counter backwards. That is
    // the whole property: each source is unreliable in a different way, and none of
    // them is ever wrong in the *high* direction.
    let next = adopted_n.unwrap_or_else(|| {
        body_max
            .map_or(0, |m| m + 1)
            .max(reserved_max.map_or(0, |m| m + 1))
            .max(frontmatter_max.map_or(0, |m| m + 1))
            .max(1)
    });

    tx.execute(
        "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(artifact_id, prefix) DO UPDATE SET
           max_allocated = excluded.max_allocated,
           updated_at    = excluded.updated_at",
        rusqlite::params![artifact_id, id_prefix, next as i64],
    )?;
    // The same `origin='write'` rows the params path inserts. A prose entry has no params row,
    // but the row is what the old scanner's tables read until Plan D; the scan's
    // `INSERT OR IGNORE` on `(src_slug, src_local, dst_ref, rel)` keeps one row if it runs later.
    if !resolved.refs.is_empty() {
        let slug = crate::librarian::catalog::artifact::ensure_slug(&tx, artifact_id)?;
        let now = chrono::Utc::now().timestamp_millis();
        for dst_ref in &resolved.refs {
            crate::librarian::catalog::entry_cite::insert_with(
                &tx,
                &crate::librarian::catalog::entry_cite::EntryCiteRow {
                    src_slug: slug.clone(),
                    src_local: format!("{id_prefix}-{next}"),
                    dst_ref: dst_ref.clone(),
                    rel: "cites".to_string(),
                    origin: "write".to_string(),
                    created_at: now,
                },
            )?;
        }
    }

    // Persist the mark BEFORE committing, and fail the whole allocation if it cannot
    // be written. The ordering is the durability argument:
    //
    // * write fails → `?` returns, the transaction rolls back, no id was handed out.
    //   Refusing is right: an id whose committed mark did not advance is precisely
    //   the reissue this exists to prevent, so a silent fallback would reintroduce it.
    // * write succeeds, commit fails → the commit goes through `commit_after_file_write`,
    //   which puts the file back (mark included) whenever it still holds our bytes. Only
    //   when that restore cannot run (a peer rewrote the file, or the restore itself
    //   failed) does the mark stay ahead of the database, and that is still safe: `next`
    //   takes the max, so the following call reads the higher mark and simply skips an
    //   integer.
    //
    // Concurrency on one machine is handled by the enclosing IMMEDIATE transaction —
    // a second session blocks on the write lock, so it cannot interleave here.
    //
    // The mark and the entry section are written by `write_entry_into_ledger` in ONE file
    // write; see `PendingSection` for why this cannot be a second write by the caller.
    //
    // The heading is formatted there and nowhere else: `<level> <ID> — <title>` is
    // exactly `link_scan`'s `def_re` (`^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+`), so an entry
    // written through this path cannot be born undefined. Callers used to receive the
    // id plus a hint asking them to format it, and a heading missing its
    // dash-and-title defines no token — the mechanism behind ~30 of 39 sampled
    // dangling tokens in this repo. CAP-5 defect class 2.
    // `observed_level` is an OBSERVATION and stays `None` when the body heads nothing,
    // because the caller phrases its hint differently for a default than for a
    // convention — collapsing it with the formatting default is U-40 in
    // `docs/trackers/codescout-usage-frictions.md`, where a hard-coded `##` told a
    // `###` ledger the wrong level while claiming it was the ledger's own.
    let observed_level = body_entry_heading_level(body, id_prefix);
    let id = format!("{id_prefix}-{next}");
    // An adopted section is already in the file, so only the committed mark can be behind it:
    // it is written when it is below the adopted id (a mark-only write: a failed commit then says
    // the retry is safe, because no section was written by this call), and the file is left
    // alone when the mark already covers it.
    //
    // `abs_path` is the catalog's, and a row can be an in-root link to a file outside the
    // repository; the write refuses a link. Before the commit, so the reservation
    // inserted above rolls back with the transaction.
    let written: Option<FileWrite> = if adopted && frontmatter_max.is_some_and(|m| m >= next) {
        None
    } else {
        Some(write_entry_into_ledger(
            &tx,
            &roots,
            artifact_id,
            std::path::Path::new(&abs_path),
            &doc,
            id_prefix,
            next,
            if adopted { None } else { rendered.as_ref() },
            true,
            "allocate_entry_id",
        )?)
    };

    // A commit that fails after the file write restores the file (when it still holds our
    // bytes) and says which remedy applies, instead of leaving the section behind an id that
    // was never persisted.
    commit_after_file_write(
        tx,
        written.as_ref(),
        &id,
        "allocate_entry_id",
        // An adopted section stays in the file whatever this call wrote, so a failed commit
        // gets the delete-first remedy even when this call wrote nothing or only the mark.
        if adopted {
            RetryRule::AdoptedOrphan
        } else {
            RetryRule::DeleteFirst
        },
    )?;

    Ok(AllocateOutcome {
        id,
        body_max,
        reserved_max,
        frontmatter_max,
        heading_level: observed_level,
        section_written: section.is_some(),
        adopted,
        cites: resolved.forms,
    })
}
/// [`allocate_entry_id_with_cites`] for a caller with no cites: the same call with `&[]`, so the
/// allocator's many callers keep their signature.
pub fn allocate_entry_id(
    cat: &mut Catalog,
    artifact_id: &str,
    id_prefix: &str,
    section: Option<&PendingSection>,
) -> Result<AllocateOutcome> {
    allocate_entry_id_with_cites(cat, artifact_id, id_prefix, section, &[])
}

/// Insert `row` immediately after the first line equal to `after_line`, comparing
/// both trimmed so a caller need not reproduce trailing whitespace.
///
/// Errors when the anchor is absent rather than appending somewhere plausible: a
/// write accepts an explicit target and never infers one, and a missed anchor must
/// abort BEFORE anything is written
/// (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`). Returns the whole
/// document so the caller splices it into the same `fs::write` as everything else.
fn insert_index_row(doc: &str, after_line: &str, row: &str) -> std::result::Result<String, String> {
    let needle = after_line.trim();
    let mut out = String::with_capacity(doc.len() + row.len() + 1);
    let mut placed = false;
    for line in doc.split_inclusive('\n') {
        out.push_str(line);
        // FIRST match only. A table separator is not unique across a ledger with
        // several tables, and silently filling every one of them would be a worse
        // outcome than refusing; the caller picks the table by naming a line inside it.
        if !placed && line.trim() == needle {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(row);
            out.push('\n');
            placed = true;
        }
    }
    if placed {
        Ok(out)
    } else {
        Err(format!("no line matching `{after_line}`"))
    }
}

/// Turn a [`RowAnchor`] into the literal line [`insert_index_row`] will match against.
///
/// `Explicit` passes through. `Declared` reads the artifact's [`SNAPSHOT_ANCHOR_KEY`]
/// from `doc`'s own frontmatter and walks that block to its tail, so the row lands
/// after the last existing row rather than under the header — which is what a
/// newest-last ledger wants, and what a caller naming the separator by hand cannot
/// express without knowing the current final row.
///
/// **Three distinct refusals, each naming which half is missing**, because they need
/// different repairs and a single "could not place the row" would send the reader to
/// the wrong one: the artifact declares no anchor (author must declare it), the
/// declaration matches nothing (it has drifted from the body), or it matches several
/// (it names no single block). All three abort before any write.
fn resolve_row_anchor(doc: &str, anchor: &RowAnchor, id: &str, caller: &str) -> Result<String> {
    let declared = match anchor {
        RowAnchor::Explicit(line) => return Ok(line.clone()),
        RowAnchor::Declared => {
            let (fm, _) = crate::librarian::frontmatter::parse(doc).unwrap_or((None, doc));
            declared_snapshot_anchor(fm.as_ref()).ok_or_else(|| {
                LibrarianRecoverableError::with_hint(
                    format!(
                        "{caller}: cannot place the index row for {id}: this artifact declares no \
                         `{SNAPSHOT_ANCHOR_KEY}` — no id was allocated and nothing was written"
                    ),
                    format!(
                        "Either pass `index_after_line` explicitly, or declare the block once: \
                         doc(action=\"update\", id=…, patch={{\"extra\": {{\"{SNAPSHOT_ANCHOR_KEY}\": \
                         \"<the table's header line, verbatim>\"}}}}). Most trackers should declare \
                         NONE — a body that renders no snapshot table has no block to anchor."
                    ),
                )
            })?
        }
    };
    snapshot_block_last_line(doc, &declared).ok_or_else(|| {
        // Which of the two failures it was is worth the extra scan: "matches nothing" and
        // "matches several" look identical from a bare None and have opposite repairs.
        let hits = doc.lines().filter(|l| l.trim() == declared.trim()).count();
        let (why, fix) = if hits == 0 {
            (
                "matches no line in the body",
                "The declaration has drifted from the body — update `snapshot_anchor` to the \
                 table's current header line, or restore the header.",
            )
        } else {
            (
                "matches more than one line in the body",
                "The declaration names no single block. Make the snapshot table's header \
                 distinguishable from the other tables that share it, then re-declare.",
            )
        };
        LibrarianRecoverableError::with_hint(
            format!(
                "{caller}: cannot place the index row for {id}: `{SNAPSHOT_ANCHOR_KEY}` \
                 (`{declared}`) {why} — no id was allocated and nothing was written"
            ),
            fix.to_string(),
        )
    })
}

/// What [`resync_snapshot_row`] ESTABLISHED about the committed row, as distinct from
/// what it did.
///
/// It returned `bool` until 2026-09-14, and the `false` conflated two facts that license
/// opposite advice: *"the rendered row and the body row are byte-identical"* and *"I could
/// not check"*. The first is the answer `snapshot_stale_note` needs and cannot compute —
/// that note receives only the id set, never the field values, so it branched on PRESENCE
/// and worded the result as a claim about CONTENT
/// (`docs/issues/archive/2026-09-13-snapshot-stale-tests-row-presence-and-reports-a-content-claim.md`).
///
/// **The comparison was already being made and thrown away**, which is why this is a
/// return type rather than a new render. The bug file superseded by that one priced the
/// honest fix at *"a template evaluation on every entry write"*; on the anchor path that
/// evaluation has already happened by the time this value is produced.
pub(crate) enum SnapshotRow {
    /// The body row differed from the render and was rewritten in place.
    Rewritten,
    /// The body row is byte-identical to what `render_template` produces for this entry.
    /// The body is current — **observed, not inferred** — so there is nothing to advise.
    AlreadyCurrent,
    /// Nothing was established: no `render_template`, no declared anchor, no rendered row
    /// for this id, or the block could not be located. The advisory is the fallback, and
    /// must not claim more than id presence supports.
    Undetermined,
}

/// Re-render ONE entry's row in the committed snapshot block, so a params write and
/// the table a reader sees stop disagreeing.
///
/// **Per-ROW rather than per-BLOCK, and that is a safety property rather than a
/// scope cut.** Re-rendering the whole block would overwrite every other row from
/// `params` — and measured 2026-09-13 on `open-issue-work-queue.md`, the columns do
/// not share an answer about which side is current: `Status` was stale in params on
/// 25 rows, `Bug` was stale in params on 6 and in the body on 1, and `Task` is fuller
/// in params on 13 rows and in the body on 4. A block re-render picks one winner for
/// all of them and is wrong about at least one column whichever way it picks, while
/// looking like a clean sync. A row re-render touches the one line whose new content
/// the caller just supplied, so the blast radius is exactly the write.
///
/// Returns [`SnapshotRow::Undetermined`] — a silent no-op, not an error — when the
/// artifact declares no [`SNAPSHOT_ANCHOR_KEY`], carries no `render_template`, or renders
/// no row for this id. That is the majority: most trackers keep no body snapshot, and for
/// them this path must not change behaviour at all.
///
/// **[`SnapshotRow::AlreadyCurrent`] is the return that exists for someone else.** Nothing
/// here acts on it — the function's own job is done either way — but it is the only place
/// in the codebase where "the body cell and the params value agree" is actually observed,
/// and `snapshot_stale_note` needs it to avoid asserting the opposite. Collapsing it back
/// into the no-op case is the defect this enum was introduced to remove, so if a future
/// change makes the distinction inconvenient, move it rather than dropping it.
///
/// **Runs INSIDE the caller's transaction, before commit**, matching
/// [`allocate_entry_id`] — so a failed splice rolls the params write back rather than
/// leaving the two halves disagreeing in a new way. That is a deliberate departure
/// from the comment below the call site, which argues the body signal is advisory and
/// "must never be able to fail the mutation the caller asked for": true of a REPORT,
/// and the wrong bargain for a WRITE, where a half-applied pair is worse than a
/// refused one. The departure is bounded by the `Undetermined` cases above — an artifact
/// that has not opted in cannot reach the new failure mode.
fn resync_snapshot_row(
    tx: &rusqlite::Transaction<'_>,
    roots: &[std::path::PathBuf],
    artifact_id: &str,
    entry_id: &str,
    params: &Value,
    render_template: Option<&str>,
) -> Result<SnapshotRow> {
    let Some(tmpl) = render_template else {
        return Ok(SnapshotRow::Undetermined);
    };
    let abs_path: Option<String> = tx
        .query_row(
            "SELECT abs_path FROM artifact WHERE id = ?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(abs_path) = abs_path else {
        return Ok(SnapshotRow::Undetermined);
    };
    // A refusal (the write door's leaf rule, or the read door) fails the call, as the write door
    // did when it refused the resync's WRITE: the transaction rolls back. An unreadable file is
    // undetermined, as before.
    let doc = match crate::librarian::read_door::ReadDoor::new(roots.to_vec())
        .text_for_write(std::path::Path::new(&abs_path))
    {
        Ok(d) => d,
        Err(crate::librarian::read_door::ReadError::Refused(e)) => return Err(e),
        Err(crate::librarian::read_door::ReadError::Io(_)) => return Ok(SnapshotRow::Undetermined),
    };
    let (fm, _) = crate::librarian::frontmatter::parse(&doc).unwrap_or((None, doc.as_str()));
    let Some(anchor) = declared_snapshot_anchor(fm.as_ref()) else {
        return Ok(SnapshotRow::Undetermined);
    };

    // Render the FULL template and pick this id's line out of it, rather than
    // rendering a one-element collection. A template may legitimately compute a cell
    // from the whole collection (a rank, a total, a "first of its status"), and the
    // one-element shortcut would silently produce a different row than the table it
    // is being spliced into.
    let rendered = crate::librarian::tools::render::render_params(tmpl, params).map_err(|e| {
        LibrarianRecoverableError::new(format!(
            "update_entry: `{entry_id}` was patched, but re-rendering the snapshot row failed: \
             {e} — nothing was written"
        ))
    })?;
    let row_re = regex::Regex::new(&format!(r"^\|\s*{}\s*\|", regex::escape(entry_id)))
        .map_err(|e| LibrarianRecoverableError::new(format!("update_entry: {e}")))?;
    let Some(new_row) = rendered.lines().find(|l| row_re.is_match(l)) else {
        // The template emits no row for this id. Not an error: an entry can exist in
        // params and be deliberately excluded from the rendered table by a filter.
        return Ok(SnapshotRow::Undetermined);
    };

    // Locate the block, then the row WITHIN it. Scanning the whole document for the
    // id would reach a row in an unrelated table, which is the defect
    // `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`
    // reports and the reason the anchor is declared rather than inferred.
    let Some((start, end)) = snapshot_block_range(&doc, &anchor) else {
        return Ok(SnapshotRow::Undetermined);
    };
    let lines: Vec<&str> = doc.lines().collect();
    let Some(idx) = (start..=end).find(|i| row_re.is_match(lines[*i])) else {
        // The id has no row in this block yet. Adding one is `append_entry`'s job with
        // an `index_row`; inventing a row here would place it by guess.
        return Ok(SnapshotRow::Undetermined);
    };
    if lines[idx] == new_row {
        // THE ONE PLACE THE BODY AND `params` ARE ACTUALLY COMPARED. Reported rather
        // than folded into the no-op above, because `snapshot_stale_note` otherwise
        // infers the opposite from id presence and states it as fact.
        return Ok(SnapshotRow::AlreadyCurrent);
    }

    // The empty-cell guard, and it exists for a specific default: MiniJinja renders an
    // UNDEFINED variable as the empty string and that is pinned as intended behaviour
    // (`missing_var_does_not_error_by_default`). Correct for a context bundle, where a
    // blank cell is cosmetic; against a file it turns a params-key rename into silent
    // truncation of real content. Comparing populated cell COUNTS catches exactly that
    // and stays silent for an ordinary edit that shortens a cell.
    let populated = |row: &str| row.split('|').filter(|c| !c.trim().is_empty()).count();
    if populated(new_row) < populated(lines[idx]) {
        return Err(LibrarianRecoverableError::with_hint(
            format!(
                "update_entry: `{entry_id}` was patched, but the re-rendered snapshot row has \
                 fewer populated cells than the one it would replace ({} -> {}) — nothing was \
                 written",
                populated(lines[idx]),
                populated(new_row)
            ),
            "MiniJinja renders an undefined variable as empty, so a renamed or removed `params` \
             key blanks its cell instead of failing. Check that `render_template` still names \
             the fields this entry carries."
                .to_string(),
        ));
    }

    let mut out: Vec<&str> = lines.clone();
    out[idx] = new_row;
    let mut text = out.join("\n");
    if doc.ends_with('\n') {
        text.push('\n');
    }
    // A catalogued row can be an in-root link to a file outside the repository;
    // `write_catalogued_file` refuses a link, inside the caller's transaction.
    crate::librarian::util::write_catalogued_file(roots, std::path::Path::new(&abs_path), &text)
        .map_err(|e| {
            LibrarianRecoverableError::new(format!(
                "update_entry: `{entry_id}` was patched, but writing the snapshot row to \
                 {abs_path} failed: {e:#} — the params change was rolled back"
            ))
        })?;
    record_written_file(tx, artifact_id, &abs_path, &text)?;
    Ok(SnapshotRow::Rewritten)
}

/// `<#×level> <id> — <title>`, a blank line, the prose with the allocator's validity stamp, a blank
/// line.
pub(crate) fn render_pending_section(id: &str, level: usize, s: &PendingSection) -> Result<String> {
    let heading = "#".repeat(level);
    let prose = s.body.trim_end();
    // Every section the server writes is born with a declared decay class, the same way it
    // is born with a def_re-conformant heading: by construction, not by convention. A
    // caller that already declared one is left alone — double-stamping would make the
    // parser's first-match rule pick between the caller's class and this one arbitrarily. A
    // malformed declaration propagates the parser's own error via `?` — before any write —
    // rather than being joined by a second `**Valid:**` line, which would make the
    // malformed one authoritative under first-wins and leave the entry permanently
    // unparseable. Repair only when exactly one interpretation is correct
    // (docs/adrs/2026-07-10-repair-and-continue-input-handling.md); a malformed date has
    // none.
    let stamped = match crate::librarian::statements::parse_validity(prose)? {
        Some(_) => prose.to_string(),
        None => format!("**Valid:** dated {}\n\n{prose}", today_iso()),
    };
    // The heading is formatted here and nowhere else: `<level> <ID> — <title>` is exactly
    // `link_scan`'s `def_re`, so an entry written through this path cannot be born
    // undefined. Trailing blank line so the anchor heading that follows is not glued to
    // this section's last prose line.
    Ok(format!("{heading} {id} — {}\n\n{stamped}\n\n", s.title))
}

/// Splice a [`PendingSection`] — and its optional index row — into `doc`, returning the
/// whole document so the caller writes it in ONE `fs::write`.
///
/// **Extracted so the params path can reach it, and shared rather than copied for the
/// reason this module keeps paying for.** It was inline in [`allocate_entry_id`], which
/// is why `append_entry` accepted `title`/`body`/`anchor_heading`/`index_row` and silently
/// dropped all four: the code that honours them lived inside the branch that excluded it
/// (`docs/issues/archive/2026-09-12-append-entry-drops-section-and-index-row-on-the-params-path.md`).
/// A second copy would have reproduced that the moment either drifted.
///
/// `caller` names the function in the refusal text. Both callers abort BEFORE any write,
/// so both can honestly say nothing was written — see each call site for the ordering that
/// makes that true.
pub(crate) fn splice_pending_section(
    doc: &str,
    id: &str,
    level: usize,
    s: &PendingSection,
    caller: &str,
) -> Result<String> {
    let section_text = render_pending_section(id, level, s)?;
    let with_section = crate::tools::markdown::edit_markdown::perform_section_edit_ext(
        doc,
        &s.anchor_heading,
        "insert_before",
        Some(&section_text),
        None,
        false,
    )
    .map_err(|e| {
        // The document is in memory right here, so the recovery can be CONCRETE instead of
        // a referral. Naming the last top-level headings specifically, because a ledger's
        // append anchor is conventionally its final stanza and `doc(action="get")`'s
        // heading window fills from the top.
        let tail: Vec<String> = crate::librarian::preview::headings::parse(doc)
            .into_iter()
            .filter(|h| h.level <= 2)
            .rev()
            .take(3)
            .map(|h| format!("`{} {}`", "#".repeat(h.level as usize), h.text))
            .collect();
        let tail_hint = if tail.is_empty() {
            "This ledger has no top-level heading to anchor against, so the section must be \
             added by hand."
                .to_string()
        } else {
            format!(
                "Its last top-level headings, closest to the end first, are: {}. A ledger's \
                 append anchor is conventionally the final one.",
                tail.join(", ")
            )
        };
        LibrarianRecoverableError::with_hint(
            format!(
                "{caller}: cannot place {id} before `{}`: {e} — no id was allocated and \
                 nothing was written",
                s.anchor_heading
            ),
            format!(
                "`anchor_heading` must name a heading that exists in the ledger verbatim, \
                 including its `#` prefix. {tail_hint}"
            ),
        )
    })?;
    // The row goes into the SAME string the section went into, so one `fs::write` carries
    // both. Writing it as a second call is exactly the window this closes: the row cannot
    // be written FIRST (the allocator counts a row as a claimed id, so it would consume the
    // number it names), so a caller doing two calls always leaves the entry row-less for
    // the interval between them, and no discipline available to them shortens it.
    match &s.index_row {
        None => Ok(with_section),
        Some(r) => {
            let row = r.row.replace("{id}", id);
            // Resolve the anchor against `with_section` — the bytes about to be written,
            // frontmatter included — so a `Declared` anchor is read from the same document
            // version the row lands in. `?` propagates BEFORE any write, so the caller's
            // "nothing was written" promise still holds.
            let after = resolve_row_anchor(&with_section, &r.after_line, id, caller)?;
            insert_index_row(&with_section, &after, &row).map_err(|e| {
                LibrarianRecoverableError::with_hint(
                    format!(
                        "{caller}: cannot place the index row for {id}: {e} — no id was \
                         allocated and nothing was written"
                    ),
                    "`index_row.after_line` must name a line that exists in the ledger, \
                     compared with surrounding whitespace trimmed. For a newest-first table \
                     that is the separator row, e.g. `|----|-------|`.",
                )
            })
        }
    }
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
        return Err(LibrarianRecoverableError::new(format!(
            "append_entry: cite `{raw}` — no such entry (slug or local id not found)"
        )));
    }
    // 3. rel_path suffix match — must resolve to exactly one artifact. Byte-exact, not `LIKE`:
    // `LIKE` folds ASCII case, so `notes.md` also named the row of `Notes.md` (and made a
    // pair of them ambiguous). `substr` with a negative start takes the last N CHARACTERS and
    // `length` counts characters, so the two agree for a non-ASCII path.
    let suffix = format!("/{raw}");
    let mut stmt = conn.prepare(
        "SELECT id FROM artifact WHERE abs_path = ?1 OR substr(abs_path, -length(?2)) = ?2",
    )?;
    let ids: Vec<String> = stmt
        .query_map(rusqlite::params![raw, suffix], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match ids.len() {
        1 => Ok(ids.into_iter().next().unwrap()),
        0 => Err(LibrarianRecoverableError::with_hint(
            format!("append_entry: cite `{raw}` did not resolve"),
            "Use a 16-hex artifact id, a `<slug>:<local>` entry id, or a unique rel_path."
                .to_string(),
        )),
        _ => Err(LibrarianRecoverableError::new(format!(
            "append_entry: cite `{raw}` is ambiguous ({} artifacts match)",
            ids.len()
        ))),
    }
}
/// The cites of one `append_entry` call after resolution.
struct ResolvedCites {
    /// Each ref's `entry_cite.dst_ref`, in input order (what the `origin='write'` rows key on).
    refs: Vec<String>,
    /// Each ref as grammar text, in input order with exact repeats dropped. Empty when the call
    /// writes no section, so there is no text to carry them.
    forms: Vec<String>,
}

/// Resolves every cite of an `append_entry` call, and, when the call writes a section, renders
/// each as grammar text with [`grammar_form_of`] against the ledger's git root. The first cite
/// that cannot be resolved or named refuses the whole call.
///
/// The ledger's git root is the one above its file ([`lookup_git_root`]); a ledger that lies in
/// no git repository has no repository to name its cites against, so a call that cites from it is
/// refused, by file name. A call with no cites needs no root.
fn resolve_cites(
    conn: &rusqlite::Connection,
    ledger_path: Option<&str>,
    cites: &[String],
    writes_section: bool,
) -> Result<ResolvedCites> {
    let mut refs = Vec::with_capacity(cites.len());
    let mut forms: Vec<String> = Vec::new();
    let text_root = match (writes_section, ledger_path, cites.is_empty()) {
        (true, Some(path), false) => {
            let root =
                crate::librarian::current_project::lookup_git_root(std::path::Path::new(path))
                    .ok_or_else(|| {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
                        LibrarianRecoverableError::new(format!(
                    "append_entry: `{name}` lies in no git repository, so its citations cannot \
                     be written as grammar text — retry without `cites`; nothing was written"
                ))
                    })?;
            Some(root)
        }
        _ => None,
    };
    for raw in cites {
        let dst_ref = resolve_cite_ref(conn, raw)?;
        if let Some(root) = &text_root {
            let form = grammar_form_of(conn, root, &dst_ref)?;
            if !forms.contains(&form) {
                forms.push(form);
            }
        }
        refs.push(dst_ref);
    }
    Ok(ResolvedCites { refs, forms })
}

/// The note a ledger gets when its own file name is not a canonical key: its entries are written
/// and their bare tokens resolve inside it, but `[[key:TOKEN]]` has no key to name it by until the
/// file is renamed. `None` when the name is a key, and when the ledger has no git root to judge
/// the name against. Names the file by its name, never by a path.
fn ledger_key_note(ledger_path: &str, entry_id: &str) -> Option<String> {
    let root =
        crate::librarian::current_project::lookup_git_root(std::path::Path::new(ledger_path))?;
    let Scope::Inside(rel) = scope_of(&root, ledger_path) else {
        return None;
    };
    key_of(&rel).err()?;
    let name = std::path::Path::new(&rel).file_name()?.to_string_lossy();
    Some(format!(
        "`{name}` is not a canonical key, so no `[[key:{entry_id}]]` can name this entry from \
         another document until the file is renamed (lowercase letters and digits, single \
         hyphens). The entry is written, and the bare token `{entry_id}` resolves inside this \
         ledger."
    ))
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

/// [`body_snapshot_row_indices`], narrowed to the artifact's DECLARED snapshot block
/// when it declares one.
///
/// Takes the whole document rather than a body slice because it needs both halves —
/// the frontmatter to read [`SNAPSHOT_ANCHOR_KEY`], the body to locate the block —
/// and all three consumers already hold the file exactly as `read_to_string` returned
/// it, so this costs no caller an extra read.
///
/// **An absent, drifted or ambiguous anchor falls back to the whole-document scan**,
/// byte for byte today's behaviour. That is the asymmetry
/// `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`
/// already names: a **read** may fall back, a **write** may not — which is why
/// [`resync_snapshot_row`], the write, returns `Ok(false)` on the very same input
/// instead of guessing a block.
///
/// **Measured 2026-09-14 before wiring, and the population is why the fallback is a
/// ruling rather than a compromise.** Of 13 params-backed ledgers in this repo, **4**
/// pass [`body_keeps_snapshot`] — `prompt-hamsa-audit-log` (39/39),
/// `windows-platform-support` (35/35), `2026-08-16-iron-law-gate-firing-audit` (8/8)
/// and `open-issue-work-queue` (77/77). Of those four, exactly **one** anchors its ids
/// in more than one table, and it is the one that declares. For the other three the
/// block IS the document, so both readings agree by construction. The two defaults the
/// bug file weighed therefore differ on **zero** files: absent-⇒-whole-body changes
/// nothing anywhere, while absent-⇒-no-block would silence three working advisories to
/// fix nothing observable. Re-derive before citing — the catalog is gitignored.
pub(crate) fn snapshot_rows_in_declared_block(
    doc: &str,
    id_prefix: &str,
) -> std::collections::BTreeSet<u64> {
    let whole = || body_snapshot_row_indices(doc, id_prefix);
    let Ok((fm, _)) = crate::librarian::frontmatter::parse(doc) else {
        return whole();
    };
    let Some(anchor) = declared_snapshot_anchor(fm.as_ref()) else {
        return whole();
    };
    let Some((start, end)) = snapshot_block_range(doc, &anchor) else {
        return whole();
    };
    // Feed the primitive the block alone. Re-running the same regex over a slice keeps
    // ONE definition of what a snapshot row looks like; a second regex scoped by line
    // number would be a copy free to drift from it.
    let block = doc
        .lines()
        .skip(start)
        .take(end - start + 1)
        .collect::<Vec<_>>()
        .join("\n");
    body_snapshot_row_indices(&block, id_prefix)
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
        "SELECT {AUGMENTATION_COLUMNS} FROM artifact_augmentation WHERE artifact_id IN ({placeholders})"
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
            // Byte-exact "at or under this root", not `LIKE`: the project path's `_`/`%` were
            // wildcards here (no `ESCAPE`) and ASCII case folded, so a sibling project's stale
            // augmentations were listed under this one.
            let (root, root_prefix) = crate::librarian::util::root_and_prefix(prefix_s.as_str());
            sql.push_str(&format!(
                " AND {}",
                crate::librarian::util::path_at_or_under_sql(
                    "a.abs_path",
                    &format!("?{idx}"),
                    &format!("?{}", idx + 1)
                )
            ));
            params.push(Box::new(root));
            params.push(Box::new(root_prefix));
            idx += 2;
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

    // ---- a file this module writes keeps its artifact row current --------------------------

    /// The row's content columns must describe the file this module just wrote. `doctor`'s
    /// `row_behind_file` compares exactly `file_sha256` with the file's hash, so a writer that
    /// leaves the row behind makes every ledger it touches read as needing a `reindex`.
    /// docs/issues/archive/2026-09-28-update-entry-snapshot-resync-leaves-the-catalog-row-behind-its-file.md
    fn assert_row_describes_file(cat: &Catalog, id: &str, path: &std::path::Path, writer: &str) {
        let row = crate::librarian::catalog::artifact::get(cat, id)
            .unwrap()
            .expect("the artifact row exists");
        let on_disk = crate::librarian::util::sha_of_bytes(&std::fs::read(path).unwrap());
        assert_eq!(
            row.file_sha256, on_disk,
            "{writer} rewrote the file, so the row's file_sha256 must hash the bytes it wrote; \
             otherwise doctor reports row_behind_file until a reindex"
        );
        let mtime = std::fs::metadata(path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        assert_eq!(
            row.file_mtime, mtime,
            "{writer} rewrote the file, so the row's file_mtime must be the file's own mtime"
        );
    }

    /// An artifact row whose content columns describe no file at all.
    fn stale_art(id: &str, path: &std::path::Path) -> ArtifactRow {
        let mut art = sample_art(id);
        art.abs_path = path.to_path_buf();
        // LOAD-BEARING: `abc` is no file's sha256 and `1` no real mtime, so the assertions pass
        // only if the writer stored fresh values. `sample_art`'s `now` mtime can equal the
        // file's to the millisecond and let a missing mtime refresh through.
        art.file_sha256 = "abc".to_string();
        art.file_mtime = 1;
        art
    }

    // ---- a row that is a link to a file outside the repo is never written through -----------
    //
    // Every writer below takes `abs_path` from the catalog row, and a row can name a link out of
    // the repository (a catalogued file replaced by one, or an older catalog). Fixtures replace the ledger a
    // helper just wrote with a link to a copy in a SEPARATE tempdir (bound by the caller; an
    // inline temporary would drop and leave the link dangling). Unix-only: links.

    /// Move `md`'s bytes into a fresh directory and leave a symlink at `md` pointing there.
    /// Returns the directory (keep it alive) and the victim path.
    #[cfg(unix)]
    fn replace_with_outward_link(md: &std::path::Path) -> (tempfile::TempDir, std::path::PathBuf) {
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.md");
        std::fs::write(&victim, std::fs::read(md).unwrap()).unwrap();
        std::fs::remove_file(md).unwrap();
        std::os::unix::fs::symlink(&victim, md).unwrap();
        (outside, victim)
    }

    /// The assertions every site shares: refused with the link named, the victim byte-identical,
    /// and the link still a link.
    #[cfg(unix)]
    fn assert_refused_and_untouched(
        err: &anyhow::Error,
        md: &std::path::Path,
        victim: &std::path::Path,
        victim_before: &str,
    ) {
        assert!(
            format!("{err:#}").contains("symbolic link"),
            "the refusal must say why: {err:#}"
        );
        assert_eq!(std::fs::read_to_string(victim).unwrap(), victim_before);
        assert!(std::fs::symlink_metadata(md)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn allocate_entry_id_refuses_a_row_that_is_a_link_to_a_file_outside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, before) = ledger_fixture(dir.path(), &mut cat);
        let (_outside, victim) = replace_with_outward_link(&md);

        let err = allocate_entry_id(&mut cat, "art1", "F", None)
            .expect_err("the high-water-mark write must not follow the link");

        assert_refused_and_untouched(&err, &md, &victim, &before);
        // The reservation was inserted inside the transaction before the refusal; it must have
        // rolled back, or the next allocation skips an id for a call that wrote nothing.
        let reserved: Option<i64> = cat
            .conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id='art1'",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(reserved, None, "a refused allocation consumes no id");
    }

    #[cfg(unix)]
    #[test]
    fn append_entry_refuses_to_write_its_section_through_a_link_outside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, before) = ledger_fixture(dir.path(), &mut cat);
        let (_outside, victim) = replace_with_outward_link(&md);
        let section = PendingSection {
            title: "first".to_string(),
            body: "the prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: None,
        };

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&section),
        )
        .expect_err("the section write must not follow the link");

        assert_refused_and_untouched(&err, &md, &victim, &before);
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(
            params["failures"].as_array().unwrap().len(),
            0,
            "the params row must roll back with the refused section"
        );
    }

    #[cfg(unix)]
    #[test]
    fn update_entry_refuses_to_resync_a_snapshot_row_through_a_link_outside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("queue.md");
        let before = "---\nkind: tracker\nsnapshot_anchor: '| ID | status |'\n---\n\n# Q\n\n\
                      | ID | status |\n| T-1 | open |\n\ntail\n";
        std::fs::write(&md, before).unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &stale_art("art1", &md)).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("tasks".to_string());
        a.params = r#"{"tasks":[{"id":"T-1","status":"open"}]}"#.to_string();
        a.render_template = Some(
            "| ID | status |\n{% for t in tasks %}| {{ t.id }} | {{ t.status }} |\n{% endfor %}"
                .to_string(),
        );
        upsert(&cat, &a).unwrap();
        let (_outside, victim) = replace_with_outward_link(&md);

        let err = update_entry(&mut cat, "art1", "tasks", "T-1", json!({"status": "done"}))
            .expect_err("the snapshot resync must not follow the link");

        assert_refused_and_untouched(&err, &md, &victim, before);
        let row = get(&cat, "art1").unwrap().unwrap();
        assert!(
            row.params.contains("open") && !row.params.contains("done"),
            "the params change is rolled back with the refused write: {}",
            row.params
        );
    }

    #[test]
    fn allocate_entry_id_leaves_the_row_describing_the_file_it_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        // No section: recording the high-water mark alone rewrites the frontmatter, the
        // smallest input on which this writer touches the file.
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &stale_art("art1", &md)).unwrap();

        allocate_entry_id(&mut cat, "art1", "F", None).unwrap();

        assert_row_describes_file(&cat, "art1", &md, "allocate_entry_id");
    }

    #[test]
    fn append_entry_leaves_the_row_describing_the_section_it_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, _) = ledger_fixture(dir.path(), &mut cat);
        art_upsert(&cat, &stale_art("art1", &md)).unwrap();
        let section = PendingSection {
            title: "first".to_string(),
            body: "the prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: None,
        };

        append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&section),
        )
        .unwrap();

        assert_row_describes_file(&cat, "art1", &md, "append_entry");
    }

    #[test]
    fn update_entry_resync_leaves_the_row_describing_the_table_it_rewrote() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("queue.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nsnapshot_anchor: '| ID | status |'\n---\n\n# Q\n\n\
             | ID | status |\n| T-1 | open |\n\ntail\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &stale_art("art1", &md)).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("tasks".to_string());
        a.params = r#"{"tasks":[{"id":"T-1","status":"open"}]}"#.to_string();
        a.render_template = Some(
            "| ID | status |\n{% for t in tasks %}| {{ t.id }} | {{ t.status }} |\n{% endfor %}"
                .to_string(),
        );
        upsert(&cat, &a).unwrap();

        update_entry(&mut cat, "art1", "tasks", "T-1", json!({"status": "done"})).unwrap();

        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("| T-1 | done |"),
            "the row must have been re-rendered, or this test exercises no write at all: {text}"
        );
        assert_row_describes_file(&cat, "art1", &md, "update_entry's snapshot resync");
    }

    // ---- prefix uniqueness -----------------------------------------------------------------

    /// A temp repository (`.git` makes it one for `lookup_git_root`) with its ledgers written
    /// to disk AND seeded into the catalog, since the check reads both.
    fn repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, Catalog) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        for (i, (rel, text)) in files.iter().enumerate() {
            let p = tmp.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
            art_upsert(
                &cat,
                &TestArtifactRowBuilder::new(&format!("a{i}"))
                    .with_abs_path(&p)
                    .build(),
            )
            .unwrap();
        }
        (tmp, cat)
    }

    fn prefixes(ps: &[&str]) -> Vec<String> {
        ps.iter().map(|p| p.to_string()).collect()
    }

    /// The whole feature in one case: a second ledger claiming a taken prefix is refused, and
    /// the refusal names who owns it and what is free. The remedy half is asserted as SHAPE
    /// (an owner path, a free alternative), never as prose, so rewording stays green and the
    /// deletion of either half reds.
    #[test]
    fn a_prefix_another_ledger_owns_is_refused_naming_the_owner_and_a_free_alternative() {
        let (tmp, cat) = repo_with(&[(
            "docs/trackers/recon.md",
            "---\nkind: tracker\nentry_prefix: R\n---\n\n## R-1 — first\n",
        )]);
        let err = refuse_taken_prefixes(
            &cat.conn,
            &tmp.path().join("docs/trackers/new.md"),
            &prefixes(&["R"]),
        )
        .unwrap_err();
        let rec = err
            .downcast_ref::<crate::librarian::tools::LibrarianRecoverableError>()
            .expect("a refusal the caller can act on must stay recoverable");
        let text = format!("{} {}", rec.message, rec.hint.clone().unwrap_or_default());
        assert!(text.contains("recon.md"), "must name the owner: {text}");
        assert!(
            text.contains("`RA`") || text.contains("`RB`"),
            "must offer a free alternative to pick instead: {text}"
        );
    }

    /// A suggested alternative is FREE. Load-bearing: `RA` is taken here, so a suggestion
    /// list that skipped the ownership check would offer it — and the first test cannot see
    /// that, since its fixture owns only `R` and every `R?` candidate is free anyway. A
    /// refusal that hands the caller a taken prefix sends them straight into a second refusal.
    #[test]
    fn a_suggested_alternative_is_never_itself_taken() {
        let (tmp, cat) = repo_with(&[
            ("docs/trackers/r.md", "---\nentry_prefix: R\n---\n"),
            ("docs/trackers/ra.md", "---\nentry_prefix: RA\n---\n"),
        ]);
        let err = refuse_taken_prefixes(
            &cat.conn,
            &tmp.path().join("docs/trackers/new.md"),
            &prefixes(&["R"]),
        )
        .unwrap_err();
        let rec = err
            .downcast_ref::<crate::librarian::tools::LibrarianRecoverableError>()
            .unwrap();
        let hint = rec.hint.clone().unwrap_or_default();
        assert!(!hint.contains("`RA`"), "offered a taken prefix: {hint}");
        assert!(
            hint.contains("`RB`"),
            "the next free one should be offered: {hint}"
        );
    }

    /// A prefix the entry-token grammar cannot express is refused even when FREE — the
    /// ownership refusal above never fires here, since no other ledger holds `DCTX`, which is
    /// exactly how the live `DCTX` ledger was born. The remedy is asserted as SHAPE: a
    /// three-letter alternative is offered.
    ///
    /// The `DCX` control is load-bearing: without it, a refusal of EVERY prefix passes.
    /// docs/issues/archive/2026-09-21-a-four-letter-entry-prefix-allocates-but-cannot-be-cited.md
    #[test]
    fn a_prefix_the_token_grammar_cannot_express_is_refused_even_when_free() {
        let (tmp, cat) = repo_with(&[]);
        let new = tmp.path().join("docs/trackers/new.md");
        for bad in ["DCTX", "r"] {
            let err = refuse_taken_prefixes(&cat.conn, &new, &prefixes(&[bad])).unwrap_err();
            let rec = err
                .downcast_ref::<crate::librarian::tools::LibrarianRecoverableError>()
                .expect("a refusal the caller can act on must stay recoverable");
            let hint = rec.hint.clone().unwrap_or_default();
            assert!(
                rec.message.contains(bad),
                "must name `{bad}`: {}",
                rec.message
            );
            if bad == "DCTX" {
                assert!(
                    hint.contains("`DCA`"),
                    "must offer a citable alternative: {hint}"
                );
            } else {
                // Uppercased before suggesting: `rA` would be a second uncitable prefix.
                assert!(hint.contains("`RA`"), "must suggest uppercase: {hint}");
            }
        }
        refuse_taken_prefixes(&cat.conn, &new, &prefixes(&["DCX"]))
            .expect("a free three-letter prefix is citable and must pass");
    }

    /// Re-declaring your OWN prefix is not a conflict — `doc(update)` rewrites the whole
    /// `extra` value, so a ledger touching an unrelated key re-sends its own declaration.
    #[test]
    fn a_ledger_re_declaring_its_own_prefix_is_not_a_conflict() {
        let (tmp, cat) = repo_with(&[(
            "docs/trackers/recon.md",
            "---\nkind: tracker\nentry_prefix: R\n---\n",
        )]);
        refuse_taken_prefixes(
            &cat.conn,
            &tmp.path().join("docs/trackers/recon.md"),
            &prefixes(&["R"]),
        )
        .expect("the owner re-declaring its own prefix must pass");
    }

    /// The owner is recognized under ANY spelling of its path that the catalog's own
    /// write normalization (`RepoPath`, i.e. `to_forward_slash`) maps onto the stored one.
    ///
    /// The Windows shape of this bug: `tmp.path().join("docs/trackers/recon.md")` is
    /// `C:\…\docs/trackers/recon.md` there, the stored row is all-forward-slash, and a raw
    /// text comparison refused the owner as a conflict with itself (c6cff39d, first seen on
    /// the wine lane). The sibling test above only reds on Windows. This one reds
    /// everywhere, because `to_forward_slash` rewrites `\` on every platform. On Linux,
    /// `trackers\recon.md` is ONE filename component, so this path names no real file. It
    /// exists only as a second spelling of the stored path, which is exactly what
    /// `declaring` is on Windows.
    #[test]
    fn the_owner_is_recognized_under_a_backslash_spelling_of_its_own_path() {
        let (tmp, cat) = repo_with(&[(
            "docs/trackers/recon.md",
            "---\nkind: tracker\nentry_prefix: R\n---\n",
        )]);
        refuse_taken_prefixes(
            &cat.conn,
            // Load-bearing: the last separator is a BACKSLASH. With `/` this is the
            // byte-identical spelling and the test passes whether or not `me` is normalized.
            &tmp.path().join("docs").join("trackers\\recon.md"),
            &prefixes(&["R"]),
        )
        .expect("a second spelling of the owner's own path must not read as another owner");
    }

    /// The one shared family. Load-bearing: the declarer here would be refused for `R` in the
    /// same call if the exemption were a blanket "allow everything", so the case pins that
    /// the exemption is F/W and nothing wider.
    #[test]
    fn the_session_log_family_is_shared_and_nothing_else_is() {
        let (tmp, cat) = repo_with(&[
            (
                "docs/trackers/a-session-log.md",
                "---\nentry_prefix: [F, W]\n---\n",
            ),
            ("docs/trackers/recon.md", "---\nentry_prefix: R\n---\n"),
        ]);
        let new = tmp.path().join("docs/trackers/b-session-log.md");
        refuse_taken_prefixes(&cat.conn, &new, &prefixes(&["F", "W"]))
            .expect("every session log declares F and W, cited qualified by stem");
        assert!(
            refuse_taken_prefixes(&cat.conn, &new, &prefixes(&["F", "W", "R"])).is_err(),
            "the exemption covers F and W only"
        );
    }

    /// An `external_prefix` declaration owns its prefix exactly as a ledger does: a ledger
    /// declaring `entry_prefix: TC` would make `TC` known to the resolver and turn every
    /// citation of the benchmark's ids dangling — the measured −1 finding, +6 damage.
    #[test]
    fn an_external_namespace_owns_its_prefix() {
        let (tmp, cat) = repo_with(&[(
            "docs/trackers/benchmark.md",
            "---\nexternal_prefix:\n  TC: scripts/tc-suites/legacy-natural.json\n---\n",
        )]);
        assert!(refuse_taken_prefixes(
            &cat.conn,
            &tmp.path().join("docs/trackers/new.md"),
            &prefixes(&["TC"]),
        )
        .is_err());
    }

    /// Status is ignored: an archived ledger's namespace is still bound by every citation of
    /// its entries, so reusing it captures them. Pinned against an "active rows only"
    /// optimisation that would read as harmless.
    #[test]
    fn an_archived_ledger_still_owns_its_prefix() {
        let (tmp, cat) = repo_with(&[(
            "docs/trackers/archive/old.md",
            "---\nkind: tracker\nstatus: archived\nentry_prefix: U\n---\n",
        )]);
        cat.conn
            .execute("UPDATE artifact SET status = 'archived'", [])
            .unwrap();
        assert!(refuse_taken_prefixes(
            &cat.conn,
            &tmp.path().join("docs/trackers/new.md"),
            &prefixes(&["U"]),
        )
        .is_err());
    }

    /// Ownership is per REPOSITORY: bare citations resolve inside one repo, and a cross-repo
    /// citation is qualified (`<repo>:<TOKEN>`). Refusing across repos would stop a sibling
    /// project from ever using `R` because this one does. Load-bearing: the two temp repos
    /// are SIBLINGS, so neither path is a prefix of the other.
    #[test]
    fn a_ledger_in_another_repository_does_not_own_the_prefix_here() {
        let (other, cat) = repo_with(&[("docs/trackers/recon.md", "---\nentry_prefix: R\n---\n")]);
        let here = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(here.path().join(".git")).unwrap();
        let _keep = other;
        refuse_taken_prefixes(
            &cat.conn,
            &here.path().join("docs/trackers/new.md"),
            &prefixes(&["R"]),
        )
        .expect("a prefix owned in a sibling repository is free here");
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

    /// Seeds an artifact whose `candidates` collection holds rows that carry `key` and NO
    /// `id` — the shape of the legibility backlog (`CandidateRow { key, .. }`).
    fn seed_key_rows(cat: &Catalog, id: &str, rows: &str) {
        art_upsert(cat, &sample_art(id)).unwrap();
        let mut a = aug(id);
        a.entry_collection = Some("candidates".into());
        a.params = format!(r#"{{"candidates":{rows}}}"#);
        upsert(cat, &a).unwrap();
    }

    /// `(message, hint)` of a refused `update_entry`. Asserting on the parts and not on
    /// `to_string()`, which folds the hint into the message and so cannot tell which of the two
    /// carried a given word.
    fn refusal_parts(e: anyhow::Error) -> (String, String) {
        let r = e
            .downcast_ref::<LibrarianRecoverableError>()
            .unwrap_or_else(|| panic!("expected a LibrarianRecoverableError, got: {e}"));
        (r.message.clone(), r.hint.clone().unwrap_or_default())
    }

    fn candidates_of(cat: &Catalog, id: &str) -> Vec<Value> {
        let row = get(cat, id).unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        params["candidates"].as_array().unwrap().clone()
    }

    const KEY_ROWS: &str = r#"[
        {"key":"src/a.rs::f","status":"open","tier":1},
        {"key":"src/b.rs::g","status":"open","tier":2},
        {"key":"tests/c.rs::all","status":"open","tier":2}
    ]"#;

    /// The case this bug was filed on: the legibility backlog's rows carry `key` and no `id`,
    /// so `update_entry` answered `no entry ...` for a row that exists.
    /// docs/issues/archive/2026-10-06-update-entry-cannot-address-rows-keyed-by-key-and-prints-an-empty-known-ids-list.md
    #[test]
    fn update_entry_addresses_a_row_that_has_a_key_and_no_id() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_key_rows(&cat, "art1", KEY_ROWS);

        let out = update_entry(
            &mut cat,
            "art1",
            "candidates",
            "src/b.rs::g",
            json!({"status":"closed","closed_reason":"refactored"}),
        )
        .unwrap();

        assert_eq!(out.entry_id, "src/b.rs::g");
        assert_eq!(
            out.entries_total, 3,
            "an update never changes the row count"
        );
        let mut changed = out.changed_fields.clone();
        changed.sort();
        assert_eq!(
            changed,
            vec!["closed_reason".to_string(), "status".to_string()]
        );
        let rows = candidates_of(&cat, "art1");
        assert_eq!(rows[1]["status"], "closed");
        assert_eq!(rows[1]["closed_reason"], "refactored");
        assert_eq!(rows[1]["key"], "src/b.rs::g", "the address is untouched");
        assert_eq!(rows[1]["tier"], 2, "a field outside the patch survives");
        for i in [0, 2] {
            assert_eq!(
                rows[i]["status"], "open",
                "row {i} is not the addressed one"
            );
            assert!(rows[i].get("closed_reason").is_none(), "row {i} untouched");
        }
    }

    /// A row with both an `id` and a `key` is addressed by its `id` only; its `key` is an
    /// ordinary field there. Otherwise one row would answer to two names.
    #[test]
    fn update_entry_addresses_a_row_that_has_both_by_its_id_only() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_key_rows(
            &cat,
            "art1",
            r#"[{"id":"T-1","key":"k1","status":"open"},{"key":"k2","status":"open"}]"#,
        );

        update_entry(
            &mut cat,
            "art1",
            "candidates",
            "T-1",
            json!({"status":"done"}),
        )
        .unwrap();
        let rows = candidates_of(&cat, "art1");
        assert_eq!(rows[0]["status"], "done");
        assert_eq!(rows[1]["status"], "open");

        let (msg, hint) = refusal_parts(
            update_entry(&mut cat, "art1", "candidates", "k1", json!({"status":"x"})).unwrap_err(),
        );
        assert!(
            msg.contains("no entry `k1`"),
            "a row that has an id is not addressed by its key; got: {msg}"
        );
        assert_eq!(hint, "Known ids and keys: T-1, k2");

        // `key` is not the address of that row, so patching it is allowed.
        update_entry(&mut cat, "art1", "candidates", "T-1", json!({"key":"k1b"})).unwrap();
        assert_eq!(candidates_of(&cat, "art1")[0]["key"], "k1b");
    }

    /// Never guess. A value that is one row's `id` and another row's `key`, or the `key` of two
    /// rows, names more than one row, so the call is refused and nothing is written.
    #[test]
    fn update_entry_refuses_to_guess_between_an_id_and_a_key_or_between_two_keys() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_key_rows(
            &cat,
            "art1",
            r#"[{"id":"X-1","status":"open"},{"key":"X-1","status":"open"}]"#,
        );
        let before = candidates_of(&cat, "art1");
        let (msg, hint) = refusal_parts(
            update_entry(&mut cat, "art1", "candidates", "X-1", json!({"status":"d"})).unwrap_err(),
        );
        assert!(
            msg.contains("`X-1` is ambiguous in `candidates`")
                && msg.contains("the `id` of one row and the `key` of 1 other row(s)"),
            "got: {msg}"
        );
        assert!(hint.contains("will not guess"), "got: {hint}");
        assert_eq!(
            candidates_of(&cat, "art1"),
            before,
            "a refusal writes nothing"
        );

        seed_key_rows(
            &cat,
            "art2",
            r#"[{"key":"dup","status":"open"},{"key":"dup","status":"open"},{"key":"solo","status":"open"}]"#,
        );
        let before = candidates_of(&cat, "art2");
        let (msg, _) = refusal_parts(
            update_entry(&mut cat, "art2", "candidates", "dup", json!({"status":"d"})).unwrap_err(),
        );
        assert!(msg.contains("it is the `key` of 2 rows"), "got: {msg}");
        assert_eq!(
            candidates_of(&cat, "art2"),
            before,
            "a refusal writes nothing"
        );
        // The unique key beside the duplicates is still addressable.
        update_entry(
            &mut cat,
            "art2",
            "candidates",
            "solo",
            json!({"status":"d"}),
        )
        .unwrap();
        assert_eq!(candidates_of(&cat, "art2")[2]["status"], "d");
    }

    /// The hint used to print `Known ids:` followed by nothing for rows without an `id`, with
    /// the whole collection counted as `(+N more)`. It must list what IS addressable.
    #[test]
    fn update_entry_unknown_key_lists_the_addressable_keys_and_not_an_empty_list() {
        let mut cat = Catalog::open_in_memory().unwrap();
        let rows: Vec<String> = (0..14).map(|i| format!(r#"{{"key":"k{i:02}"}}"#)).collect();
        seed_key_rows(&cat, "art1", &format!("[{}]", rows.join(",")));

        let (msg, hint) = refusal_parts(
            update_entry(&mut cat, "art1", "candidates", "nope", json!({"s":1})).unwrap_err(),
        );
        assert_eq!(msg, "update_entry: no entry `nope` in `candidates`");
        let shown: Vec<String> = (0..12).map(|i| format!("k{i:02}")).collect();
        assert_eq!(hint, format!("Known keys: {} (+2 more)", shown.join(", ")));

        // Rows with neither handle are counted, not folded into "more".
        seed_key_rows(&cat, "art2", r#"[{"key":"a"},{"note":"x"}]"#);
        let (_, hint) = refusal_parts(
            update_entry(&mut cat, "art2", "candidates", "nope", json!({"s":1})).unwrap_err(),
        );
        assert_eq!(
            hint,
            "Known keys: a (1 row(s) carry neither an `id` nor a `key` and cannot be addressed)"
        );
    }

    /// The legacy path is unchanged: an id-only collection prints the same `Known ids:` list,
    /// capped at twelve with the same `(+N more)`, and a duplicate id still patches the first.
    #[test]
    fn update_entry_keeps_the_id_only_behaviour_byte_for_byte() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_task_list(&cat, "art1");
        let (_, hint) = refusal_parts(
            update_entry(&mut cat, "art1", "tasks", "T-9", json!({"status":"done"})).unwrap_err(),
        );
        assert_eq!(hint, "Known ids: T-1, T-2, T-3");

        let rows: Vec<String> = (1..=15).map(|i| format!(r#"{{"id":"T-{i}"}}"#)).collect();
        art_upsert(&cat, &sample_art("art2")).unwrap();
        let mut a = aug("art2");
        a.entry_collection = Some("tasks".into());
        a.params = format!(r#"{{"tasks":[{}]}}"#, rows.join(","));
        upsert(&cat, &a).unwrap();
        let (_, hint) = refusal_parts(
            update_entry(&mut cat, "art2", "tasks", "T-99", json!({"status":"done"})).unwrap_err(),
        );
        let shown: Vec<String> = (1..=12).map(|i| format!("T-{i}")).collect();
        assert_eq!(hint, format!("Known ids: {} (+3 more)", shown.join(", ")));

        art_upsert(&cat, &sample_art("art3")).unwrap();
        let mut a = aug("art3");
        a.entry_collection = Some("tasks".into());
        a.params = r#"{"tasks":[{"id":"T-1","n":1},{"id":"T-1","n":2}]}"#.to_string();
        upsert(&cat, &a).unwrap();
        update_entry(&mut cat, "art3", "tasks", "T-1", json!({"status":"done"})).unwrap();
        let row = get(&cat, "art3").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(
            params["tasks"][0]["status"], "done",
            "the first match is patched"
        );
        assert!(
            params["tasks"][1].get("status").is_none(),
            "the duplicate is not"
        );
    }

    /// A `key` is the only address of a row that has no `id`, so a patch that rewrites it would
    /// re-address the row. Refused, and nothing is written.
    #[test]
    fn update_entry_refuses_to_rewrite_the_key_that_is_a_rows_only_address() {
        let mut cat = Catalog::open_in_memory().unwrap();
        seed_key_rows(&cat, "art1", KEY_ROWS);
        let before = candidates_of(&cat, "art1");

        let (msg, hint) = refusal_parts(
            update_entry(
                &mut cat,
                "art1",
                "candidates",
                "src/a.rs::f",
                json!({"key":"src/a.rs::renamed","status":"closed"}),
            )
            .unwrap_err(),
        );
        assert!(
            msg.contains("`key` cannot be changed through a field patch on `src/a.rs::f`"),
            "got: {msg}"
        );
        assert!(hint.contains("only address"), "got: {hint}");
        assert_eq!(
            candidates_of(&cat, "art1"),
            before,
            "a refusal writes nothing"
        );
    }

    /// The two body advisories reason about CITABLE ids. A `key` that merely LOOKS like one
    /// (`F-8`) must not be parsed as one. The id-addressed twin proves the note does fire for a
    /// real entry in the same file, so the silence below is the skip and not an empty body.
    #[test]
    fn update_entry_says_nothing_about_citable_headings_for_a_row_addressed_by_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(&path, "# Tracker\n\n## F-1 — one\nprose\n").unwrap();

        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = path.clone();
        art_upsert(&cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("candidates".to_string());
        a.params = r#"{"candidates":[{"id":"F-9","status":"open"},{"key":"F-8","status":"open"}]}"#
            .to_string();
        upsert(&cat, &a).unwrap();

        let twin = update_entry(
            &mut cat,
            "art1",
            "candidates",
            "F-9",
            json!({"status":"done"}),
        )
        .unwrap();
        assert!(
            twin.undefined_in_body.is_some(),
            "control: F-9 is an id with no `## F-9 —` heading, so the advisory fires"
        );

        let out = update_entry(
            &mut cat,
            "art1",
            "candidates",
            "F-8",
            json!({"status":"done"}),
        )
        .unwrap();
        assert!(
            out.undefined_in_body.is_none(),
            "a key is not an entry id: {:?}",
            out.undefined_in_body
        );
        assert!(out.snapshot_stale.is_none());
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
            None,
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

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None)
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

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None)
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

        let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None).unwrap();

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
    /// docs/issues/2026-09-24-residual-append-entry-keeps-rendered-snapshot-in-sync.md
    ///
    /// The shape of `system-retrospective-improvements` (SRI: 17 defining headings and **2**
    /// table rows, `SRI-11` and `SRI-12`, sitting in an unrelated findings table): a ledger
    /// whose entries are SECTIONS and which merely cites two of its ids in a table that is
    /// not a snapshot of anything. It keeps no rendered table, so it is owed no
    /// `snapshot_missing` — and with no declared anchor there is nothing it could be told to
    /// fix, because the row advisory's remedy is "add the row to the table".
    ///
    /// **The contract that decides this is the majority gate in `body_keeps_snapshot`**, and
    /// it holds because a stray citation covers a MINORITY of the ids (here 2 of 6), where a
    /// snapshot that lags at its tail still covers most of them.
    ///
    /// **The control is inside the test and it is what makes the silence evidence.** An
    /// `is_empty()` assertion is monotone under removal: a dead mechanism, or a fixture whose
    /// rows never matched, produces exactly the same silence. The second half runs the SAME
    /// params against a table that covers 4 of the 6 ids and demands the advisory, so a
    /// fixture that cannot be reached by the check fails there instead of passing here.
    ///
    /// Mutations probed with `./scripts/mutation-probe.sh` against these three tests alone, all
    /// KILLED: the heading alternation added to `body_snapshot_row_indices`; the majority gate
    /// loosened to "any row at all"; the gate raised past any snapshot (kills the controls).
    #[test]
    fn append_entry_stays_silent_for_a_heading_ledger_that_cites_two_ids_in_a_table() {
        let params =
            r#"{"failures":[{"id":"F-1"},{"id":"F-2"},{"id":"F-3"},{"id":"F-4"},{"id":"F-5"}]}"#;
        let headings = "## F-1 — one\nprose\n\n## F-2 — two\nprose\n\n## F-3 — three\nprose\n\n\
                        ## F-4 — four\nprose\n\n## F-5 — five\nprose\n\n";
        let run = |table: &str| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("tracker.md");
            std::fs::write(&path, format!("# Tracker\n\n{headings}{table}")).unwrap();
            let mut cat = Catalog::open_in_memory().unwrap();
            let mut art = sample_art("art1");
            art.abs_path = path.clone();
            art_upsert(&cat, &art).unwrap();
            let mut a = aug("art1");
            a.entry_collection = Some("failures".to_string());
            a.params = params.to_string();
            upsert(&cat, &a).unwrap();
            append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None).unwrap()
        };

        // Load-bearing: `F-1`/`F-2` are the FIRST CELL of a `|` row, which is the only shape
        // `body_snapshot_row_indices` counts. Moving them to a later cell would make this
        // fixture a heading-only body and the test would stop discriminating the gate.
        let stray =
            run("| Finding | Cause |\n|---|---|\n| F-1 | cited here |\n| F-2 | and here |\n");
        assert_eq!(stray.id, "F-6");
        assert!(
            stray.snapshot_missing.is_empty(),
            "2 of 6 ids cited in a table is a citation, not a snapshot: {:?}",
            stray.snapshot_missing
        );

        let kept =
            run("| ID | T |\n|---|---|\n| F-1 | a |\n| F-2 | b |\n| F-3 | c |\n| F-4 | d |\n");
        assert_eq!(
            kept.snapshot_missing,
            vec!["F-5".to_string(), "F-6".to_string()],
            "CONTROL: the same params against a table covering 4 of 6 ids IS a lagging snapshot"
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

        let out = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None).unwrap();

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

    /// The narrowing itself: a declared anchor confines the set to ITS block, so a
    /// `PREFIX-N` row sitting in an unrelated table further down is not a snapshot
    /// row. This is `20159deb3f4d4c09` /
    /// `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`.
    ///
    /// **The first assertion is the control and it is what makes the second one
    /// evidence.** Without it, a fixture whose second table simply failed to match
    /// would satisfy the narrowing assertion while the narrowing did nothing —
    /// § *Testing Discipline*'s "a broken world produces the same result".
    ///
    /// Fixture detail that is load-bearing: `| A-9 | z |` lives under a DIFFERENT
    /// header (`| Other | T |`) separated by a blank line. Move it adjacent to the
    /// first table and the contiguity walk swallows it legitimately, leaving this
    /// test green and no longer discriminating.
    ///
    /// Mutation that reds here and nowhere else: pass `doc` instead of the sliced
    /// `block` to `body_snapshot_row_indices`.
    #[test]
    fn snapshot_rows_in_declared_block_cannot_reach_an_unrelated_table() {
        let doc = "---\nkind: tracker\nsnapshot_anchor: '| ID | X |'\n---\n\n\
                   | ID | X |\n|----|---|\n| A-1 | a |\n| A-2 | b |\n\n\
                   prose\n\n| Other | T |\n|---|---|\n| A-9 | z |\n";
        assert_eq!(
            body_snapshot_row_indices(doc, "A"),
            [1, 2, 9].into_iter().collect(),
            "precondition: the un-narrowed scan DOES reach the second table — without \
             this the assertion below would pass on a fixture that never had a stray"
        );
        assert_eq!(
            snapshot_rows_in_declared_block(doc, "A"),
            [1, 2].into_iter().collect(),
            "the declared block stops at the blank line; A-9 is in another table"
        );
    }

    /// The ruling, pinned as a test: **an artifact that declares no anchor keeps
    /// today's whole-document behaviour**, byte for byte.
    ///
    /// Measured 2026-09-14 over this repo's 13 params-backed ledgers, 4 of which pass
    /// `body_keeps_snapshot`: exactly one of those four anchors ids in more than one
    /// table, and it declares. So the rejected alternative (absent ⇒ no block) would
    /// have silenced three working advisories — `prompt-hamsa-audit-log`,
    /// `windows-platform-support`, `2026-08-16-iron-law-gate-firing-audit` — to fix
    /// nothing observable.
    ///
    /// Mutation that reds here and nowhere else: return `Default::default()` from the
    /// `declared_snapshot_anchor` `None` arm instead of `whole()`.
    #[test]
    fn snapshot_rows_in_declared_block_without_an_anchor_scans_the_whole_document() {
        let doc = "---\nkind: tracker\n---\n\n\
                   | ID | X |\n|----|---|\n| A-1 | a |\n\nprose\n\n\
                   | Other | T |\n|---|---|\n| A-9 | z |\n";
        assert_eq!(
            snapshot_rows_in_declared_block(doc, "A"),
            body_snapshot_row_indices(doc, "A"),
            "no declaration means no narrowing — identical to the primitive, not empty"
        );
        assert_eq!(
            snapshot_rows_in_declared_block(doc, "A"),
            [1, 9].into_iter().collect(),
            "and the un-narrowed set is genuinely wider than one table, so the \
             equality above is not two empties agreeing"
        );
        assert_eq!(
            snapshot_rows_in_declared_block("| A-1 | a |\n", "A"),
            [1].into_iter().collect(),
            "a document with no frontmatter block at all falls back too, rather than \
             failing to parse into an empty set"
        );
    }

    /// A declaration the body no longer answers to — drifted, or ambiguous — falls
    /// back to the whole document rather than reporting an empty snapshot.
    ///
    /// This is the READ half of the asymmetry the bug file names: a read may fall
    /// back, a write may not. `resync_snapshot_row` takes the same two inputs and
    /// returns `Ok(false)`, declining to write, because guessing a block to SPLICE
    /// into is unrecoverable in a way guessing a block to COUNT is not.
    ///
    /// Mutation that reds here and nowhere else: return `Default::default()` from the
    /// `snapshot_block_range` `None` arm instead of `whole()`.
    #[test]
    fn snapshot_rows_in_declared_block_falls_back_when_the_anchor_does_not_identify_one() {
        let drifted = "---\nkind: tracker\nsnapshot_anchor: '| Gone | Y |'\n---\n\n\
                       | ID | X |\n|----|---|\n| A-1 | a |\n";
        assert_eq!(
            snapshot_rows_in_declared_block(drifted, "A"),
            [1].into_iter().collect(),
            "an anchor matching no line is a stale declaration, not an empty table"
        );

        // Two lines answer to this anchor, so `snapshot_block_range` refuses. The
        // read must still report what it can see rather than going silent.
        let ambiguous = "---\nkind: tracker\nsnapshot_anchor: '| ID | X |'\n---\n\n\
                         | ID | X |\n|----|---|\n| A-1 | a |\n\nprose\n\n\
                         | ID | X |\n|----|---|\n| A-9 | z |\n";
        assert_eq!(
            snapshot_block_range(ambiguous, "| ID | X |"),
            None,
            "precondition: the range walk genuinely refuses this input"
        );
        assert_eq!(
            snapshot_rows_in_declared_block(ambiguous, "A"),
            [1, 9].into_iter().collect(),
            "an ambiguous anchor falls back to the wide read — the pre-fix behaviour, \
             which is a false negative and never a false silence"
        );
    }

    /// The walk's whole job: from a declared header, reach the block's LAST row, so a
    /// new row lands at the tail of a newest-last ledger rather than under the header.
    ///
    /// Fixture detail that is load-bearing: the trailing `prose` line after the table.
    /// Without it the table runs to EOF and the loop's `break` is never exercised, so a
    /// walk that ignored non-table lines entirely would still pass.
    #[test]
    fn snapshot_block_last_line_walks_to_the_blocks_tail() {
        let doc = "---\nkind: tracker\n---\n\n| ID | X |\n|----|---|\n| A-1 | a |\n| A-2 | b |\n\nprose\n";
        assert_eq!(
            snapshot_block_last_line(doc, "| ID | X |").as_deref(),
            Some("| A-2 | b |"),
            "must reach the last row, not stop at the header or the separator"
        );
    }

    /// The discriminating test for
    /// `docs/issues/archive/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`:
    /// a SECOND table further down must be unreachable. Contiguity is what makes that
    /// true by construction rather than by the first table happening to be last.
    ///
    /// Mutating the walk to skip non-table lines (or to scan the whole document for the
    /// final `|` line) reds exactly here and nowhere else in this file.
    #[test]
    fn snapshot_block_last_line_cannot_reach_a_second_table() {
        let doc = "| ID | X |\n|----|---|\n| A-1 | a |\n\nprose\n\n| Other | T |\n|---|---|\n| A-9 | z |\n";
        assert_eq!(
            snapshot_block_last_line(doc, "| ID | X |").as_deref(),
            Some("| A-1 | a |"),
            "the walk must stop at the blank line, never crossing into the second table"
        );
    }

    /// Two lines answering to one anchor name no single block, so the anchor is not an
    /// explicit target and the call must refuse rather than take the first.
    ///
    /// Mutating the uniqueness check to `break` on first match (returning `Some`) reds
    /// here and NOT in `..._walks_to_the_blocks_tail` — one kill per guarded site, which
    /// is why this is a separate test rather than another assertion there.
    #[test]
    fn snapshot_block_last_line_refuses_a_duplicate_anchor() {
        let doc =
            "| ID | X |\n|----|---|\n| A-1 | a |\n\nprose\n\n| ID | X |\n|----|---|\n| A-9 | z |\n";
        assert_eq!(
            snapshot_block_last_line(doc, "| ID | X |"),
            None,
            "an anchor matching twice identifies no block; refusing is the contract"
        );
    }

    /// A declaration that has drifted from the body refuses too — the other direction of
    /// the same law, and the one a reader hits after renaming a column.
    #[test]
    fn snapshot_block_last_line_refuses_an_absent_anchor() {
        let doc = "| ID | X |\n|----|---|\n| A-1 | a |\n";
        assert_eq!(snapshot_block_last_line(doc, "| Gone | Y |"), None);
        assert_eq!(
            snapshot_block_last_line(doc, "   "),
            None,
            "a blank declaration is absent, not a match against a blank line"
        );
    }

    /// A header with no rows under it yields the header itself, so the first row of an
    /// empty table lands directly beneath it. Degenerate but reachable: it is the state
    /// of every snapshot block on the day it is created.
    #[test]
    fn snapshot_block_last_line_on_an_empty_table_returns_the_header() {
        let doc = "| ID | X |\n\nprose\n";
        assert_eq!(
            snapshot_block_last_line(doc, "| ID | X |").as_deref(),
            Some("| ID | X |")
        );
    }

    /// `declared_snapshot_anchor` reads `extra`, so it must accept what a caller writes
    /// through `doc(update, patch={extra: …})` and reject what would silently become a
    /// match against a blank line.
    #[test]
    fn declared_snapshot_anchor_reads_the_frontmatter_key_and_rejects_blanks() {
        let parse = |doc: &str| {
            let (fm, _) = crate::librarian::frontmatter::parse(doc).unwrap();
            declared_snapshot_anchor(fm.as_ref())
        };
        assert_eq!(
            parse("---\nkind: tracker\nsnapshot_anchor: '| ID | X |'\n---\n\nbody\n").as_deref(),
            Some("| ID | X |")
        );
        assert_eq!(
            parse("---\nkind: tracker\nsnapshot_anchor: '   '\n---\n\nbody\n"),
            None,
            "whitespace-only is a non-declaration, not an anchor"
        );
        assert_eq!(
            parse("---\nkind: tracker\n---\n\nbody\n"),
            None,
            "no key at all is the CORRECT state for most trackers"
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

    /// The section text is built in one place: `splice_pending_section` inserts exactly what
    /// `render_pending_section` returns, so the cutover renders a recovered entry as a live write does.
    #[test]
    fn render_pending_section_is_the_text_splice_inserts() {
        let doc = "# L\n\n## U-1 — first\n\nx\n\n## Template for new entries\n";
        let section_of = |body: &str| PendingSection {
            title: "second".into(),
            body: body.into(),
            anchor_heading: "## Template for new entries".into(),
            index_row: None,
        };
        for body in ["prose here\n", "**Valid:** dated 2026-01-02\n\nprose here"] {
            let s = section_of(body);
            let section = render_pending_section("U-2", 2, &s).unwrap();
            let spliced = splice_pending_section(doc, "U-2", 2, &s, "test").unwrap();
            let anchor = "## Template for new entries\n";
            assert_eq!(spliced, doc.replace(anchor, &format!("{section}{anchor}")));
        }
        let stamped = render_pending_section("U-2", 3, &section_of("prose here\n")).unwrap();
        let today = today_iso();
        assert_eq!(
            stamped,
            format!("### U-2 — second\n\n**Valid:** dated {today}\n\nprose here\n\n")
        );
        let declared =
            render_pending_section("U-2", 2, &section_of("**Valid:** dated 2026-01-02\n\nx"))
                .unwrap();
        assert_eq!(
            declared,
            "## U-2 — second\n\n**Valid:** dated 2026-01-02\n\nx\n\n"
        );
        let malformed = render_pending_section("U-2", 2, &section_of("**Valid:** dated nope\n\nx"));
        assert!(
            malformed.is_err(),
            "a malformed declaration is refused, not double-stamped"
        );
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

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None)
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

        let id = append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None)
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

        let err = append_entry(&mut cat, "art1", "bugs", "B", json!({}), &[], None).unwrap_err();
        assert!(err.to_string().contains("failures"));
    }

    #[test]
    fn append_entry_rejects_missing_augmentation() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();

        let err =
            append_entry(&mut cat, "art1", "failures", "F", json!({}), &[], None).unwrap_err();
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
            None,
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
            None,
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
            None,
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
            None,
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

        // Both connections are opened HERE, one after the other, and only the appends race. Opening
        // is setup, not the property under test: `Catalog::open` runs schema and trigger DDL, with
        // fsync'd commits, under the write lock, so two opens racing inside the 5 s busy_timeout
        // fail with "applying schema: database is locked" once the host's commits are slow
        // (docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md).
        // The barrier releases both appends together, which maximises the chance they overlap
        // (it cannot force an interleaving).
        let mut cat1 = Catalog::open(&path).unwrap();
        let mut cat2 = Catalog::open(&path).unwrap();
        // The WAL file is deleted when the setup block's connection closes, and the first write
        // after that recreates it with three fsyncs under the write lock (wal, directory, wal).
        // Traces also show a directory fsync on each connection's own first write. Nothing above
        // writes now that an up-to-date open takes no write lock, so write once on each connection
        // here: this test measures the contention of two appends, not those cold starts. With 3 s
        // injected per fsync, warming only cat1 left the second test failing 2 of 2 and warming
        // both passed 2 of 2 (see
        // docs/issues/2026-10-29-two-concurrent-catalog-writes-exceed-the-5-s-busy-timeout-when-fsync-takes-3-s.md).
        art_upsert(&cat1, &sample_art("warm")).unwrap();
        art_upsert(&cat2, &sample_art("warm2")).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b1 = barrier.clone();
        let h1 = std::thread::spawn(move || {
            b1.wait();
            append_entry(
                &mut cat1,
                "art1",
                "failures",
                "F",
                json!({"who": "one"}),
                &[],
                None,
            )
            .unwrap()
        });
        let h2 = std::thread::spawn(move || {
            barrier.wait();
            append_entry(
                &mut cat2,
                "art1",
                "failures",
                "F",
                json!({"who": "two"}),
                &[],
                None,
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
        // The template exactly as shipped, with nothing prepended: its own frontmatter must declare
        // F and W, or the first allocation below is refused on a fresh copy (bug 4d25c5b2). A
        // fixture that supplied the declaration itself would pass whatever the template said.
        std::fs::write(&md, &template_body).unwrap();

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
        // draft of this error prescribed `doc(action="augment", merge=true, …)`, which
        // refuses with "call doc(action=\"augment\") first" exactly when no augmentation
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
            !text.contains("doc(action=\"augment\""),
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

        // Both connections are opened HERE, before the race: `Catalog::open` is setup and fails with
        // "applying schema: database is locked" when two opens collide on a slow host (see
        // `append_entry_serializes_across_independent_connections_to_same_file`). Only the
        // allocations race, released together by the barrier.
        let mut cat1 = Catalog::open(&db).unwrap();
        let mut cat2 = Catalog::open(&db).unwrap();
        // Write once on each connection before the race; see
        // `append_entry_serializes_across_independent_connections_to_same_file`.
        art_upsert(&cat1, &sample_art("warm")).unwrap();
        art_upsert(&cat2, &sample_art("warm2")).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b1 = barrier.clone();
        let h1 = std::thread::spawn(move || {
            b1.wait();
            allocate_entry_id(&mut cat1, "art1", "R", None).unwrap().id
        });
        let h2 = std::thread::spawn(move || {
            barrier.wait();
            allocate_entry_id(&mut cat2, "art1", "R", None).unwrap().id
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
            // FLUSH items — zero extra indentation — are valid YAML, are the style
            // `serde_yml` emits, and are the form 5 of this repo's 45 `entry_prefix`
            // ledgers use on disk, including the busiest (`bug-fix-session-log.md`).
            // Every block-sequence fixture above is INDENTED; that omission is what let
            // `declared_entry_prefixes` ship a branch that breaks on the first flush
            // item and returns `[]`, so `append_entry`'s cross-host high-water collision
            // guard and the librarian write guard both silently skipped those five
            // files. Indent these two and the hole reopens with the suite still green.
            (
                "flush block sequence",
                "---\nkind: tracker\nentry_prefix:\n- F\n- W\n---\n\n# L\n",
            ),
            // Pairs with the case above: the fix deletes the indentation test, so this
            // pins that the ITEM test (`- ` absent ends the sequence) is what keeps a
            // sibling key from being swallowed as a list item.
            (
                "flush sequence then sibling key",
                "---\nkind: tracker\nentry_prefix:\n- F\n- W\nentry_high_water_F: 3\n---\n\n# L\n",
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
            // UNCITABLE declarations — the population the thirteen fixtures above never
            // reached, so the length and case rules were exercised by nothing and
            // `DCTX` shipped as a live ledger both readers disagreed about
            // (docs/issues/archive/2026-09-21-a-four-letter-entry-prefix-allocates-but-cannot-be-cited.md).
            // Each value is one the entry-token grammar `[A-Z]{1,3}-\d+` cannot express;
            // shorten `DCTX` to three letters or uppercase `r` and the case stops
            // discriminating.
            (
                "four letters",
                "---\nkind: tracker\nentry_prefix: DCTX\n---\n\n# L\n",
            ),
            // Mixed list: the citable member must survive, so a reader that drops the
            // whole list on one bad member is caught too.
            (
                "four letters beside a citable one",
                "---\nkind: tracker\nentry_prefix: [DCTX, DWF]\n---\n\n# L\n",
            ),
            (
                "lowercase",
                "---\nkind: tracker\nentry_prefix: r\n---\n\n# L\n",
            ),
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

    /// Direction (3) of
    /// `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`:
    /// the section and its index row land in ONE file write, closing the interval in
    /// which the ledger holds an entry that no row names.
    ///
    /// Measured 2026-09-07: 21 of 49 guarded ledgers keep an entry-row table, so the
    /// cheaper "stop instructing the row" direction is a loss for 43% of them.
    ///
    /// LOAD-BEARING: the fixture's table carries `| F-1 | first |` AND a `## F-1`
    /// section, so both of the allocator's body inputs agree on the maximum. If only
    /// one carried it, this test would also be asserting which input wins, which is a
    /// different question with its own tests.
    #[test]
    fn the_section_and_its_index_row_land_in_one_write() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n## Index\n\n\
             | ID | Title |\n|----|-------|\n| F-1 | first |\n\n\
             ## F-1 — first\n\nbody\n\n## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let section = PendingSection {
            title: "second".to_string(),
            body: "the prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: Some(PendingIndexRow {
                row: "| {id} | second |".to_string(),
                after_line: RowAnchor::Explicit("|----|-------|".to_string()),
            }),
        };
        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&section)).unwrap();
        assert_eq!(out.id, "F-2");

        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("## F-2 — second"),
            "the section must still be written: {text}"
        );
        assert!(
            text.contains("| F-2 | second |"),
            "the row must be written with `{{id}}` substituted: {text}"
        );
        // Discriminator: kills an implementation that inserts the template verbatim,
        // which would satisfy "a row was written" without the id ever resolving.
        assert!(
            !text.contains("{id}"),
            "the placeholder must not survive into the file: {text}"
        );
        // Placement is explicit, never inferred — same law as `anchor_heading`
        // (docs/adrs/2026-07-10-repair-and-continue-input-handling.md).
        let sep = text.find("|----|-------|").unwrap();
        let new_row = text.find("| F-2 | second |").unwrap();
        let old_row = text.find("| F-1 | first |").unwrap();
        assert!(
            sep < new_row && new_row < old_row,
            "the row must land immediately after its anchor line, above existing rows: {text}"
        );
    }

    /// A missing row anchor must refuse BEFORE anything is written — the same
    /// discipline `anchor_heading` already has. Otherwise a caller who mistypes the
    /// separator burns an id AND leaves a half-written ledger.
    ///
    /// The retry is the discriminator: an implementation that writes the section,
    /// fails on the row, and lets the transaction commit would issue F-3 on the
    /// second call. Asserting only "it errored" is monotone under exactly that bug.
    #[test]
    fn a_missing_index_row_anchor_writes_nothing_and_allocates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let before = "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
                      | ID | Title |\n|----|-------|\n| F-1 | first |\n\n\
                      ## F-1 — first\n\nbody\n\n## Template for new entries\n\nboilerplate\n";
        std::fs::write(&md, before).unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(&cat, &art).unwrap();

        let bad = PendingSection {
            title: "never lands".to_string(),
            body: "x".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: Some(PendingIndexRow {
                row: "| {id} | never |".to_string(),
                after_line: RowAnchor::Explicit("| NO SUCH SEPARATOR |".to_string()),
            }),
        };
        let err = allocate_entry_id(&mut cat, "art1", "F", Some(&bad)).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("index row"),
            "the refusal must name WHICH anchor failed — the section anchor was fine: {text}"
        );

        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "the file must be byte-identical: the section must not land without its row"
        );

        let good = PendingSection {
            index_row: None,
            ..bad.clone()
        };
        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&good)).unwrap();
        assert_eq!(
            out.id, "F-2",
            "the refused call must not have burned an id — F-3 here means the \
             transaction committed through the error"
        );
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
            index_row: None,
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

    /// Arms a DETERMINISTIC `tx.commit()` failure on the production path.
    ///
    /// A deferred foreign-key violation is the one SQLite failure that surfaces at
    /// `COMMIT` rather than at the statement that caused it: every statement inside
    /// `append_entry` — including the `std::fs::write` of the section — succeeds, and
    /// only `tx.commit()` returns `Err`. The bug is reported as a lock race, and
    /// `append_entry` cannot tell the two causes apart; this one carries no timing, so
    /// it is a test rather than a flake.
    ///
    /// LOAD-BEARING: the trigger must fire on `artifact_augmentation`, the table
    /// `append_entry` UPDATEs inside its transaction. Retarget it at a table this path
    /// does not write and the commit SUCCEEDS — every assertion below then passes while
    /// testing nothing at all.
    fn arm_commit_tripwire(cat: &Catalog) {
        cat.conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER UPDATE ON artifact_augmentation
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();
    }

    fn disarm_commit_tripwire(cat: &Catalog) {
        cat.conn
            .execute_batch("DROP TRIGGER commit_tripwire_trg; DROP TABLE commit_tripwire;")
            .unwrap();
    }

    /// Sets up a ledger artifact with an empty `failures` collection and its file on
    /// disk, returning the file's path and its exact pre-call bytes.
    fn ledger_fixture(dir: &std::path::Path, cat: &mut Catalog) -> (std::path::PathBuf, String) {
        ledger_fixture_named(dir, cat, "ledger.md")
    }

    /// [`ledger_fixture`] with the ledger's file named `name`.
    fn ledger_fixture_named(
        dir: &std::path::Path,
        cat: &mut Catalog,
        name: &str,
    ) -> (std::path::PathBuf, String) {
        let md = dir.join(name);
        let before = "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
                      ## Template for new entries\n\nboilerplate\n"
            .to_string();
        std::fs::write(&md, &before).unwrap();
        let mut art = sample_art("art1");
        art.abs_path = md.clone();
        art_upsert(cat, &art).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(cat, &a).unwrap();
        (md, before)
    }

    fn pending(title: &str) -> PendingSection {
        PendingSection {
            title: title.to_string(),
            body: "prose".to_string(),
            anchor_heading: "## Template for new entries".to_string(),
            index_row: None,
        }
    }

    /// Regression: `docs/issues/archive/2026-09-11-doc-update-writes-the-file-then-fails-the-catalog-and-reports-only-the-failure.md`
    ///
    /// The section is written BEFORE `tx.commit()` deliberately — a splice failure then
    /// rolls the transaction back and the refusal's "nothing was written" is true. That
    /// argument covers a failure of the SPLICE and is silent about a failure of the
    /// COMMIT, which is the one failure that happens AFTER the bytes reach disk.
    #[test]
    fn a_commit_that_fails_after_the_section_write_leaves_the_file_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, before) = ledger_fixture(dir.path(), &mut cat);
        arm_commit_tripwire(&cat);

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&pending("first")),
        )
        .unwrap_err();

        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "a commit that failed rolled the catalog back, so a file still carrying the \
             section is carrying an entry whose id was never persisted. Error was: {err}"
        );
    }

    /// The consequence the restore prevents, asserted end to end. `append_entry` folds
    /// the BODY's max into the next id (`append_entry_skips_ids_already_claimed_by_the_body`),
    /// so an orphaned `## F-1` left on disk pushes the retry to `F-2` and writes a
    /// SECOND section — two entries on disk for one logical append, with params naming
    /// only the second.
    #[test]
    fn a_retry_after_a_failed_commit_allocates_the_same_id_not_a_second_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, _before) = ledger_fixture(dir.path(), &mut cat);
        arm_commit_tripwire(&cat);

        append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&pending("first")),
        )
        .unwrap_err();
        disarm_commit_tripwire(&cat);

        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&pending("first")),
        )
        .unwrap();

        assert_eq!(
            out.id, "F-1",
            "the failed attempt persisted no id, so the retry must allocate the FIRST one"
        );
        let text = std::fs::read_to_string(&md).unwrap();
        assert_eq!(
            text.matches("## F-").count(),
            1,
            "exactly one section for one logical append; a count, not a `contains`, \
             because the defect is a SECOND section and `contains` is satisfied by both: {text}"
        );
    }
    // ---- a retry finds the section it already wrote ------------------------------------------
    //
    // The oracle for every test below is the FILE and the catalog's own tables, read back; the
    // response's `id` alone would be satisfied by the defect (a second section under a new id).

    /// A ledger whose file already holds `## F-1 — first` with the body `prose` (as a failed
    /// call left it), and whose catalog records nothing: params empty, no reservation. `mark`
    /// is the committed `entry_high_water_F`, when the file has one.
    fn orphan_fixture(
        dir: &std::path::Path,
        cat: &mut Catalog,
        mark: Option<u64>,
    ) -> std::path::PathBuf {
        let (md, _) = ledger_fixture(dir, cat);
        let mark_line = mark
            .map(|m| format!("entry_high_water_F: {m}\n"))
            .unwrap_or_default();
        std::fs::write(
            &md,
            format!(
                "---\nkind: tracker\nentry_prefix: F\n{mark_line}---\n\n# Ledger\n\n\
                 ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
                 ## Template for new entries\n\nboilerplate\n"
            ),
        )
        .unwrap();
        md
    }

    fn params_ids(cat: &Catalog, artifact_id: &str) -> Vec<String> {
        let row = get(cat, artifact_id).unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        params["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn reserved_max(cat: &Catalog, artifact_id: &str, prefix: &str) -> Option<i64> {
        cat.conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id = ?1 AND prefix = ?2",
                rusqlite::params![artifact_id, prefix],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
    }

    fn append_first(cat: &mut Catalog, section: &PendingSection) -> AppendOutcome {
        append_entry(cat, "art1", "failures", "F", json!({}), &[], Some(section)).unwrap()
    }

    fn different_body(title: &str) -> PendingSection {
        PendingSection {
            body: "other prose".to_string(),
            ..pending(title)
        }
    }

    #[test]
    fn a_params_retry_adopts_a_section_the_file_has_and_params_lack() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, Some(1));
        let before = std::fs::read_to_string(&md).unwrap();

        let out = append_first(&mut cat, &pending("first"));

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        assert!(out.section_written, "the section is on disk: {out:?}");
        assert!(
            out.warning.is_none(),
            "nothing was renumbered around the body, so there is no collision to warn about: {out:?}"
        );
        let text = std::fs::read_to_string(&md).unwrap();
        assert_eq!(text.matches("## F-").count(), 1, "{text}");
        assert_eq!(
            text, before,
            "the committed mark already holds 1, so an adoption has nothing to write"
        );
        assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
    }
    // mutation: skip the adoption lookup on the params path → F-2, two sections.
    // mutation: drop `&& !adopted` from the `warning` filter → the `warning.is_none()` assertion
    // red.

    // ---- the params path writes the entry into the markdown ----------------------------------
    //
    // Oracles: the parse oracle (`parse_document` on the bytes read back from disk) for what the
    // file defines, and the FILE itself for the mark; the response's `id` alone would be
    // satisfied by a params-only row.

    /// Oracle: parse.
    #[test]
    fn a_params_append_writes_the_section_and_raises_the_mark() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, _) = ledger_fixture(dir.path(), &mut cat);

        let out = append_first(&mut cat, &pending("first"));

        assert_eq!(out.id, "F-1");
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("entry_high_water_F: 1"),
            "the entry is under the COMMITTED mark, the one input that survives a clone: {text}"
        );
        let parsed = crate::librarian::derived::build::parse_document(
            "docs/ledger.md",
            &text,
            &crate::librarian::classify::default_rules().unwrap(),
        );
        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|e| e.token.as_str())
                .collect::<Vec<_>>(),
            vec![out.id.as_str()],
            "the markdown defines exactly the token the call returned: {text}"
        );
        assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
    }
    // mutation: leave `write_mark: false` in the params path's `write_entry_into_ledger` call →
    // the `entry_high_water_F: 1` assertion is red.

    /// The mark leads params and body on a fresh clone of a compacted ledger.
    #[test]
    fn the_committed_mark_counts_on_the_params_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, _) = ledger_fixture(dir.path(), &mut cat);
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\nentry_high_water_F: 7\n---\n\n# Ledger\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();

        let out = append_first(&mut cat, &pending("first"));

        assert_eq!(
            out.id, "F-8",
            "params are empty and no heading claims an id; only the committed mark says 7 \
             were handed out"
        );
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("entry_high_water_F: 8"), "{text}");
        assert!(text.contains("## F-8 — first"), "{text}");
    }
    // mutation: drop `.max(committed_mark.map_or(0, |m| m + 1))` from `next` → F-1.

    /// A section write records the mark, so a document with no frontmatter block to hold it is
    /// refused with nothing written and nothing allocated.
    #[test]
    fn a_params_ledger_with_no_frontmatter_is_refused_with_a_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, _) = ledger_fixture(dir.path(), &mut cat);
        let before = "# Ledger\n\n## Template for new entries\n\nboilerplate\n";
        std::fs::write(&md, before).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&pending("first")),
        )
        .unwrap_err();

        let text = format!("{err:#}");
        assert!(
            text.contains("no frontmatter block") && text.contains("entry_high_water_F"),
            "the refusal names what the document lacks: {text}"
        );
        let hint = err
            .downcast_ref::<LibrarianRecoverableError>()
            .expect("a recoverable refusal")
            .hint
            .clone()
            .unwrap_or_default();
        assert!(
            hint.contains("Add a `---` frontmatter block at the top of the file")
                && !hint.contains("entry_prefix"),
            "a params ledger is a ledger by its sidecar, not by an `entry_prefix`, so the hint \
             may not claim the missing block carries one: {hint}"
        );
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "nothing reached the file"
        );
        assert!(params_ids(&cat, "art1").is_empty(), "nothing was allocated");
    }
    // mutation: pass `write_mark: false` → the call is Ok and the section is written.
    // mutation: restore the old hint wording in `write_entry_into_ledger` ("... the same block
    // already carries the `entry_prefix` declaration that made this a ledger") → the hint
    // assertion is red.

    /// An adoption whose committed mark is below the adopted id brings the mark up, in a
    /// mark-only write (no second section).
    #[test]
    fn a_params_adoption_raises_a_committed_mark_that_is_below_the_adopted_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, None);

        let out = append_first(&mut cat, &pending("first"));

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("entry_high_water_F: 1\n"),
            "an adopted id must be under the committed mark, or a fresh clone reissues it: {text}"
        );
        assert_eq!(
            text.matches("## F-").count(),
            1,
            "a mark-only write: no second section: {text}"
        );
        assert_eq!(params_ids(&cat, "art1"), vec!["F-1"]);
    }
    // mutation: change the write guard to `if !adopted` → the file keeps no mark.

    /// The mark-only write of an adoption that then fails to commit is put back, so the retry
    /// finds the same orphan and adopts it again.
    #[test]
    fn a_params_adoption_whose_commit_fails_restores_the_file_and_the_retry_adopts_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, None);
        let before = std::fs::read_to_string(&md).unwrap();
        arm_commit_tripwire(&cat);

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &[],
            Some(&pending("first")),
        )
        .unwrap_err();

        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "the mark-only write is restored with the rolled-back row: {err:#}"
        );
        disarm_commit_tripwire(&cat);
        let out = append_first(&mut cat, &pending("first"));
        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
    }
    // mutation: pass `None` instead of `written_section.as_ref()` to `commit_after_file_write` →
    // the file keeps the mark the rolled-back call wrote and the equality assertion is red.

    /// A temp repository (a `.git` directory makes it one for `lookup_git_root`) holding the
    /// params ledger `docs/<ledger>` (artifact `art1`), the document `docs/other.md`
    /// (`0123456789abcdef`), and `docs/target.md` (`fedcba9876543210`, slug `target-slug`, entry
    /// `F-3` in its params). Returns the ledger's path.
    fn cite_repo(dir: &std::path::Path, cat: &mut Catalog, ledger: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let docs = dir.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        let (md, _) = ledger_fixture_named(&docs, cat, ledger);
        plain_doc(cat, "0123456789abcdef", &docs.join("other.md"), None);
        plain_doc(
            cat,
            "fedcba9876543210",
            &docs.join("target.md"),
            Some("target-slug"),
        );
        let mut a = aug("fedcba9876543210");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[{"id":"F-3"}]}"#.to_string();
        upsert(cat, &a).unwrap();
        md
    }

    /// A one-line markdown file at `path` and its catalog row; `slug` is set when given.
    fn plain_doc(cat: &Catalog, id: &str, path: &std::path::Path, slug: Option<&str>) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "# Doc\n").unwrap();
        art_upsert(
            cat,
            &TestArtifactRowBuilder::new(id).with_abs_path(path).build(),
        )
        .unwrap();
        if let Some(slug) = slug {
            cat.conn
                .execute(
                    "UPDATE artifact SET slug = ?1 WHERE id = ?2",
                    rusqlite::params![slug, id],
                )
                .unwrap();
        }
    }

    /// The `origin='write'` `entry_cite` rows of entry `local`.
    fn write_rows(cat: &Catalog, local: &str) -> i64 {
        cat.conn
            .query_row(
                "SELECT COUNT(*) FROM entry_cite WHERE src_local = ?1 AND origin = 'write'",
                [local],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn cited(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_params_adoption_writes_the_cites_for_the_adopted_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        // The orphan a failed call with this cite left: the section AND its `**Cites:**` line.
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
             ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
             **Cites:** [[other]]\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();

        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["0123456789abcdef"]),
            Some(&pending("first")),
        )
        .unwrap();

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        assert_eq!(out.cites, vec!["[[other]]"]);
        let src_local: String = cat
            .conn
            .query_row(
                "SELECT src_local FROM entry_cite WHERE rel = 'cites'",
                [],
                |r| r.get(0),
            )
            .expect("the adopted entry's citation is recorded under its own id");
        assert_eq!(src_local, "F-1");
    }
    // mutation: skip the `entry_cite` insert when the section is adopted → the query finds no row.

    /// The adoption key is the RENDERED body: an orphan written without the cites line is a
    /// different entry from one that carries them, so the retry numbers a new section.
    #[test]
    fn an_orphan_without_the_cites_line_is_not_adopted_by_a_call_that_cites() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
             ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();

        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["0123456789abcdef"]),
            Some(&pending("first")),
        )
        .unwrap();

        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("## F-2 — first") && text.contains("**Cites:** [[other]]"),
            "{text}"
        );
    }
    // mutation: compare the adoption against the caller's un-rendered body (`s.body` instead of
    // the rendered section's) → the orphan is adopted as F-1 and the assertion is red.

    /// Oracle: parse.
    #[test]
    fn params_cites_are_grammar_text_and_write_rows() {
        use crate::librarian::identity::grammar::Reference;
        use crate::librarian::identity::key::Key;
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");

        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["docs/other.md", "target-slug:F-3", "docs/other.md"]),
            Some(&pending("first")),
        )
        .unwrap();

        let text = std::fs::read_to_string(&md).unwrap();
        let parsed = crate::librarian::derived::build::parse_document(
            "docs/ledger.md",
            &text,
            &crate::librarian::classify::default_rules().unwrap(),
        );
        assert!(
            parsed.citations.iter().any(|c| c.reference
                == Reference::File {
                    repo: None,
                    key: Key::parse("other").unwrap()
                }),
            "{text}"
        );
        assert!(
            parsed.citations.iter().any(|c| c.reference
                == Reference::Entry {
                    repo: None,
                    key: Key::parse("target").unwrap(),
                    token: "F-3".into()
                }),
            "{text}"
        );
        assert!(
            text.contains("prose\n\n**Cites:** [[other]], [[target:F-3]]\n"),
            "one line at the end of the section, input order, the repeat dropped: {text}"
        );
        let entry = parsed
            .entries
            .iter()
            .find(|e| e.token == out.id)
            .expect("the entry's section is defined");
        assert_eq!(parsed.citations.len(), 2, "{text}");
        for c in &parsed.citations {
            assert!(
                (entry.heading_line..=entry.end_line).contains(&c.line),
                "the cites line lies inside {}'s section ({}..={}), not after it: {text}",
                out.id,
                entry.heading_line,
                entry.end_line
            );
        }
        assert_eq!(write_rows(&cat, &out.id), 2);
        assert_eq!(out.cites, vec!["[[other]]", "[[target:F-3]]"]);
        assert_eq!(out.key_note, None, "docs/ledger.md is a canonical key");
    }
    // mutation: skip `with_cites` (`rendered = section.cloned()`) → no citation parses.
    // mutation: `with_cites` puts a `# Other` heading before the body → the section-bounds
    // assertion is red (the text assertion still passes).

    /// Review Focus 3. Oracle: the file's bytes and the catalog's params.
    #[test]
    fn a_cite_that_names_a_shared_key_aborts_the_whole_append() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        plain_doc(
            &cat,
            "1111111111111111",
            &dir.path().join("docs/a/notes.md"),
            None,
        );
        plain_doc(
            &cat,
            "2222222222222222",
            &dir.path().join("docs/b/notes.md"),
            None,
        );
        let before = std::fs::read_to_string(&md).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["docs/other.md", "docs/a/notes.md"]),
            Some(&pending("first")),
        )
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("docs/a/notes.md") && msg.contains("docs/b/notes.md"),
            "{msg}"
        );
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert!(params_ids(&cat, "art1").is_empty());
        assert_eq!(
            write_rows(&cat, "F-1"),
            0,
            "no cite row of the good ref either"
        );
    }
    // mutation: swallow `grammar_form_of`'s error in `resolve_cites` (`.unwrap_or_default()`) → the
    // call is Ok and `unwrap_err` panics. The file and params halves hold by construction: every
    // refusal precedes the write.

    #[test]
    fn a_cite_outside_the_key_space_aborts_the_whole_append() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/key-space.toml"),
            "exclude = [\"contrib/**\"]\n",
        )
        .unwrap();
        plain_doc(
            &cat,
            "3333333333333333",
            &dir.path().join("contrib/x.md"),
            None,
        );
        let before = std::fs::read_to_string(&md).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["docs/other.md", "contrib/x.md"]),
            Some(&pending("first")),
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("key-space.toml"), "{err:#}");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert!(params_ids(&cat, "art1").is_empty());
    }
    // mutation: same as above (swallow `grammar_form_of`'s error) → the call is Ok.

    /// A cite into another repository is `[[repo/key]]` from that repository's committed name.
    #[test]
    fn a_cite_into_a_named_other_repository_is_qualified_by_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let other = dir.path().join("elsewhere");
        std::fs::create_dir_all(other.join(".git")).unwrap();
        std::fs::create_dir_all(other.join(".codescout")).unwrap();
        std::fs::write(
            other.join(".codescout/repo.toml"),
            "name = \"other-repo\"\n",
        )
        .unwrap();
        plain_doc(&cat, "4444444444444444", &other.join("docs/notes.md"), None);

        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["4444444444444444"]),
            Some(&pending("first")),
        )
        .unwrap();

        assert_eq!(out.cites, vec!["[[other-repo/notes]]"]);
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("**Cites:** [[other-repo/notes]]"), "{text}");
        assert_eq!(write_rows(&cat, &out.id), 1);
    }
    // mutation: same as above (`if true` in `grammar_form_of`) → `[[notes]]`, the assertion is red.

    /// C1-R15: a target repository with no committed name refuses the whole append, naming the
    /// config file to add and no path.
    #[test]
    fn a_cite_into_an_unnamed_other_repository_aborts_the_append_naming_repo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let other = dir.path().join("elsewhere");
        std::fs::create_dir_all(other.join(".git")).unwrap();
        plain_doc(&cat, "4444444444444444", &other.join("docs/notes.md"), None);
        let before = std::fs::read_to_string(&md).unwrap();

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["4444444444444444"]),
            Some(&pending("first")),
        )
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(msg.contains(".codescout/repo.toml"), "{msg}");
        assert!(
            !msg.contains(&dir.path().display().to_string()) && !msg.contains("4444444444444444"),
            "no absolute path and no id: {msg}"
        );
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert!(params_ids(&cat, "art1").is_empty());
    }
    // mutation: in `grammar_form_of`, take the same-repository branch always (`if true`) → the
    // cite is written unqualified and the call is Ok (and the named case loses its prefix).

    /// A ledger that lies in no git repository has no repository to name its cites against.
    #[test]
    fn a_cite_from_a_ledger_in_no_repository_is_refused_by_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let (md, before) = ledger_fixture(dir.path(), &mut cat);
        // The target is in a repository; the ledger is not.
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        plain_doc(&cat, "0123456789abcdef", &repo.join("docs/other.md"), None);

        let err = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["0123456789abcdef"]),
            Some(&pending("first")),
        )
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("ledger.md")
                && msg.contains("no git repository")
                && msg.contains("retry without `cites`"),
            "{msg}"
        );
        assert!(!msg.contains(&dir.path().display().to_string()), "{msg}");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
    }
    // mutation: in `resolve_cites`, take the text path only when the ledger has a git root (a
    // `match` guard) → the call is Ok with no forms.

    /// Oracle: commit. The text the call wrote resolves, in the derived index built from a
    /// commit, to the entry and the file it cites.
    #[test]
    fn a_dual_written_entry_resolves_in_the_derived_index_after_a_commit() {
        use crate::librarian::derived::testkit::{build_files, edges_from, entry_tokens};
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let out = append_entry(
            &mut cat,
            "art1",
            "failures",
            "F",
            json!({}),
            &cited(&["docs/other.md", "target-slug:F-3"]),
            Some(&pending("first")),
        )
        .unwrap();
        let ledger_text = std::fs::read_to_string(&md).unwrap();

        let b = build_files(&[
            ("docs/ledger.md", &ledger_text),
            ("docs/target.md", "## F-3 — t\n"),
            ("docs/other.md", "# O\n"),
        ]);

        assert_eq!(
            entry_tokens(&b.conn, "docs/ledger.md"),
            vec![out.id.clone()]
        );
        let edges = edges_from(&b.conn, "docs/ledger.md");
        assert!(
            edges.contains(&(
                "entry".into(),
                None,
                Some("target".into()),
                Some("F-3".into())
            )),
            "{edges:?}"
        );
        assert!(
            edges.contains(&("file".into(), None, Some("other".into()), None)),
            "{edges:?}"
        );
    }
    // mutation: wrap the `cites_line` text in an HTML comment → no edges.

    #[test]
    fn a_ledger_whose_own_name_is_not_a_key_still_takes_the_entry_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "Ledger_One.md");

        let out = append_first(&mut cat, &pending("first"));

        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("## F-1 — first"), "{text}");
        let note = out.key_note.expect("the ledger's name is not a key");
        assert!(
            note.contains("Ledger_One.md") && note.contains("F-1"),
            "{note}"
        );
        assert!(
            !note.contains(&dir.path().display().to_string()),
            "a file name, not a path: {note}"
        );
    }
    // mutation: never set `key_note` → `expect` panics.

    // ---- the prose path writes `cites` too ----------------------------------------------------
    //
    // The ledger is the `cite_repo` fixture (a real `.git`; its params row is never read by the
    // prose path), driven through `allocate_entry_id_with_cites`. Oracles: the parse oracle on the
    // bytes read back, the commit oracle for resolved edges, and the file and the catalog's own
    // tables for what a refusal must leave untouched.

    /// Oracle: parse.
    #[test]
    fn prose_cites_are_grammar_text_and_write_rows() {
        use crate::librarian::identity::grammar::Reference;
        use crate::librarian::identity::key::Key;
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");

        let out = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["docs/other.md", "target-slug:F-3", "docs/other.md"]),
        )
        .unwrap();

        assert_eq!(out.id, "F-1");
        assert_eq!(out.cites, vec!["[[other]]", "[[target:F-3]]"]);
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("prose\n\n**Cites:** [[other]], [[target:F-3]]\n"),
            "one line at the end of the section, input order, the repeat dropped: {text}"
        );
        let parsed = crate::librarian::derived::build::parse_document(
            "docs/ledger.md",
            &text,
            &crate::librarian::classify::default_rules().unwrap(),
        );
        assert!(
            parsed.citations.iter().any(|c| c.reference
                == Reference::File {
                    repo: None,
                    key: Key::parse("other").unwrap()
                }),
            "{text}"
        );
        assert!(
            parsed.citations.iter().any(|c| c.reference
                == Reference::Entry {
                    repo: None,
                    key: Key::parse("target").unwrap(),
                    token: "F-3".into()
                }),
            "{text}"
        );
        let entry = parsed
            .entries
            .iter()
            .find(|e| e.token == out.id)
            .expect("the entry's section is defined");
        assert_eq!(parsed.citations.len(), 2, "{text}");
        for c in &parsed.citations {
            assert!(
                (entry.heading_line..=entry.end_line).contains(&c.line),
                "the cites line lies inside {}'s section, not after it: {text}",
                out.id
            );
        }
        assert_eq!(
            write_rows(&cat, &out.id),
            2,
            "the prose path writes the same origin='write' rows as the params path"
        );
        assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
        assert!(text.contains("entry_high_water_F: 1"), "{text}");
    }
    // mutation: skip `with_cites` on the prose path (`rendered = section.cloned()`) → the text
    // assertion is red (`out.cites` still lists the forms).
    // mutation: skip the `entry_cite` insert on the prose path → `write_rows` is 0.

    /// Oracle: the file's bytes and the catalog's reservation table.
    #[test]
    fn prose_cites_without_a_section_are_refused_and_reserve_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let before = std::fs::read_to_string(&md).unwrap();

        let err =
            allocate_entry_id_with_cites(&mut cat, "art1", "F", None, &cited(&["docs/other.md"]))
                .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("`title`") && msg.contains("`body`") && msg.contains("`anchor_heading`"),
            "the refusal names the fields that give the cites a place to land: {msg}"
        );
        assert!(
            !msg.contains(&dir.path().display().to_string()) && !msg.contains("0123456789abcdef"),
            "no path and no id: {msg}"
        );
        assert_eq!(reserved_max(&cat, "art1", "F"), None, "nothing reserved");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert_eq!(write_rows(&cat, "F-1"), 0);
    }
    // mutation: drop the no-section refusal → Ok with an id and no text (`unwrap_err` panics).

    /// A cite the grammar cannot name refuses the whole call, the good cite before it included.
    /// Oracle: the file's bytes, the reservation table and the `entry_cite` rows.
    #[test]
    fn a_prose_cite_of_a_non_canonical_name_aborts_and_reserves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        plain_doc(
            &cat,
            "5555555555555555",
            &dir.path().join("docs/Bad_Name.md"),
            None,
        );
        let before = std::fs::read_to_string(&md).unwrap();

        let err = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["docs/other.md", "docs/Bad_Name.md"]),
        )
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(msg.contains("Bad_Name.md"), "{msg}");
        assert!(
            !msg.contains(&dir.path().display().to_string()) && !msg.contains("5555555555555555"),
            "no path and no id: {msg}"
        );
        assert_eq!(reserved_max(&cat, "art1", "F"), None, "nothing reserved");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert_eq!(write_rows(&cat, "F-1"), 0, "no row of the good cite either");
    }
    // mutation: swallow `grammar_form_of`'s error in `resolve_cites` (`.unwrap_or_default()`) → the
    // call is Ok and `unwrap_err` panics. The brief's mutation (stage the reservation before the
    // cites resolve and commit it on error) is unreachable through this function: an `Err` return
    // drops the transaction, which rolls the reservation back.

    /// Oracle: commit. The text the prose call wrote resolves, in the derived index built from a
    /// commit, to the entry it defines and to the file and the entry it cites.
    #[test]
    fn a_dual_written_prose_entry_resolves_in_the_derived_index_after_a_commit() {
        use crate::librarian::derived::testkit::{build_files, edges_from, entry_tokens};
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let out = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["docs/other.md", "target-slug:F-3"]),
        )
        .unwrap();
        let ledger_text = std::fs::read_to_string(&md).unwrap();

        let b = build_files(&[
            ("docs/ledger.md", &ledger_text),
            ("docs/target.md", "## F-3 — t\n"),
            ("docs/other.md", "# O\n"),
        ]);

        assert_eq!(
            entry_tokens(&b.conn, "docs/ledger.md"),
            vec![out.id.clone()]
        );
        let edges = edges_from(&b.conn, "docs/ledger.md");
        assert!(
            edges.contains(&(
                "entry".into(),
                None,
                Some("target".into()),
                Some("F-3".into())
            )),
            "{edges:?}"
        );
        assert!(
            edges.contains(&("file".into(), None, Some("other".into()), None)),
            "{edges:?}"
        );
    }
    // mutation: wrap the `cites_line` text in an HTML comment → no edges.

    /// A cite into another repository is `[[repo/key]]` on the prose path too: it shares
    /// `resolve_cites` with the params path. Oracle: the file's bytes and the `entry_cite` rows.
    #[test]
    fn a_prose_cite_into_a_named_other_repository_is_qualified_by_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let other = dir.path().join("elsewhere");
        std::fs::create_dir_all(other.join(".git")).unwrap();
        std::fs::create_dir_all(other.join(".codescout")).unwrap();
        std::fs::write(
            other.join(".codescout/repo.toml"),
            "name = \"other-repo\"\n",
        )
        .unwrap();
        plain_doc(&cat, "4444444444444444", &other.join("docs/notes.md"), None);

        let out = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["4444444444444444"]),
        )
        .unwrap();

        assert_eq!(out.cites, vec!["[[other-repo/notes]]"]);
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("**Cites:** [[other-repo/notes]]"), "{text}");
        assert_eq!(write_rows(&cat, &out.id), 1);
    }
    // mutation: in `grammar_form_of`, take the same-repository branch always (`if true`) →
    // `[[notes]]`, the assertion is red.

    /// The prose adoption key includes the `**Cites:**` line, as the params key does: a retry with
    /// the same title, body and cites adopts the orphan and records its cite rows. Oracle: the
    /// file's bytes (unchanged: the mark already covers the id) and the catalog's tables.
    #[test]
    fn a_prose_retry_with_the_same_title_body_and_cites_adopts_and_records_the_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\nentry_high_water_F: 1\n---\n\n# Ledger\n\n\
             ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
             **Cites:** [[other]]\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let before = std::fs::read_to_string(&md).unwrap();

        let out = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["0123456789abcdef"]),
        )
        .unwrap();

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        assert_eq!(out.cites, vec!["[[other]]"]);
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
        assert_eq!(write_rows(&cat, "F-1"), 1, "the adopted entry's cite row");
    }
    // mutation: skip the `entry_cite` insert when the section is adopted → `write_rows` is 0.

    /// Different cites are a different entry on the prose path, in both directions: an orphan with
    /// no cites line is not adopted by a call that cites, and an orphan that has one is not adopted
    /// by a call that does not. Oracle: the file's bytes.
    #[test]
    fn a_prose_orphan_is_adopted_only_by_a_call_with_the_same_cites() {
        // Orphan without the line, call with cites.
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
             ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let out = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["0123456789abcdef"]),
        )
        .unwrap();
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("## F-2 — first") && text.contains("**Cites:** [[other]]"),
            "{text}"
        );

        // Orphan with the line, call without cites.
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n\
             ## F-1 — first\n\n**Valid:** dated 2026-01-01\n\nprose\n\n\
             **Cites:** [[other]]\n\n\
             ## Template for new entries\n\nboilerplate\n",
        )
        .unwrap();
        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
    }
    // mutation: compare the prose adoption against the caller's un-rendered body (`s.body` instead
    // of the rendered section's) → the first half adopts F-1 and is red.

    /// A prose call that cites and whose COMMIT fails leaves the file and the catalog as they were:
    /// the section, its cites line and the mark are restored, and no `entry_cite` row survives the
    /// rollback. Oracle: the file's bytes and the catalog's tables.
    #[test]
    fn a_prose_commit_failure_after_a_cited_write_restores_the_file_and_keeps_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = cite_repo(dir.path(), &mut cat, "ledger.md");
        let before = std::fs::read_to_string(&md).unwrap();
        cat.conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON entry_reservation
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();

        let err = allocate_entry_id_with_cites(
            &mut cat,
            "art1",
            "F",
            Some(&pending("first")),
            &cited(&["docs/other.md"]),
        )
        .unwrap_err();

        let hint = err
            .downcast_ref::<LibrarianRecoverableError>()
            .expect("a recoverable error, so it routes isError:false")
            .hint
            .clone()
            .unwrap_or_default();
        assert!(hint.contains("safe to retry as-is"), "{hint}");
        assert_eq!(std::fs::read_to_string(&md).unwrap(), before);
        assert_eq!(write_rows(&cat, "F-1"), 0, "the rollback took the row");
        assert_eq!(reserved_max(&cat, "art1", "F"), None);
    }
    // mutation: pass `None` instead of `written.as_ref()` to `commit_after_file_write` on the prose
    // path → the file keeps the section and the cites line and the equality assertion is red.

    #[test]
    fn a_prose_retry_with_no_later_reservation_adopts_the_section() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, Some(1));
        let before = std::fs::read_to_string(&md).unwrap();

        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            before,
            "the committed mark already holds 1, so the adoption has nothing to write"
        );
    }
    // mutation: skip the adoption lookup on the prose path → F-2.

    #[test]
    fn a_prose_adoption_raises_a_committed_mark_that_is_below_the_adopted_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, None);

        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();

        assert_eq!((out.id.as_str(), out.adopted), ("F-1", true));
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(
            text.contains("entry_high_water_F: 1\n"),
            "an adopted id must be under the committed mark, or a fresh clone reissues it: {text}"
        );
        assert_eq!(
            text.matches("## F-").count(),
            1,
            "a mark-only write: no second section: {text}"
        );
        assert_eq!(reserved_max(&cat, "art1", "F"), Some(1));
        assert_eq!(
            crate::librarian::catalog::artifact::get(&cat, "art1")
                .unwrap()
                .unwrap()
                .file_sha256,
            crate::librarian::util::sha_of_bytes(text.as_bytes()),
            "the row records the bytes the adoption wrote"
        );
    }
    // mutation: skip the mark write when the adopted id is above the committed mark → the
    // `entry_high_water_F: 1` assertion red.

    #[test]
    fn a_different_body_under_the_same_title_is_a_new_entry() {
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        let md = orphan_fixture(dir.path(), &mut cat, None);
        let out = append_first(&mut cat, &different_body("first"));
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
        assert_eq!(
            std::fs::read_to_string(&md)
                .unwrap()
                .matches("## F-")
                .count(),
            2
        );

        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        orphan_fixture(dir.path(), &mut cat, Some(1));
        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&different_body("first"))).unwrap();
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
    }
    // mutation: `find_unrecorded_section` ignores the body comparison (titles only) → F-1 adopted,
    // red.

    /// An ADOPTED prose call whose own COMMIT fails leaves the adopted section in the file, so its
    /// hint is the delete-first remedy for that section, whether the call wrote nothing (the mark
    /// already covered the id) or wrote only the mark. "Safe to retry as-is" is false here: once a
    /// peer reserves a later id the retry no longer finds the orphan and writes a second section
    /// (C1-R22). The oracle is the hint text and the file bytes.
    #[test]
    fn an_adopted_prose_call_whose_commit_fails_keeps_the_delete_first_remedy() {
        for mark in [Some(1u64), None] {
            let dir = tempfile::tempdir().unwrap();
            let mut cat = Catalog::open_in_memory().unwrap();
            let md = orphan_fixture(dir.path(), &mut cat, mark);
            let before = std::fs::read_to_string(&md).unwrap();
            cat.conn
                .execute_batch(
                    "CREATE TABLE commit_tripwire(
                         missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                     );
                     CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON entry_reservation
                     BEGIN
                         INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                     END;",
                )
                .unwrap();

            let err =
                allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap_err();

            let hint = err
                .downcast_ref::<LibrarianRecoverableError>()
                .expect("a recoverable error, so it routes isError:false")
                .hint
                .clone()
                .unwrap_or_default();
            assert!(
                hint.contains("Delete the `F-1` section"),
                "mark {mark:?}: the adopted section is still in the file, so the caller deletes \
                 it first: {hint}"
            );
            assert!(
                !hint.contains("safe to retry as-is"),
                "mark {mark:?}: a blind retry can write a second section: {hint}"
            );
            assert!(
                !hint.contains(&dir.path().to_string_lossy().into_owned()),
                "mark {mark:?}: no absolute path in a hint: {hint}"
            );
            assert_eq!(
                std::fs::read_to_string(&md).unwrap(),
                before,
                "mark {mark:?}: the adopted section stays, and a mark this call wrote is restored"
            );
        }
    }
    // mutation: pass `RetryRule::DeleteFirst` for an adopted call in `allocate_entry_id` → the
    // no-write case says "no file was touched ... safe to retry as-is" and the mark-only case says
    // "No section was written ... safe to retry as-is", red.

    #[test]
    fn a_recorded_section_is_never_adopted() {
        // Params path: params hold F-1.
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        orphan_fixture(dir.path(), &mut cat, None);
        cat.conn
            .execute(
                "UPDATE artifact_augmentation SET params = ?1 WHERE artifact_id = 'art1'",
                [r#"{"failures":[{"id":"F-1"}]}"#],
            )
            .unwrap();
        let out = append_first(&mut cat, &pending("first"));
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));

        // Prose path: the reservation table already covers F-1.
        let dir = tempfile::tempdir().unwrap();
        let mut cat = Catalog::open_in_memory().unwrap();
        orphan_fixture(dir.path(), &mut cat, Some(1));
        cat.conn
            .execute(
                "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
                 VALUES ('art1', 'F', 1, '2026-01-01T00:00:00.000Z')",
                [],
            )
            .unwrap();
        let out = allocate_entry_id(&mut cat, "art1", "F", Some(&pending("first"))).unwrap();
        assert_eq!((out.id.as_str(), out.adopted), ("F-2", false));
    }
    // mutation: `is_recorded = |_| false` → adopts F-1, red.

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
            index_row: None,
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
            index_row: None,
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
            index_row: None,
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
            index_row: None,
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
            index_row: None,
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
            index_row: None,
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
            index_row: None,
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
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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

    /// `LIKE` ignores ASCII case, and an `ESCAPE` clause cures only the wildcard half: a cite of
    /// `notes.md` matched the row of `Notes.md` and, with both present, was reported ambiguous
    /// instead of resolving to the file it names. The suffix comparison is byte-exact.
    /// docs/issues/archive/2026-10-18-the-remaining-like-prefix-sites-are-case-folded-or-unbounded-and-none-is-destructive-today.md
    #[test]
    fn resolve_cite_ref_does_not_fold_case_in_a_rel_path() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("upper")
                .with_abs_path("/x/Notes.md")
                .build(),
        )
        .unwrap();

        let err = resolve_cite_ref(&cat.conn, "notes.md").unwrap_err();
        assert!(
            err.to_string().contains("did not resolve"),
            "a case-only sibling is not the file named, got: {err}"
        );

        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("lower")
                .with_abs_path("/x/notes.md")
                .build(),
        )
        .unwrap();
        assert_eq!(
            resolve_cite_ref(&cat.conn, "notes.md").unwrap(),
            "lower",
            "the exact-case row resolves; the case-only sibling is not a second match"
        );
        // The suffix still anchors at a `/`: `otes.md` is not a path component of either row.
        assert!(resolve_cite_ref(&cat.conn, "otes.md").is_err());
        // And a whole-path cite still resolves by equality.
        assert_eq!(resolve_cite_ref(&cat.conn, "/x/Notes.md").unwrap(), "upper");
    }

    /// `list_stale`'s project filter is a path prefix. `LIKE` made `_`/`%` in the project's path
    /// wildcards (no `ESCAPE` was given) and folded case, so a project at `/x/my_app` also listed
    /// the stale augmentations of `/x/myXapp` and `/x/MY_APP`.
    /// docs/issues/archive/2026-10-18-the-remaining-like-prefix-sites-are-case-folded-or-unbounded-and-none-is-destructive-today.md
    #[test]
    fn list_stale_scopes_to_the_project_path_exactly() {
        let cat = Catalog::open_in_memory().unwrap();
        for (id, path) in [
            ("mine", "/x/my_app/docs/a.md"),
            ("wild", "/x/myXapp/docs/a.md"),
            ("case", "/x/MY_APP/docs/a.md"),
            ("longer", "/x/my_app2/docs/a.md"),
        ] {
            art_upsert(
                &cat,
                &TestArtifactRowBuilder::new(id)
                    .with_abs_path(path)
                    .with_kind("tracker")
                    .build(),
            )
            .unwrap();
            upsert(&cat, &aug(id)).unwrap();
        }

        let listed = list_stale(
            &cat,
            "2999-01-01T00:00:00.000Z",
            50,
            Some(std::path::Path::new("/x/my_app")),
        )
        .unwrap();

        let ids: Vec<&str> = listed.iter().map(|e| e.artifact_id.as_str()).collect();
        assert_eq!(ids, vec!["mine"], "only rows under the project path");
    }
}
