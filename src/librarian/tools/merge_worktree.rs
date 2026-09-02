//! First-class worktree merge: fold every shadow row's DELTA (vs the fork
//! event's base snapshot) onto its main twin, reseat lineage-less rows, close
//! the registration.
//!
//! **F-2 invariant (never violate this):** a shadow row's augmentation params
//! are a base COPY (taken at fork time, see `worktree::resolve_write_target`)
//! plus whatever the worktree appended/edited afterwards. Bare-`graft_rows`-ing
//! a seeded shadow would collide EVERY base entry against main's live array and
//! re-append it as a duplicate. So this module never calls
//! `graft::graft_rows`/`graft::merge_augmentation` on a shadow that has a
//! `worktree_of` lineage edge — it extracts the DELTA against the fork event's
//! recorded `base_params` and folds ONLY that (via `graft::fold_entries`), then
//! re-points history (`graft::repoint_history`) separately. `graft_rows` (the
//! full-history primitive) is only safe for lineage-LESS rows (worktree-born,
//! never base-seeded) in `reseat_one` below.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Map, Value};

use super::worktree::{FORK_EVENT_KIND, LINEAGE_REL};
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::artifact::{self, ArtifactRow};
use crate::librarian::catalog::augmentation;
use crate::librarian::catalog::events;
use crate::librarian::catalog::graft::{self, GraftReport};
use crate::librarian::catalog::worktree as reg;
use crate::librarian::catalog::Catalog;
use crate::librarian::ids;
use crate::util::fs::RepoPath;

#[derive(serde::Deserialize)]
struct Args {
    root: String,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    abandon: bool,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| RecoverableError::new(format!("merge_worktree requires 'root': {e}")))?;
    let root = RepoPath::from(Path::new(&a.root)).into_string();

    let mut cat = ctx.catalog.lock();
    let Some(registration) = reg::get(&cat, &root)? else {
        return Err(RecoverableError::with_hint(
            format!("no worktree registration for `{root}`"),
            "Unregistered legacy rows: use librarian(action=\"doctor\") + fix=\"reseat_worktree\", or doc(action=\"graft\") instead.",
        ));
    };
    if registration.status != "active" {
        return Err(RecoverableError::new(format!(
            "registration for `{root}` is `{}` — nothing to merge",
            registration.status
        )));
    }
    let now = chrono::Utc::now().timestamp_millis();

    // Every catalog row living under `root` — forked shadow twins AND
    // worktree-born artifacts alike. `root` is bound as a LIKE *pattern*, so
    // its own `%`/`_` must be escaped; `descendant_path_like` is the one
    // spelling of that predicate.
    let under_root = crate::librarian::util::descendant_path_like("?1");
    let mut stmt = cat.conn.prepare(&format!(
        "SELECT id FROM artifact WHERE abs_path = ?1 OR abs_path {under_root} \
         ORDER BY abs_path"
    ))?;
    let shadow_ids: Vec<String> = stmt
        .query_map([&root], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    if a.abandon {
        if !a.dry_run {
            cat.conn.execute(
                &format!("DELETE FROM artifact WHERE abs_path = ?1 OR abs_path {under_root}"),
                [&root],
            )?;
            reg::set_status(&cat, &root, "abandoned", now)?;
        }
        return Ok(json!({
            "merged": [], "reseated": [], "conflicts": [], "remap": {}, "suspicious": [],
            "abandoned": shadow_ids.len(),
            "registration": if a.dry_run { "active" } else { "abandoned" },
            "dry_run": a.dry_run,
        }));
    }

    check_rebase_invariant(&registration)?;

    let mut merged = Vec::new();
    let mut reseated = Vec::new();
    let mut conflicts = Vec::new();
    let mut remap = Map::new();
    let mut suspicious = Vec::new();

    for sid in &shadow_ids {
        let lineage: Option<String> = cat
            .conn
            .query_row(
                "SELECT dst_id FROM artifact_link WHERE src_id=?1 AND rel=?2",
                params![sid, LINEAGE_REL],
                |r| r.get(0),
            )
            .optional()?;
        match lineage {
            Some(main_id) => {
                let outcome = merge_one(&mut cat, sid, &main_id, a.dry_run, now)?;
                if outcome.merged {
                    merged.push(json!({"shadow": sid, "into": main_id}));
                }
                conflicts.extend(outcome.conflicts);
                for (k, v) in outcome.remap {
                    remap.insert(k, v);
                }
                suspicious.extend(outcome.suspicious);
            }
            None => {
                if let Some(main_id) = reseat_one(
                    &mut cat,
                    sid,
                    &root,
                    &registration.main_root,
                    a.dry_run,
                    &mut conflicts,
                )? {
                    reseated.push(json!({"from": sid, "to": main_id}));
                }
            }
        }
    }

    if !a.dry_run {
        reg::set_status(&cat, &root, "merged", now)?;
    }

    Ok(json!({
        "merged": merged,
        "reseated": reseated,
        "conflicts": conflicts,
        "remap": remap,
        "suspicious": suspicious,
        "registration": if a.dry_run { "active(dry_run)" } else { "merged" },
        "dry_run": a.dry_run,
        "hint": "Rewrite live-tree citations for any remapped entry ids (see remap).",
    }))
}

