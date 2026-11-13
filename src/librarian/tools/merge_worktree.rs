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
use super::{LibrarianRecoverableError, ToolContext};
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
    let a: Args = serde_json::from_value(args).map_err(|e| {
        super::deser_error(
            e,
            "merge_worktree",
            "librarian(action=\"merge_worktree\") requires 'root'",
            "e.g. librarian(action=\"merge_worktree\", root=\"/abs/path/of/the/worktree\", dry_run=true) previews; omit dry_run to merge, or pass abandon=true to drop the shadow rows.",
        )
    })?;
    let root = RepoPath::from(Path::new(&a.root)).into_string();

    let mut cat = ctx.catalog.lock();
    let Some(registration) = reg::get(&cat, &root)? else {
        return Err(LibrarianRecoverableError::with_hint(
            format!("no worktree registration for `{root}`"),
            "Unregistered legacy rows: use librarian(action=\"doctor\") + fix=\"reseat_worktree\", or doc(action=\"graft\") instead.",
        ));
    };
    if registration.status != "active" {
        return Err(LibrarianRecoverableError::new(format!(
            "registration for `{root}` is `{}` — nothing to merge",
            registration.status
        )));
    }
    let now = chrono::Utc::now().timestamp_millis();

    // Every catalog row living under `root` — forked shadow twins AND
    // worktree-born artifacts alike. The comparison is EXACT (`path_at_or_under_sql`), not
    // `LIKE`: `LIKE` is ASCII-case-insensitive, so the registration `.worktrees/feat_1` also
    // selected the rows of `.worktrees/FEAT_1`, and `abandon` deleted them with their cascade
    // (and a merge would have reseated them).
    let (root_exact, root_prefix) = crate::librarian::util::root_and_prefix(&root);
    let under_root = crate::librarian::util::path_at_or_under_sql("abs_path", "?1", "?2");
    let mut stmt = cat.conn.prepare(&format!(
        "SELECT id FROM artifact WHERE {under_root} ORDER BY abs_path"
    ))?;
    let shadow_ids: Vec<String> = stmt
        .query_map([&root_exact, &root_prefix], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    if a.abandon {
        if !a.dry_run {
            cat.conn.execute(
                &format!("DELETE FROM artifact WHERE {under_root}"),
                [&root_exact, &root_prefix],
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
                let outcome = merge_one(&mut cat, sid, &main_id, &registration, a.dry_run, now)?;
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

    // A row skipped for a renumber that would (or may) split params from markdown keeps the
    // registration open: closing it would leave the skipped shadow row with no merge to
    // retry it, and its remedy names the exits.
    let held_open = conflicts
        .iter()
        .any(|c| c.get("kind").and_then(Value::as_str) == Some(RENUMBER_NEEDS_MARKDOWN));

    if !a.dry_run && !held_open {
        reg::set_status(&cat, &root, "merged", now)?;
    }

    let mut out = json!({
        "merged": merged,
        "reseated": reseated,
        "conflicts": conflicts,
        "remap": remap,
        "suspicious": suspicious,
        "registration": if a.dry_run { "active(dry_run)" } else if held_open { "active" } else { "merged" },
        "dry_run": a.dry_run,
        "hint": "Rewrite live-tree citations for any remapped entry ids (see remap).",
    });
    if held_open {
        out["hint"] = json!(if a.dry_run {
            "A row would be skipped because renumbering it would split, or may split, an entry \
             between the params and the markdown, and a merge would leave the registration \
             active. The row's `source` says whether its source was proven, and its remedy names \
             the exits. Nothing was written."
        } else {
            "A row was skipped because renumbering it would split, or may split, an entry \
             between the params and the markdown, so the registration stays active and a second \
             merge skips it again until its remedy is applied. The row's `source` says whether \
             its source was proven, and its remedy names the exits; the rows that merged are \
             done. Rewrite live-tree citations for any remapped entry ids (see remap)."
        });
    }
    Ok(out)
}

/// The conflict kind of a row `merge_one` left unmerged because folding it would renumber an
/// entry that the source's markdown defines by heading.
const RENUMBER_NEEDS_MARKDOWN: &str = "entry_renumber_needs_markdown";

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
        Some(false) => Err(LibrarianRecoverableError::with_hint(
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
    registration: &reg::RegistrationRow,
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
    // A MAIN row the read door cleared (its file resolves outside the repository) holds no file
    // values, and none can be written through the door: its file is outside. So the frontmatter
    // three-way below is skipped for it. Neither the fork's base (recorded before the clearing,
    // in the window) nor the shadow's copy (never cleared when the worktree sits outside the main
    // root, whose reindex alone clears) is printed in the conflicts, written into the
    // `worktree_merge` event, or applied to the main row (review rounds 4 and 5, R4-3, R5-1,
    // R5-7). The fork event itself is immutable and keeps the base (a known limit, see the issue).
    let main_cleared = crate::librarian::read_door::cleared_by_the_read_door(&main_row);
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

        // A renumber changes the id in the params only. When the SOURCE's markdown defines
        // an id the fold would renumber by heading, merging would leave that heading naming
        // an id the row no longer holds, and no tool renumbers one entry in both stores:
        // skip the row, before anything is written (the dry run reports it too).
        if !fold_report.remap.is_empty() {
            let (hits, by) = renumbers_defined_by_heading(
                cat,
                registration,
                &shadow_row,
                &main_row,
                &fold_report.remap,
            );
            if !hits.is_empty() {
                return Ok(MergeOneOutcome::skipped(json!({
                    "kind": RENUMBER_NEEDS_MARKDOWN,
                    "document": crate::librarian::catalog::dual_write::document_label(&main_row.abs_path),
                    "ids": hits,
                    "source": by.as_str(),
                    "remedy": by.remedy(),
                })));
            }
        }

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
    let fields: &[&str] = if main_cleared {
        &[]
    } else {
        &["status", "title", "tags", "topic", "time_scope", "owners"]
    };
    for field in fields {
        let field = *field;
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

/// The ids of `remap` (the shadow's old ids) that the SOURCE document defines by heading.
///
/// The source is the shadow's file while it exists, else that file's blob at the tip of the
/// registration's branch (the worktree directory is normally gone by the time a merge runs).
/// Main's own file cannot stand in for it: once the branch is merged, main's file holds the
/// worktree's heading too, and only the source tells it from main's own entry of that id.
///
/// FAILS CLOSED when the source cannot be read (no branch recorded, branch deleted or
/// rebased, file absent at the tip, or a door refusal of a PRESENT shadow file, which never
/// falls through to the tip): the ids count as defined whenever main's file defines them by
/// heading, and a main file the door refuses counts every id. A main file that is not on disk
/// defines nothing, so a params-only ledger merges as before. The second value says which
/// source decided, so the skip can say whether the split is proven.
fn renumbers_defined_by_heading(
    cat: &Catalog,
    registration: &reg::RegistrationRow,
    shadow_row: &ArtifactRow,
    main_row: &ArtifactRow,
    remap: &std::collections::BTreeMap<String, String>,
) -> (Vec<String>, DecidedBy) {
    let door = crate::librarian::read_door::for_catalog(cat);
    // A path that EXISTS (a file, a link even a dangling or looping one, a path that cannot be
    // inspected) is the source, and a door refusal of it is an unreadable source, never a
    // reason to read the branch tip: the tip can differ from the working file (a heading that is
    // only in the file, uncommitted, would read as absent there). `symlink_metadata` does not
    // follow a link, so a dangling link counts as present, as the door sees it. Only a path that
    // is not there at all reads the tip.
    let present = match std::fs::symlink_metadata(&shadow_row.abs_path) {
        Ok(_) => true,
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    };
    let (source, unreadable_by) = if present {
        (
            door.text(&shadow_row.abs_path)
                .ok()
                .map(|body| (body, DecidedBy::WorktreeFile)),
            DecidedBy::WorktreeFileRefused,
        )
    } else {
        (
            branch_tip_text(registration, &shadow_row.abs_path)
                .map(|body| (body, DecidedBy::BranchTip)),
            DecidedBy::MainFileSourceUnreadable,
        )
    };
    if let Some((body, by)) = source {
        return (graft::renumbered_ids_defined_in(remap, &body), by);
    }
    match door.text(&main_row.abs_path) {
        Ok(body) => (
            graft::renumbered_ids_defined_in(remap, &body),
            unreadable_by,
        ),
        Err(crate::librarian::read_door::ReadError::Io(e))
            if e.kind() == std::io::ErrorKind::NotFound =>
        {
            (Vec::new(), unreadable_by)
        }
        Err(_) => (remap.keys().cloned().collect(), DecidedBy::Unreadable),
    }
}

/// Which source decided a skipped renumber, recorded on the conflict as `source`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DecidedBy {
    /// The shadow's own file, read through the door: proven.
    WorktreeFile,
    /// The shadow's file at the tip of the registration's branch: proven.
    BranchTip,
    /// The worktree's file is not there (and no branch tip supplies it), so main's file stood
    /// in: the split is possible, not proven. Restoring the branch lets the merge read it.
    MainFileSourceUnreadable,
    /// The worktree's file IS there but the read door refuses it, so main's file stood in: the
    /// split is possible, not proven. Restoring the branch does not help; making the file a
    /// readable regular file does.
    WorktreeFileRefused,
    /// Neither the source nor main's file could be read: every renumbered id is held.
    Unreadable,
}

impl DecidedBy {
    fn as_str(self) -> &'static str {
        match self {
            DecidedBy::WorktreeFile => "worktree_file",
            DecidedBy::BranchTip => "branch_tip",
            DecidedBy::MainFileSourceUnreadable => "main_file_source_unreadable",
            DecidedBy::WorktreeFileRefused => "worktree_file_refused",
            DecidedBy::Unreadable => "unreadable",
        }
    }

    fn proven(self) -> bool {
        matches!(self, DecidedBy::WorktreeFile | DecidedBy::BranchTip)
    }

    /// What the skip says and which exits it names, true for the source that decided.
    fn remedy(self) -> String {
        let abandon = "librarian(action=\"merge_worktree\", root=<this worktree's root>, \
                       abandon=true) drops the worktree's catalog rows, so an entry held only \
                       in params is lost for good, while an entry in the markdown remains in \
                       the file that holds it, and a file may then hold two `## F-n` definers \
                       to rename by hand.";
        if self.proven() {
            return format!(
                "The worktree's {} defines these ids by heading, so renumbering would split an \
                 entry between the params and the markdown, and no tool renumbers an entry in \
                 both (`update_entry` refuses `id`; `rekey_prefix` moves a whole prefix). This \
                 row is skipped and the registration stays active. {abandon} It is the only exit.",
                if self == DecidedBy::WorktreeFile {
                    "file"
                } else {
                    "file at its branch tip"
                }
            );
        }
        let (stood_in, exits) = match self {
            DecidedBy::Unreadable => (
                "Neither the worktree's file nor main's file could be read through the read \
                 door, so every renumbered id is held",
                "make both readable regular files inside their repositories (restore the \
                 worktree's branch at its tip if its file is absent) and merge again, so the \
                 merge reads the source and decides",
            ),
            DecidedBy::WorktreeFileRefused => (
                "The read door refuses the worktree's file, so main's file stood in for it and \
                 defines these ids by heading",
                "make the worktree's file a readable regular file inside its repository and \
                 merge again, so the merge reads the source and decides",
            ),
            _ => (
                "The worktree's file could not be read (it is absent, no branch is recorded, \
                 or the branch is gone), so main's file stood in for it and defines these ids \
                 by heading",
                "restore the worktree's branch at its tip (or its directory) and merge again, \
                 so the merge reads the source and decides",
            ),
        };
        format!(
            "{stood_in}: renumbering may split an entry between the params and the markdown. \
             This row is skipped and the registration stays active. Two exits: {exits}; or \
             {abandon}"
        )
    }
}

/// `abs_path`'s text at the tip of `registration.branch` in the repository at
/// `registration.main_root`, through `git2`. `None` when no branch is recorded, the path is not
/// under the worktree root, the branch or the file is not there, or the blob is not text.
fn branch_tip_text(registration: &reg::RegistrationRow, abs_path: &Path) -> Option<String> {
    let branch = registration.branch.as_deref()?;
    let abs = RepoPath::from(abs_path).into_string();
    let rel = abs.strip_prefix(&format!("{}/", registration.worktree_root))?;
    let repo = git2::Repository::open(&registration.main_root).ok()?;
    let commit =
        crate::librarian::derived::scan::open_commit(&repo, &format!("refs/heads/{branch}"))
            .ok()?;
    crate::librarian::derived::scan::blob_text(&repo, &commit, rel)
        .ok()
        .flatten()
}

/// Reseat a lineage-less row (born in the worktree, never base-seeded) to its
/// main-repo path. Safe to `graft::graft_rows` here — unlike `merge_one`'s
/// shadow rows, this row's entire history is 100% worktree-born, so folding
/// all of it is correct (no base entries to accidentally duplicate).
///
/// Returns `Ok(Some(main_id))` on success (or on a dry-run preview of
/// success), `Ok(None)` on a `reseat_collision` or a `reseat_target_missing` (pushed
/// to `conflicts`, row left untouched) or a vanished-row race.
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

    // A row reseated onto a path with no file there is a `missing_file` defect: the file lives
    // only in the removed worktree, or on a branch that has not reached the main checkout. Leave
    // the row where it is and say so, in the dry run too, so the cost is visible before `merge`
    // is chosen over `abandon`. Bug 93a467ab0eb9bbf4: 78 rows were reseated this way.
    if !main_path.exists() {
        conflicts.push(json!({
            "kind": "reseat_target_missing",
            "shadow": shadow_id,
            "main_path": RepoPath::from(main_path.as_path()).into_string(),
            "hint": "no file exists at the main-checkout path; merge the branch that holds it, or abandon this worktree to drop the row",
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
                None,
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
                None,
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

    /// A worktree session on REAL directories: `<tmp>/repo` is the main checkout and
    /// `<tmp>/repo/.worktrees/feat` the worktree. The fictional `/repo` that `wt_ctx` uses
    /// cannot say whether a reseat target exists on disk, which these tests are about.
    fn real_wt_ctx() -> (
        tempfile::TempDir,
        crate::librarian::tools::ToolContext,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("repo");
        let wt = main.join(".worktrees").join("feat");
        std::fs::create_dir_all(wt.join("docs")).unwrap();
        std::fs::create_dir_all(main.join("docs")).unwrap();
        let ctx = crate::librarian::tools::TestToolContextBuilder::new(
            Catalog::open_in_memory().unwrap(),
        )
        .with_current_project(std::sync::Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: wt.clone(),
                git_root: wt.clone(),
                main_root: Some(main.clone()),
                umbrella: None,
            },
        ))
        .build();
        (tmp, ctx, main, wt)
    }

    /// Registers the worktree and seeds a lineage-less row at `<wt>/docs/<name>`; returns its id.
    fn seed_worktree_born(
        ctx: &crate::librarian::tools::ToolContext,
        wt: &std::path::Path,
        name: &str,
    ) -> String {
        let path = wt.join("docs").join(name);
        let id = crate::librarian::ids::artifact_id_from_abs(&path);
        let c = ctx.catalog.lock();
        artifact::upsert(
            &c,
            &artifact::TestArtifactRowBuilder::new(&id)
                .with_abs_path(path)
                .build(),
        )
        .unwrap();
        crate::librarian::tools::worktree::ensure_registration(
            &c,
            ctx.current_project.as_deref().unwrap(),
        )
        .unwrap();
        id
    }

    #[tokio::test]
    async fn new_worktree_artifact_reseats_to_main_path() {
        let (_tmp, ctx, main, wt) = real_wt_ctx();
        // The file reached the main checkout (the worktree branch was merged).
        std::fs::write(main.join("docs/new.md"), "# new\n").unwrap();
        let wt_born = seed_worktree_born(&ctx, &wt, "new.md");
        let out = call(&ctx, serde_json::json!({"root": wt.to_str().unwrap()}))
            .await
            .unwrap();
        let c = ctx.catalog.lock();
        let main_new = crate::librarian::ids::artifact_id_from_abs(&main.join("docs/new.md"));
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

    /// Bug 93a467ab0eb9bbf4: three merges reseated 78 rows onto main-checkout paths where no file
    /// exists, and `doctor` reported 78 new `missing_file` defects. A row whose main path has no file
    /// must stay where it is, reported as a conflict, in the dry run and in the real run alike.
    #[tokio::test]
    async fn a_worktree_born_row_whose_main_path_has_no_file_is_left_untouched() {
        let (_tmp, ctx, main, wt) = real_wt_ctx();
        // The worktree has the file; the main checkout does not (it lives on an unmerged branch).
        std::fs::write(wt.join("docs/side.md"), "# side\n").unwrap();
        let wt_born = seed_worktree_born(&ctx, &wt, "side.md");
        let main_side = crate::librarian::ids::artifact_id_from_abs(&main.join("docs/side.md"));

        for dry_run in [true, false] {
            let out = call(
                &ctx,
                serde_json::json!({"root": wt.to_str().unwrap(), "dry_run": dry_run}),
            )
            .await
            .unwrap();
            let conflicts = out["conflicts"].as_array().unwrap();
            assert_eq!(conflicts.len(), 1, "dry_run={dry_run}: {out}");
            assert_eq!(conflicts[0]["kind"], "reseat_target_missing");
            assert_eq!(conflicts[0]["shadow"], wt_born);
            assert!(
                out["reseated"].as_array().unwrap().is_empty(),
                "dry_run={dry_run}: a missing target is not a successful reseat: {out}"
            );
            let c = ctx.catalog.lock();
            assert!(
                artifact::get(&c, &wt_born).unwrap().is_some(),
                "dry_run={dry_run}: the worktree row stays"
            );
            assert!(
                artifact::get(&c, &main_side).unwrap().is_none(),
                "dry_run={dry_run}: no row is written at a path with no file"
            );
        }
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

    /// A fold that renumbers `F-2` (main and the worktree each appended one), over REAL
    /// directories and a real git repository at `<tmp>/repo`:
    ///
    /// - `main_text`: main's `docs/trackers/t.md`, as the working tree holds it after the merge;
    /// - `tip`: the file at the tip of branch `feat` (committed under `refs/heads/feat`), or no
    ///   such branch when `None`;
    /// - `recorded_branch`: the branch the registration names, or `None`;
    /// - `shadow_text`: the shadow's file in the worktree directory, which is absent when `None`
    ///   (the normal case: the merge precondition is that the branch is merged, and the worktree
    ///   is removed).
    ///
    /// Main's own `F-2` is a params-only row; the worktree's `F-2` is the shadow's delta.
    struct Renumber {
        _tmp: tempfile::TempDir,
        ctx: crate::librarian::tools::ToolContext,
        wt_root: String,
        main_id: String,
        shadow_id: String,
    }

    fn renumber_scenario(
        main_text: &str,
        tip: Option<&str>,
        recorded_branch: Option<&str>,
        shadow_text: Option<&str>,
    ) -> Renumber {
        use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
        let tmp = tempfile::tempdir().unwrap();
        let repo_dir = tmp.path().join("repo");
        let wt = repo_dir.join(".worktrees/feat");
        let rel = "docs/trackers/t.md";
        std::fs::create_dir_all(repo_dir.join("docs/trackers")).unwrap();
        let repo = git2::Repository::init(&repo_dir).unwrap();
        if let Some(text) = tip {
            std::fs::write(repo_dir.join(rel), text).unwrap();
            let mut idx = repo.index().unwrap();
            idx.add_path(std::path::Path::new(rel)).unwrap();
            idx.write().unwrap();
            let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
            let sig = git2::Signature::now("t", "t@e").unwrap();
            repo.commit(Some("refs/heads/feat"), &sig, &sig, "tip", &tree, &[])
                .unwrap();
            // `feat` is merged into main's HEAD, so a worktree directory that exists passes
            // the rebase precondition.
            repo.set_head("refs/heads/feat").unwrap();
        }
        std::fs::write(repo_dir.join(rel), main_text).unwrap();
        if let Some(text) = shadow_text {
            std::fs::create_dir_all(wt.join("docs/trackers")).unwrap();
            std::fs::write(wt.join(rel), text).unwrap();
        }

        let main_path = repo_dir.join(rel);
        let main_id = crate::librarian::ids::artifact_id_from_abs(&main_path);
        // The fork keys on the main row's path, not on any file under the worktree.
        let ctx = crate::librarian::tools::TestToolContextBuilder::new(
            Catalog::open_in_memory().unwrap(),
        )
        .with_current_project(std::sync::Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: wt.clone(),
                git_root: wt.clone(),
                main_root: Some(repo_dir.clone()),
                umbrella: None,
            },
        ))
        .build();
        let wt_root = crate::util::fs::RepoPath::from(wt.as_path()).into_string();
        let main_root = crate::util::fs::RepoPath::from(repo_dir.as_path()).into_string();
        let shadow_id = {
            let mut c = ctx.catalog.lock();
            artifact::upsert(
                &c,
                &TestArtifactRowBuilder::new(&main_id)
                    .with_abs_path(&main_path)
                    .with_kind("tracker")
                    .build(),
            )
            .unwrap();
            augmentation::upsert(
                &c,
                &augmentation::AugmentationRow {
                    artifact_id: main_id.clone(),
                    prompt: "p".into(),
                    params: r#"{"items":[{"id":"F-1","t":"a"}]}"#.into(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: "2026-01-01T00:00:00.000Z".into(),
                    updated_at: "2026-01-01T00:00:00.000Z".into(),
                    render_template: None,
                    params_schema: None,
                    append_mode: false,
                    history_cap: None,
                    entry_collection: Some("items".into()),
                    refreshed_at_commit: None,
                },
            )
            .unwrap();
            let sid =
                crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id)
                    .unwrap();
            // `resolve_write_target` registers the worktree without a branch (the fixture has
            // no `.git` pointer to read one from); record the scenario's.
            reg::upsert_active(&c, &wt_root, &main_root, recorded_branch, 1).unwrap();
            // Each side holds its own `F-2` in params only: set directly, because the
            // allocator counts main's file too, whose headings are the scenario's variable.
            for (id, t) in [(&sid, "wt"), (&main_id, "main")] {
                let mut aug = augmentation::get(&c, id).unwrap().unwrap();
                aug.params = serde_json::json!({
                    "items": [{"id": "F-1", "t": "a"}, {"id": "F-2", "t": t}]
                })
                .to_string();
                augmentation::upsert(&c, &aug).unwrap();
            }
            sid
        };
        Renumber {
            _tmp: tmp,
            ctx,
            wt_root,
            main_id,
            shadow_id,
        }
    }

    /// `source` is the conflict's record of which source decided; a PROVEN source (the
    /// worktree file or the branch tip) says the split would happen, an unreadable one says it
    /// may and names restoring the branch as an exit. Both name `abandon` and what it drops.
    fn assert_skipped_for_markdown(s: &Renumber, out: &serde_json::Value, source: &str) {
        assert_eq!(
            out["conflicts"][0]["kind"],
            serde_json::json!("entry_renumber_needs_markdown"),
            "{out}"
        );
        assert_eq!(
            out["conflicts"][0]["ids"],
            serde_json::json!(["F-2"]),
            "{out}"
        );
        assert_eq!(
            out["conflicts"][0]["source"],
            serde_json::json!(source),
            "{out}"
        );
        let remedy = out["conflicts"][0]["remedy"].as_str().unwrap();
        assert!(remedy.contains("abandon"), "{remedy}");
        assert!(
            remedy.contains("held only in params"),
            "abandon's cost is named: {remedy}"
        );
        assert!(
            remedy.contains("remains in the file that holds it") && !remedy.contains("merged file"),
            "abandon does not promise the entry is in a merged file: {remedy}"
        );
        if source == "worktree_file" || source == "branch_tip" {
            assert!(
                remedy.contains("would split") && !remedy.contains("may split"),
                "a proven source states the split as fact: {remedy}"
            );
        } else if source == "worktree_file_refused" {
            assert!(
                remedy.contains("may split") && !remedy.contains("would split"),
                "a refused file is an unproven source: {remedy}"
            );
            assert!(
                remedy.contains("readable regular file inside its repository"),
                "the exit for a refused file is to make it readable: {remedy}"
            );
            assert!(
                !remedy.contains("restore the worktree's branch") && !remedy.contains("absent"),
                "restoring the branch does not help a file that is present: {remedy}"
            );
        } else {
            assert!(
                remedy.contains("may split") && !remedy.contains("would split"),
                "an unproven source says `may`: {remedy}"
            );
            assert!(
                remedy.contains("restore the worktree's branch"),
                "an unproven skip names the exit that lets the merge read the source: {remedy}"
            );
        }
        assert_eq!(out["merged"], serde_json::json!([]), "{out}");
        let c = s.ctx.catalog.lock();
        assert_eq!(
            reg::get(&c, &s.wt_root).unwrap().unwrap().status,
            "active",
            "a skipped row keeps the registration active"
        );
        assert_eq!(out["registration"], serde_json::json!("active"), "{out}");
        let hint = out["hint"].as_str().unwrap();
        assert!(
            hint.contains("stays active") && hint.contains("skips it again"),
            "{hint}"
        );
        assert!(
            artifact::get(&c, &s.shadow_id).unwrap().is_some(),
            "the skipped shadow row stays for `abandon`"
        );
        let params = augmentation::get(&c, &s.main_id).unwrap().unwrap().params;
        assert!(
            !params.contains("\"F-3\""),
            "nothing is renumbered in main's params: {params}"
        );
        let text = out.to_string();
        assert!(
            !text.contains(&s.main_id) && !text.contains(&s.shadow_id),
            "a conflict names a document by label, never by id: {text}"
        );
    }

    /// The worktree directory is gone, so the source is the shadow's file at the tip of its
    /// branch. Main's file (post-merge) holds the worktree's one `## F-2` heading, and main's
    /// own `F-2` is params-only. Mutation: ignore both the branch tip (`branch_tip_text`
    /// returns `None`) and main's file (the fallback returns no ids) and the merge goes
    /// through; `the_branch_tip_alone_can_block_the_merge` isolates the tip.
    #[tokio::test]
    async fn a_gone_worktree_is_read_at_its_branch_tip() {
        let s = renumber_scenario(
            "# T\n\n## F-1 — a\n\n## F-2 — wt\n",
            Some("# T\n\n## F-1 — a\n\n## F-2 — wt\n"),
            Some("feat"),
            None,
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "branch_tip");
    }

    /// Discriminates the branch tip from main's file: only the tip defines `F-2`. Mutation:
    /// ignore the branch tip (`branch_tip_text` returns `None`) and main's file, which has
    /// no `F-2` heading, lets the merge through.
    #[tokio::test]
    async fn the_branch_tip_alone_can_block_the_merge() {
        let s = renumber_scenario(
            "# T\n\n## F-1 — a\n",
            Some("# T\n\n## F-1 — a\n\n## F-2 — wt\n"),
            Some("feat"),
            None,
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "branch_tip");
    }

    /// No branch recorded and no worktree directory: the source cannot be read, so the row
    /// is skipped when main's file defines the id by heading. Mutation: treat an unreadable
    /// source as "no heading".
    #[tokio::test]
    async fn an_unreadable_source_fails_closed_when_main_defines_the_id() {
        let s = renumber_scenario("# T\n\n## F-2 — wt\n", None, None, None);
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "main_file_source_unreadable");
    }

    /// A shadow file that is PRESENT but that the read door refuses (a symlink to a file
    /// outside the repository) is an unreadable source: it fails closed on main's file, and
    /// never falls through to the branch tip, which can differ from the working file. Here
    /// the tip says "no heading" while main's file defines `F-2`. The conflict's `source` is
    /// `worktree_file_refused`, and its remedy asks for a readable file, not a restored
    /// branch. Mutation: restore the `.ok()` that sends a door refusal to `branch_tip_text`;
    /// the merge then goes through on the stale tip and renumbers an entry its file still
    /// names. Mutation: report the case as `MainFileSourceUnreadable`, and the remedy offers
    /// the branch restore that cannot help.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_refused_shadow_file_never_falls_through_to_the_branch_tip() {
        let s = renumber_scenario(
            "# T\n\n## F-2 — wt\n",
            Some("# T\n\n## F-1 — a\n"),
            Some("feat"),
            None,
        );
        let outside = s._tmp.path().join("outside.md");
        std::fs::write(&outside, "# T\n\n## F-2 — wt\n").unwrap();
        let shadow_file = {
            let c = s.ctx.catalog.lock();
            artifact::get(&c, &s.shadow_id).unwrap().unwrap().abs_path
        };
        std::fs::create_dir_all(shadow_file.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &shadow_file).unwrap();
        assert!(
            std::fs::metadata(&shadow_file).unwrap().is_file(),
            "precondition: the shadow file is present"
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "worktree_file_refused");
    }

    /// A shadow path that is a DANGLING symlink exists as a path the door refuses, though
    /// `metadata` (which follows links) calls it absent: it is a refused worktree file, not
    /// a missing one, so the branch tip (which says "no heading" here) is not read. Mutation:
    /// test presence with `std::fs::metadata` instead of `symlink_metadata`.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_dangling_shadow_link_is_a_refused_file_not_an_absent_one() {
        let s = renumber_scenario(
            "# T\n\n## F-2 — wt\n",
            Some("# T\n\n## F-1 — a\n"),
            Some("feat"),
            None,
        );
        let shadow_file = {
            let c = s.ctx.catalog.lock();
            artifact::get(&c, &s.shadow_id).unwrap().unwrap().abs_path
        };
        std::fs::create_dir_all(shadow_file.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(s._tmp.path().join("nowhere.md"), &shadow_file).unwrap();
        assert!(
            std::fs::metadata(&shadow_file).is_err() && shadow_file.symlink_metadata().is_ok(),
            "precondition: the link dangles"
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "worktree_file_refused");
    }

    /// A present shadow file whose heading is only in the working file (the branch tip and main
    /// both say "no heading": the uncommitted-heading case) blocks the merge, decided by the
    /// worktree file. Mutation: ignore the shadow body in the `WorktreeFile` arm.
    #[tokio::test]
    async fn a_present_shadow_heading_blocks_the_merge() {
        let s = renumber_scenario(
            "# T\n\n## F-1 — a\n",
            Some("# T\n\n## F-1 — a\n"),
            Some("feat"),
            Some("# T\n\n## F-1 — a\n\n## F-2 — wt\n"),
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "worktree_file");
    }

    /// Neither the shadow's file nor main's file can be read through the door: every
    /// renumbered id is held, and the conflict says no source could be read. Mutation: let an
    /// unreadable main file count no ids.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_refused_main_file_holds_every_renumbered_id() {
        let s = renumber_scenario("# T\n", None, None, None);
        let main_file = {
            let c = s.ctx.catalog.lock();
            artifact::get(&c, &s.main_id).unwrap().unwrap().abs_path
        };
        let outside = s._tmp.path().join("outside.md");
        std::fs::write(&outside, "# T\n").unwrap();
        std::fs::remove_file(&main_file).unwrap();
        std::os::unix::fs::symlink(&outside, &main_file).unwrap();
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_skipped_for_markdown(&s, &out, "unreadable");
    }

    /// The same unreadable source with no heading in main's file for the id: nothing says the
    /// entry is defined in markdown, so the params-only renumber goes through as before.
    /// Mutation: skip every row whose source is unreadable.
    #[tokio::test]
    async fn an_unreadable_source_with_no_heading_in_main_still_renumbers() {
        let s = renumber_scenario("# T\n\n## F-1 — a\n", None, None, None);
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_eq!(out["conflicts"], serde_json::json!([]), "{out}");
        assert_eq!(out["registration"], serde_json::json!("merged"), "{out}");
    }

    /// The source is readable and defines no `F-2`; main's file does (its own entry). The
    /// renumber is a params-only one: merged, `F-2` becomes `F-3`, the registration closes.
    /// Mutation: check main's body instead of the source's.
    #[tokio::test]
    async fn a_source_with_no_heading_still_renumbers() {
        let s = renumber_scenario(
            "# T\n\n## F-1 — a\n\n## F-2 — main\n",
            None,
            None,
            Some("# T\n\n## F-1 — a\n"),
        );
        let out = call(&s.ctx, serde_json::json!({"root": s.wt_root}))
            .await
            .unwrap();
        assert_eq!(out["conflicts"], serde_json::json!([]), "{out}");
        assert_eq!(
            out["remap"][format!("{}:items:F-2", s.shadow_id)],
            serde_json::json!("F-3"),
            "{out}"
        );
        assert_eq!(out["registration"], serde_json::json!("merged"), "{out}");
        let c = s.ctx.catalog.lock();
        assert_eq!(reg::get(&c, &s.wt_root).unwrap().unwrap().status, "merged");
    }

    /// A dry run reports the skip too and changes nothing.
    #[tokio::test]
    async fn a_dry_run_reports_the_skip_and_writes_nothing() {
        let s = renumber_scenario(
            "# T\n\n## F-2 — wt\n",
            Some("# T\n\n## F-2 — wt\n"),
            Some("feat"),
            None,
        );
        let out = call(
            &s.ctx,
            serde_json::json!({"root": s.wt_root, "dry_run": true}),
        )
        .await
        .unwrap();
        assert_eq!(
            out["conflicts"][0]["kind"],
            serde_json::json!("entry_renumber_needs_markdown"),
            "{out}"
        );
        assert_eq!(out["merged"], serde_json::json!([]), "{out}");
        let hint = out["hint"].as_str().unwrap();
        assert!(
            hint.contains("would be skipped")
                && !hint.contains("second merge")
                && !hint.contains("are done"),
            "a dry run says what it knows, not what a merge did: {hint}"
        );
        let c = s.ctx.catalog.lock();
        assert_eq!(reg::get(&c, &s.wt_root).unwrap().unwrap().status, "active");
        assert!(artifact::get(&c, &s.shadow_id).unwrap().is_some());
    }
}