/// The worktree branch must be fully rebased onto (or already merged into)
/// main's HEAD before its content is folded in — otherwise the merge would
/// silently paper over an un-rebased divergence. Skips silently (never
/// blocks) whenever the worktree directory is gone, no branch is recorded, or
/// git itself errors: the DB state is self-sufficient once the working
/// directory disappears, and this must never block on git absence.
fn check_rebase_invariant(registration: &reg::RegistrationRow) -> Result<()> {
    let Some(branch) = registration.branch.as_deref() else {
        return Ok(());
    };
    if !Path::new(&registration.worktree_root).exists() {
        return Ok(());
    }
    let is_ancestor = |ancestor: &str, descendant: &str| -> Option<bool> {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&registration.main_root)
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .status()
            .ok()
            .map(|s| s.success())
    };
    match is_ancestor(branch, "HEAD") {
        Some(true) => return Ok(()),
        Some(false) => {}
        None => return Ok(()),
    }
    match is_ancestor("HEAD", branch) {
        Some(true) | None => Ok(()),
        Some(false) => Err(RecoverableError::with_hint(
            format!(
                "worktree branch `{branch}` has diverged from `{}`'s HEAD — neither is an ancestor of the other",
                registration.main_root
            ),
            "Rebase the worktree branch onto main's HEAD (git rebase) before merging, then retry merge_worktree.",
        )),
    }
}

/// Result of folding a single shadow row (one with a `worktree_of` lineage
/// edge) onto its main twin.
struct MergeOneOutcome {
    /// False for a hard skip (missing fork event / missing row) — the shadow
    /// was left untouched and must not be reported as merged.
    merged: bool,
    conflicts: Vec<Value>,
    remap: Map<String, Value>,
    suspicious: Vec<Value>,
}

impl MergeOneOutcome {
    fn skipped(conflict: Value) -> Self {
        Self {
            merged: false,
            conflicts: vec![conflict],
            remap: Map::new(),
            suspicious: Vec::new(),
        }
    }
}

/// Fold `shadow_id`'s delta (vs. the `worktree_fork` event's recorded base
/// snapshot) onto `main_id`. See the module doc comment for the F-2
/// invariant this implements. All reads happen first (steps 1–6, pure
/// computation); writes (steps 7-10) are guarded by `!dry_run` and run inside
/// a single `cat.conn.unchecked_transaction()` — the same pattern
/// `worktree::resolve_write_target` uses, chosen because it borrows the
/// connection immutably (`&self`) so `&Catalog`-taking helpers
/// (`artifact::upsert`, etc.) can still be called while the transaction
/// handle is alive.
fn merge_one(
    cat: &mut Catalog,
    shadow_id: &str,
    main_id: &str,
    dry_run: bool,
    now: i64,
) -> Result<MergeOneOutcome> {
    // Step 1: load the shadow's fork event (base snapshot).
    let fork_payload: Option<String> = cat
        .conn
        .query_row(
            "SELECT payload FROM events WHERE artifact_id=?1 AND kind=?2 ORDER BY created_at DESC LIMIT 1",
            params![shadow_id, FORK_EVENT_KIND],
            |r| r.get(0),
        )
        .optional()?;
    let Some(fork_payload) = fork_payload else {
        // Never guess at a base snapshot — that's the legacy doctor path.
        return Ok(MergeOneOutcome::skipped(json!({
            "kind": "missing_fork_event", "shadow": shadow_id, "artifact": main_id,
        })));
    };
    let fork: Value = serde_json::from_str(&fork_payload).unwrap_or(Value::Null);
    let base_params = fork
        .get("base_params")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let base_frontmatter = fork
        .get("base_frontmatter")
        .cloned()
        .unwrap_or_else(|| json!({}));

    // Step 2: load both rows + augmentation.
    let (Some(shadow_row), Some(main_row)) =
        (artifact::get(cat, shadow_id)?, artifact::get(cat, main_id)?)
    else {
        return Ok(MergeOneOutcome::skipped(json!({
            "kind": "missing_artifact_row", "shadow": shadow_id, "artifact": main_id,
        })));
    };
    let shadow_aug = augmentation::get(cat, shadow_id)?;
    let main_aug = augmentation::get(cat, main_id)?;
    let shadow_params: Value = shadow_aug
        .as_ref()
        .and_then(|a| serde_json::from_str(&a.params).ok())
        .unwrap_or_else(|| json!({}));
    let main_params: Value = main_aug
        .as_ref()
        .and_then(|a| serde_json::from_str(&a.params).ok())
        .unwrap_or_else(|| json!({}));
    let coll = shadow_aug.as_ref().and_then(|a| a.entry_collection.clone());

    let mut conflicts = Vec::new();
    let mut remap = Map::new();
    let mut suspicious = Vec::new();
    let mut new_main_params = main_params.clone();
    let mut fold_report = GraftReport::default();

    // Step 3+4: split by entry_collection, fold appended entries, three-way edited-base entries.
    if let Some(coll_name) = &coll {
        let base_arr = collection_of(&base_params, coll_name);
        let shadow_arr = collection_of(&shadow_params, coll_name);
        let main_arr = collection_of(&main_params, coll_name);
        let base_ids: HashSet<&str> = base_arr.iter().filter_map(entry_id).collect();

        let mut appended = Vec::new();
        let mut edited_base = Vec::new();
        for entry in &shadow_arr {
            let Some(id) = entry_id(entry) else { continue };
            if !base_ids.contains(id) {
                appended.push(entry.clone());
            } else if let Some(base_entry) = base_arr.iter().find(|b| entry_id(b) == Some(id)) {
                if without_id(entry) != without_id(base_entry) {
                    edited_base.push(entry.clone());
                }
            }
        }

        let mut final_arr = if appended.is_empty() {
            main_arr.clone()
        } else {
            graft::fold_entries(&main_arr, &appended, &mut fold_report)
        };
        for (old, new) in &fold_report.remap {
            remap.insert(format!("{shadow_id}:{coll_name}:{old}"), json!(new));
        }
        suspicious.extend(fold_report.suspicious.iter().cloned());

        for shadow_entry in &edited_base {
            let Some(id) = entry_id(shadow_entry) else {
                continue;
            };
            let base_entry = base_arr.iter().find(|b| entry_id(b) == Some(id));
            if let Some(pos) = final_arr.iter().position(|e| entry_id(e) == Some(id)) {
                let main_entry = final_arr[pos].clone();
                let main_unchanged = base_entry.map(without_id) == Some(without_id(&main_entry));
                if main_unchanged {
                    final_arr[pos] = shadow_entry.clone();
                } else {
                    conflicts.push(json!({
                        "artifact": main_id,
                        "key": format!("{coll_name}[{id}]"),
                        "base": base_entry,
                        "main": main_entry,
                        "worktree": shadow_entry,
                    }));
                }
            }
        }

        if let Some(obj) = new_main_params.as_object_mut() {
            obj.insert(coll_name.clone(), Value::Array(final_arr));
        }
    }

    // Step 5: three-way scalar keys (every top-level key except `coll`, union of shadow+base).
    let coll_key = coll.clone().unwrap_or_default();
    let mut scalar_keys: BTreeSet<String> = BTreeSet::new();
    if let Some(obj) = shadow_params.as_object() {
        scalar_keys.extend(obj.keys().filter(|k| **k != coll_key).cloned());
    }
    if let Some(obj) = base_params.as_object() {
        scalar_keys.extend(obj.keys().filter(|k| **k != coll_key).cloned());
    }
    for key in &scalar_keys {
        let shadow_v = shadow_params.get(key).cloned().unwrap_or(Value::Null);
        let base_v = base_params.get(key).cloned().unwrap_or(Value::Null);
        if shadow_v == base_v {
            continue; // worktree never touched this key
        }
        let main_v = main_params.get(key).cloned().unwrap_or(Value::Null);
        if main_v == base_v {
            if let Some(obj) = new_main_params.as_object_mut() {
                obj.insert(key.clone(), shadow_v);
            }
        } else {
            conflicts.push(json!({
                "artifact": main_id, "key": key, "base": base_v, "main": main_v, "worktree": shadow_v,
            }));
        }
    }

    // Step 6: frontmatter three-way (status/title/tags/topic/time_scope/owners).
    let mut amended_main_row = main_row.clone();
    let shadow_fm = frontmatter_json(&shadow_row);
    let main_fm = frontmatter_json(&main_row);
    for field in ["status", "title", "tags", "topic", "time_scope", "owners"] {
        let base_v = base_frontmatter.get(field).cloned().unwrap_or(Value::Null);
        let shadow_v = shadow_fm.get(field).cloned().unwrap_or(Value::Null);
        if shadow_v == base_v {
            continue;
        }
        let main_v = main_fm.get(field).cloned().unwrap_or(Value::Null);
        if main_v == base_v {
            apply_frontmatter_field(&mut amended_main_row, field, &shadow_v);
        } else {
            conflicts.push(json!({
                "artifact": main_id, "key": field, "base": base_v, "main": main_v, "worktree": shadow_v,
            }));
        }
    }

    if dry_run {
        return Ok(MergeOneOutcome {
            merged: true,
            conflicts,
            remap,
            suspicious,
        });
    }

    // Steps 7-10: writes, single atomic transaction.
    let tx = cat.conn.unchecked_transaction()?;

    artifact::upsert(cat, &amended_main_row)?;
    cat.conn.execute(
        "UPDATE artifact_augmentation SET params=?1 WHERE artifact_id=?2",
        params![serde_json::to_string(&new_main_params)?, main_id],
    )?;

    // Delete the shadow's own worktree_of lineage link BEFORE repoint_history
    // runs. repoint_history's `UPDATE OR IGNORE artifact_link SET src_id=main
    // WHERE src_id=shadow` would otherwise turn this
    // (src=shadow, dst=main, rel=worktree_of) row into a self-referential
    // (src=main, dst=main, rel=worktree_of) row: there is no pre-existing
    // (main,main,worktree_of) row for `OR IGNORE` to skip on, so the update
    // succeeds and creates the self-link — and since neither endpoint is
    // `shadow_id` anymore, the shadow's cascade-delete below no longer
    // touches it, leaving main durably (and wrongly) recorded as a
    // worktree-shadow of itself. Deleting it here leaves nothing for
    // `repoint_history` to mis-repoint.
    tx.execute(
        "DELETE FROM artifact_link WHERE src_id=?1 AND dst_id=?2 AND rel=?3",
        params![shadow_id, main_id, LINEAGE_REL],
    )?;

    // Re-point events (fork event included, as audit trail), observations,
    // links, and event_edges from shadow to main. NEVER graft::graft_rows /
    // graft::merge_augmentation here — see module doc comment (F-2).
    let mut history_report = GraftReport::default();
    graft::repoint_history(&tx, shadow_id, main_id, &mut history_report)?;

    let audit_payload = json!({
        "branch": fork.get("branch").cloned().unwrap_or(Value::Null),
        "remap": remap,
        "conflicts": conflicts,
        "entries_merged": fold_report.entries_merged,
        "entries_renumbered": fold_report.entries_renumbered,
    });
    events::insert_with(
        &tx,
        &events::EventRow {
            id: ulid::Ulid::new().to_string(),
            artifact_id: main_id.to_string(),
            kind: "worktree_merge".to_string(),
            payload: audit_payload.to_string(),
            anchor_commit: None,
            head_commit: None,
            author: Some("worktree-overlay".into()),
            created_at: now,
        },
    )?;

    tx.execute("DELETE FROM artifact WHERE id=?1", [shadow_id])?;
    tx.commit()?;

    Ok(MergeOneOutcome {
        merged: true,
        conflicts,
        remap,
        suspicious,
    })
}

/// Reseat a lineage-less row (born in the worktree, never base-seeded) to its
/// main-repo path. Safe to `graft::graft_rows` here — unlike `merge_one`'s
/// shadow rows, this row's entire history is 100% worktree-born, so folding
/// all of it is correct (no base entries to accidentally duplicate).
///
/// Returns `Ok(Some(main_id))` on success (or on a dry-run preview of
/// success), `Ok(None)` on a `reseat_collision` (pushed to `conflicts`, row
/// left untouched) or a vanished-row race.
fn reseat_one(
    cat: &mut Catalog,
    shadow_id: &str,
    root: &str,
    main_root: &str,
    dry_run: bool,
    conflicts: &mut Vec<Value>,
) -> Result<Option<String>> {
    let Some(row_w) = artifact::get(cat, shadow_id)? else {
        return Ok(None); // race: row vanished since the scan; nothing to reseat
    };
    let Ok(rel) = row_w.abs_path.strip_prefix(Path::new(root)) else {
        return Ok(None); // defensive; shouldn't happen given the LIKE scan above
    };
    let main_path: PathBuf = Path::new(main_root).join(rel);
    let id_m = ids::artifact_id_from_abs(&main_path);

    if artifact::get(cat, &id_m)?.is_some() {
        conflicts.push(json!({
            "kind": "reseat_collision",
            "shadow": shadow_id,
            "main_path": RepoPath::from(main_path.as_path()).into_string(),
            "into_id": id_m,
        }));
        return Ok(None);
    }

    if dry_run {
        return Ok(Some(id_m));
    }

    let row_m = ArtifactRow {
        id: id_m.clone(),
        abs_path: main_path,
        ..row_w
    };
    // Two separate transactions (`upsert` autocommits; `graft_rows` runs its
    // own IMMEDIATE tx) — mirrors doctor.rs's `reseat_worktree`. A crash
    // between them is recoverable, not data loss: either an orphan `id_m` row
    // with no history yet, or an un-grafted `shadow_id` that the next run
    // reports as a `reseat_collision` against `id_m`.
    artifact::upsert(cat, &row_m)?;
    graft::graft_rows(cat, shadow_id, &id_m)?;
    Ok(Some(id_m))
}

fn collection_of(params: &Value, coll: &str) -> Vec<Value> {
    params
        .get(coll)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn entry_id(entry: &Value) -> Option<&str> {
    entry.get("id").and_then(Value::as_str)
}

/// Deep-clone of `entry` with the `id` field removed, for content-equality
/// comparison. Local equivalent of `graft::strip_id` (private to that
/// module).
fn without_id(entry: &Value) -> Value {
    let mut e = entry.clone();
    if let Some(obj) = e.as_object_mut() {
        obj.remove("id");
    }
    e
}

/// Same shape `worktree::resolve_write_target` records into a fork event's
/// `base_frontmatter` payload — keep both in sync.
fn frontmatter_json(row: &ArtifactRow) -> Value {
    json!({
        "status": row.status, "title": row.title, "tags": row.tags,
        "topic": row.topic, "time_scope": row.time_scope, "owners": row.owners,
    })
}

fn apply_frontmatter_field(row: &mut ArtifactRow, field: &str, value: &Value) {
    match field {
        "status" => {
            if let Some(s) = value.as_str() {
                row.status = s.to_string();
            }
        }
        "title" => row.title = value.as_str().map(String::from),
        "topic" => row.topic = value.as_str().map(String::from),
        "time_scope" => row.time_scope = value.as_str().map(String::from),
        "tags" => {
            if let Ok(v) = serde_json::from_value::<Vec<String>>(value.clone()) {
                row.tags = v;
            }
        }
        "owners" => {
            if let Ok(v) = serde_json::from_value::<Vec<String>>(value.clone()) {
                row.owners = v;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::call;
    use crate::librarian::catalog::worktree as reg;
    use crate::librarian::catalog::{artifact, augmentation, Catalog};
    use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};

    #[tokio::test]
    async fn merge_folds_delta_without_duplicating_base_entries() {
        // F-2 regression — THE invariant test of this feature.
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        }; // base: items=[F-1]
           // worktree appends F-2 (via the write gate → fork + shadow append)
        let shadow_id = {
            let mut c = ctx.catalog.lock();
            let sid =
                crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id)
                    .unwrap();
            augmentation::append_entry(
                &mut c,
                &sid,
                "items",
                "F",
                serde_json::json!({"t":"wt"}),
                &[],
            )
            .unwrap();
            sid
        };
        // main concurrently appends its own F-2
        {
            let mut c = ctx.catalog.lock();
            augmentation::append_entry(
                &mut c,
                &main_id,
                "items",
                "F",
                serde_json::json!({"t":"main"}),
                &[],
            )
            .unwrap();
        }
        let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"}))
            .await
            .unwrap();
        let c = ctx.catalog.lock();
        let params: serde_json::Value =
            serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params)
                .unwrap();
        let ids: Vec<&str> = params["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        // base F-1 exactly once; main's F-2 kept; worktree's F-2 renumbered to F-3.
        assert_eq!(
            ids,
            vec!["F-1", "F-2", "F-3"],
            "no duplicates, deterministic renumber: {ids:?}"
        );
        assert_eq!(out["remap"][format!("{shadow_id}:items:F-2")], "F-3");
        // shadow row gone, its events re-pointed under main
        assert!(artifact::get(&c, &shadow_id).unwrap().is_none());
        let n: i64 = c
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE artifact_id=?1 AND kind='worktree_fork'",
                [&main_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "fork event preserved as audit trail under main id");
        // registration closed
        assert_eq!(
            reg::get(&c, "/repo/.worktrees/feat")
                .unwrap()
                .unwrap()
                .status,
            "merged"
        );
        // Regression: repoint_history's `UPDATE OR IGNORE artifact_link SET
        // src_id=main WHERE src_id=shadow` turns the shadow's
        // (src=shadow, dst=main, rel=worktree_of) lineage link into a
        // self-referential (src=main, dst=main, rel=worktree_of) row — there is
        // no pre-existing (main,main,worktree_of) row for OR IGNORE to skip on,
        // and the shadow cascade-delete no longer touches it since neither
        // endpoint is shadow_id anymore. Merge must leave NO such self-link.
        let self_link: i64 = c
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 AND rel='worktree_of'",
                [&main_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
                    self_link, 0,
                    "merge must not leave a worktree_of lineage link with main as its own source (self-link)"
                );
        let dst_from_shadow: i64 = c
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM artifact_link WHERE dst_id=?1 AND src_id=?2 AND rel='worktree_of'",
                        [&main_id, &shadow_id],
                        |r| r.get(0),
                    )
                    .unwrap();
        assert_eq!(
            dst_from_shadow, 0,
            "the shadow's own worktree_of link must be gone after merge, not re-pointed"
        );
    }

    #[tokio::test]
    async fn merge_three_ways_scalars_and_reports_conflicts() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        }; // note:"base"
        let _sid = {
            let mut c = ctx.catalog.lock();
            let sid =
                crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id)
                    .unwrap();
            // worktree edits scalar `note`
            augmentation::merge_params(&c, &sid, &serde_json::json!({"note": "wt-edit"})).unwrap();
            sid
        };
        // main ALSO edits `note` → both-changed → conflict, main value survives
        {
            let c = ctx.catalog.lock();
            augmentation::merge_params(&c, &main_id, &serde_json::json!({"note": "main-edit"}))
                .unwrap();
        }
        let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"}))
            .await
            .unwrap();
        let c = ctx.catalog.lock();
        let params: serde_json::Value =
            serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params)
                .unwrap();
        assert_eq!(
            params["note"], "main-edit",
            "conflicted field keeps main value"
        );
        let conflicts = out["conflicts"].as_array().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["key"], "note");
        assert_eq!(conflicts[0]["worktree"], "wt-edit");
    }

    #[tokio::test]
    async fn new_worktree_artifact_reseats_to_main_path() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c);
        }
        // artifact born in the worktree (no lineage edge)
        let wt_born = {
            let c = ctx.catalog.lock();
            let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
                "/repo/.worktrees/feat/docs/new.md",
            ));
            artifact::upsert(
                &c,
                &artifact::TestArtifactRowBuilder::new(&id)
                    .with_abs_path("/repo/.worktrees/feat/docs/new.md")
                    .build(),
            )
            .unwrap();
            crate::librarian::tools::worktree::ensure_registration(
                &c,
                ctx.current_project.as_deref().unwrap(),
            )
            .unwrap();
            id
        };
        let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"}))
            .await
            .unwrap();
        let c = ctx.catalog.lock();
        let main_new =
            crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/repo/docs/new.md"));
        assert!(
            artifact::get(&c, &main_new).unwrap().is_some(),
            "reseated at main path"
        );
        assert!(
            artifact::get(&c, &wt_born).unwrap().is_none(),
            "worktree row gone"
        );
        assert!(out["reseated"].as_array().unwrap().len() == 1);
    }

    #[tokio::test]
    async fn dry_run_writes_nothing_and_abandon_sweeps() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let sid = {
            let mut c = ctx.catalog.lock();
            crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        let _ = call(
            &ctx,
            serde_json::json!({"root": "/repo/.worktrees/feat", "dry_run": true}),
        )
        .await
        .unwrap();
        {
            let c = ctx.catalog.lock();
            assert!(
                artifact::get(&c, &sid).unwrap().is_some(),
                "dry_run left shadow intact"
            );
            assert_eq!(
                reg::get(&c, "/repo/.worktrees/feat")
                    .unwrap()
                    .unwrap()
                    .status,
                "active"
            );
        }
        let _ = call(
            &ctx,
            serde_json::json!({"root": "/repo/.worktrees/feat", "abandon": true}),
        )
        .await
        .unwrap();
        let c = ctx.catalog.lock();
        assert!(
            artifact::get(&c, &sid).unwrap().is_none(),
            "abandon removed shadow"
        );
        assert_eq!(
            reg::get(&c, "/repo/.worktrees/feat")
                .unwrap()
                .unwrap()
                .status,
            "abandoned"
        );
    }

    #[tokio::test]
    async fn merge_skips_shadow_missing_fork_event_and_leaves_main_untouched() {
        // Integrity backstop: a shadow row that sits under the worktree root
        // and carries a `worktree_of` lineage link but has NO `worktree_fork`
        // event (e.g. a corrupted/legacy row) must never be folded — there is
        // no recorded base snapshot to diff against, so merge_one must skip it
        // with a `missing_fork_event` conflict rather than guessing.
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let shadow_id = {
            let c = ctx.catalog.lock();
            let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
                "/repo/.worktrees/feat/docs/trackers/t.md",
            ));
            artifact::upsert(
                &c,
                &artifact::TestArtifactRowBuilder::new(&id)
                    .with_abs_path("/repo/.worktrees/feat/docs/trackers/t.md")
                    .with_kind("tracker")
                    .build(),
            )
            .unwrap();
            // Lineage link present, but deliberately NO worktree_fork event.
            crate::librarian::catalog::links::insert(
                &c,
                &crate::librarian::catalog::links::LinkRow {
                    src_id: id.clone(),
                    dst_id: main_id.clone(),
                    rel: super::LINEAGE_REL.to_string(),
                    created_at: 0,
                },
            )
            .unwrap();
            crate::librarian::tools::worktree::ensure_registration(
                &c,
                ctx.current_project.as_deref().unwrap(),
            )
            .unwrap();
            id
        };
        let before_params: serde_json::Value = {
            let c = ctx.catalog.lock();
            serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params).unwrap()
        };
        let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"}))
            .await
            .unwrap();
        let conflicts = out["conflicts"].as_array().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["kind"], "missing_fork_event");
        assert_eq!(conflicts[0]["shadow"], shadow_id);
        assert!(
            !out["merged"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["shadow"] == shadow_id),
            "a hard-skipped shadow must never be reported as merged"
        );
        let c = ctx.catalog.lock();
        let after_params: serde_json::Value =
            serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params)
                .unwrap();
        assert_eq!(
            before_params, after_params,
            "main params must be untouched when the fork event is missing (no fold, no corruption)"
        );
        assert!(
            artifact::get(&c, &shadow_id).unwrap().is_some(),
            "skipped shadow row must survive the merge, not be silently dropped"
        );
    }

    #[tokio::test]
    async fn worktree_born_row_reseat_collision_reports_conflict_and_leaves_main_untouched() {
        // Integrity backstop: a lineage-less (worktree-born) row must NOT be
        // grafted onto an unrelated main-repo artifact that already occupies
        // its would-be main path. reseat_one must record a `reseat_collision`
        // conflict and leave both rows untouched.
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c);
        }
        // Pre-existing, UNRELATED main-repo artifact at the path the
        // worktree-born row would reseat to.
        let main_collide_id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
            "/repo/docs/existing.md",
        ));
        {
            let c = ctx.catalog.lock();
            artifact::upsert(
                &c,
                &artifact::TestArtifactRowBuilder::new(&main_collide_id)
                    .with_abs_path("/repo/docs/existing.md")
                    .with_title("pre-existing main artifact")
                    .build(),
            )
            .unwrap();
        }
        // worktree-born row (no lineage link) at the same relative path.
        let wt_born = {
            let c = ctx.catalog.lock();
            let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
                "/repo/.worktrees/feat/docs/existing.md",
            ));
            artifact::upsert(
                &c,
                &artifact::TestArtifactRowBuilder::new(&id)
                    .with_abs_path("/repo/.worktrees/feat/docs/existing.md")
                    .build(),
            )
            .unwrap();
            crate::librarian::tools::worktree::ensure_registration(
                &c,
                ctx.current_project.as_deref().unwrap(),
            )
            .unwrap();
            id
        };
        let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"}))
            .await
            .unwrap();
        let conflicts = out["conflicts"].as_array().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0]["kind"], "reseat_collision");
        assert_eq!(conflicts[0]["shadow"], wt_born);
        assert_eq!(conflicts[0]["into_id"], main_collide_id);
        assert!(
            out["reseated"].as_array().unwrap().is_empty(),
            "collision must not be reported as a successful reseat"
        );
        let c = ctx.catalog.lock();
        let row = artifact::get(&c, &main_collide_id).unwrap().unwrap();
        assert_eq!(
            row.title.as_deref(),
            Some("pre-existing main artifact"),
            "pre-existing unrelated main row must be intact, not overwritten by the graft"
        );
        assert!(
            artifact::get(&c, &wt_born).unwrap().is_some(),
            "worktree-born row must survive the collision, not be silently dropped"
        );
    }
}
