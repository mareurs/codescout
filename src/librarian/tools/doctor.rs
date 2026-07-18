//! Doctor — catalog drift scanner.
//!
//! Read-only invariant checks against the librarian catalog. Surfaces drift
//! the moment it lands instead of when it cascades into downstream query
//! failures (e.g. rounds 5–9 of the forward-slash saga, where the symptom
//! was `LIKE` patterns returning empty sets rather than the catalog
//! flagging its own corruption).
//!
//! Checks (MVP):
//!
//! 1. `abs_path_must_be_absolute` — the schema declares
//!    `abs_path TEXT NOT NULL UNIQUE` but does not enforce absoluteness.
//!    Pre-#66 code paths stored relative strings in some rows; the doctor
//!    surfaces them so they can be migrated (or evicted via `reindex`)
//!    rather than masquerading as `missing_file` false positives.
//!    Discovered in the live-catalog smoke test after the #69 commit.
//! 2. `backslash_in_abs_path` — `artifact.abs_path` must contain only `/`
//!    separators. After the [`crate::util::fs::RepoPath`] newtype migration,
//!    every write goes through `to_forward_slash` — any backslash row is
//!    pre-migration drift.
//! 3. `ads_colon_in_abs_path` — no colon outside the optional Windows
//!    drive-letter prefix (`[a-zA-Z]:/`). Defends against the NTFS alternate
//!    data stream `foo.txt:hidden` shape (Ibex S-2 in rounds 3–8 review).
//! 4. `dotdot_segment_in_abs_path` — no segment is exactly `..`. Catches
//!    path-escape strings even though the gather tool's
//!    [`guard_relative_path`] already rejects them on input.
//! 5. `missing_file` — every `artifact.abs_path` must exist on disk
//!    (`Path::exists()`). Catches rows orphaned by `git rm` /
//!    out-of-band file moves that bypassed `reindex`.
//! 6. `backslash_in_git_root` — `commits.git_root` carries paths too;
//!    the same forward-slash invariant applies (commits.rs writes via
//!    `RepoPath::from_path(...).into_string()` post-#66).
//! 7. `worktree_scoped_row` — flags catalog rows whose abs_path is under a
//!    linked git worktree; classifies no_collision vs collision, and flags
//!    `registered` (an ACTIVE `worktree_registration` covers the row's
//!    worktree root — pending `librarian(action="merge_worktree")`, not a
//!    reseat). Unregistered rows still feed `fix=reseat_worktree`, which is
//!    now the LEGACY fallback for catalog drift the overlay never saw.
//!
//! Deferred to a follow-up: NFC unicode normalization, orphan
//! `artifact_augmentation` rows (the FK already cascades on artifact
//! deletion, but a defensive check would catch FK-constraint disabled
//! corruption).
//!
//! The default scan is read-only. An opt-in `fix=prune_missing` mode (with a
//! required `root=` argument) prunes every `artifact` + `commits` row anchored
//! under a dead/renamed repo root — cascade-safe through codescout's own
//! (vec0-linked) connection, which a bare `sqlite3` CLI cannot do (7ca71bf7).
//! Output is a JSON report with `violations` + `summary` (per-check counts); a
//! fix run returns `pruned` counts instead.
//!
//! A second opt-in fix, `fix=reseat_worktree`, consumes `scan_worktree_scoped`
//! violations: `no_collision` rows are durably re-seeded at the main-repo
//! path — a fresh row is written at `id_m = artifact_id_from_abs(main_path)`
//! and [`crate::librarian::catalog::graft::graft_rows`] folds the worktree
//! row's entire history (events, links, event_edges, and the git-invisible
//! `append_entry` augmentation) onto it before deleting the worktree row. The
//! id CHANGES (`id_w` -> `id_m`): catalog identity is
//! `id == artifact_id_from_abs(abs_path)`, so a bare `abs_path` UPDATE that
//! kept `id_w` would leave the row mismatched, and the next MAIN-repo
//! reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact WHERE
//! abs_path=? AND id != ?`) would delete it — cascading away exactly the
//! history this exists to preserve. `collision` rows are left untouched and
//! reported for a manual `graft`. `registered` rows (an ACTIVE
//! `worktree_registration` covers them) are SKIPPED entirely and reported
//! under `skipped` — they belong to `librarian(action="merge_worktree")`.
//!
//! `fix=prune_missing` carries the same registration guard in the other
//! direction: it refuses to prune a dead root an ACTIVE registration still
//! covers, so a `git worktree remove` before merge can't silently delete the
//! catalog's only remaining record of that worktree's unmerged history.

use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::librarian::catalog::artifact::{self, ArtifactRow};
use crate::librarian::catalog::graft;
use crate::librarian::catalog::worktree;
use crate::librarian::{current_project, ids};

use super::{RecoverableError, ToolContext};

/// One violation of a doctor invariant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    /// Which check fired. One of: `abs_path_must_be_absolute`,
    /// `backslash_in_abs_path`, `ads_colon_in_abs_path`,
    /// `dotdot_segment_in_abs_path`, `missing_file`,
    /// `backslash_in_git_root`, `worktree_scoped_row`.
    pub check: String,
    /// The artifact id that owns the violating row, when applicable.
    /// `None` for table-wide checks (e.g. `commits.git_root` has no
    /// artifact_id).
    pub artifact_id: Option<String>,
    /// The path string that triggered the violation.
    pub path: String,
    /// Human-readable detail (position of the offending byte, segment,
    /// etc.). Empty string when the check name alone is sufficient.
    pub detail: String,
}

impl Violation {
    fn new(
        check: &str,
        artifact_id: Option<String>,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            check: check.into(),
            artifact_id,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

/// MCP entry point. Runs every invariant check and returns a structured
/// report. Reads-only; safe to invoke against a live catalog.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Opt-in mutation: prune catalog rows under a dead/renamed repo root.
    // Default (no `fix`) stays read-only.
    if let Some(fix) = args.get("fix").and_then(Value::as_str) {
        let confirm = args
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return run_fix(ctx, fix, args.get("root").and_then(Value::as_str), confirm).await;
    }

    let cat = ctx.catalog.lock();
    let mut all_violations: Vec<Violation> = Vec::new();

    all_violations.extend(scan_artifact_paths(&cat.conn)?);
    all_violations.extend(scan_commits_git_root(&cat.conn)?);
    all_violations.extend(scan_worktree_scoped(&cat.conn)?);

    // Drop the lock before computing the summary — keeps lock scope minimal.
    drop(cat);

    let mut by_check: std::collections::BTreeMap<String, usize> = Default::default();
    for v in &all_violations {
        *by_check.entry(v.check.clone()).or_insert(0) += 1;
    }

    Ok(json!({
        "violations": all_violations,
        "summary": {
            "total": all_violations.len(),
            "by_check": by_check,
        },
    }))
}

/// Validate a `prune_missing` request without touching the catalog beyond a
/// read-only registration check. Returns the validated dead-root path, or a
/// `RecoverableError` for an unsupported fix, a missing/relative root, a root
/// that still exists (a live root's rows are not orphans — per-file
/// deletions belong to reindex's walk, not a bulk prune), or a root an
/// ACTIVE `worktree_registration` still covers (pruning it would delete the
/// catalog's only remaining record of an unmerged worktree's history — that
/// belongs to `librarian(action="merge_worktree")`, not a bulk prune).
fn validate_prune_request<'a>(
    fix: &str,
    root: Option<&'a str>,
    conn: &rusqlite::Connection,
) -> Result<&'a std::path::Path> {
    if fix != "prune_missing" {
        return Err(RecoverableError::new(format!(
            "unknown fix '{fix}' — supported: prune_missing (requires root=<absolute path of the dead/renamed repo root>)"
        )));
    }
    let root = root.ok_or_else(|| {
        RecoverableError::new(
            "fix=prune_missing requires root=<absolute path of the dead/renamed repo root to prune>",
        )
    })?;
    let root_path = std::path::Path::new(root);
    if !root_path.is_absolute() {
        return Err(RecoverableError::new(format!(
            "root must be an absolute path, got '{root}'"
        )));
    }
    if root_path.exists() {
        return Err(RecoverableError::new(format!(
            "root '{root}' still exists on disk — prune_missing only removes rows under a dead/renamed root; nothing pruned"
        )));
    }
    let root_str = crate::util::fs::RepoPath::from_path(root_path).to_string();
    if worktree::covering_conn(conn, &root_str)?.is_some() {
        return Err(RecoverableError::with_hint(
            format!(
                "root '{root}' is covered by an ACTIVE worktree registration — pruning would delete the catalog's only record of an unmerged worktree's history"
            ),
            format!(
                "merge it first via librarian(action=\"merge_worktree\", root=\"{root}\"), or if the worktree is being discarded, librarian(action=\"merge_worktree\", root=\"{root}\", abandon=true) — then retry prune_missing"
            ),
        ));
    }
    Ok(root_path)
}

/// Opt-in catalog repair. Two fixes: `prune_missing` — remove every row
/// anchored under a dead/renamed repo `root`; `reseat_worktree` — re-point
/// `no_collision` worktree-scoped rows (from `scan_worktree_scoped`) onto
/// their main-repo path, leaving `collision` rows untouched for a manual
/// `graft`.
async fn run_fix(ctx: &ToolContext, fix: &str, root: Option<&str>, confirm: bool) -> Result<Value> {
    match fix {
        "prune_missing" => {
            let cat = ctx.catalog.lock();
            match root {
                Some(_) => {
                    // Single-root path (unchanged behaviour).
                    let root_path = validate_prune_request(fix, root, &cat.conn)?;
                    let (artifact_rows, commit_rows) = prune_dead_root(&cat.conn, root_path)?;
                    let out = json!({
                        "fix": "prune_missing",
                        "root": root_path.to_string_lossy(),
                        "pruned": { "artifact_rows": artifact_rows, "commit_rows": commit_rows },
                    });
                    drop(cat);
                    Ok(out)
                }
                None => {
                    // Batch mode over all doctor-identified dead roots.
                    let dead_roots = derive_dead_roots(&cat.conn)?;
                    if !confirm {
                        let mut rows = Vec::new();
                        let (mut ta, mut tc) = (0usize, 0usize);
                        for r in &dead_roots {
                            let (a, c) = count_dead_root(&cat.conn, r)?;
                            ta += a;
                            tc += c;
                            rows.push(json!({
                                "root": r.to_string_lossy(),
                                "artifact_rows": a, "commit_rows": c,
                            }));
                        }
                        return Ok(json!({
                            "fix": "prune_missing", "mode": "dry_run",
                            "dead_roots": rows,
                            "totals": { "roots": dead_roots.len(), "artifact_rows": ta, "commit_rows": tc },
                            "hint": "re-run with confirm=true to prune these rows",
                        }));
                    }
                    let mut results = Vec::new();
                    let (mut ta, mut tc) = (0usize, 0usize);
                    for r in &dead_roots {
                        let root_str = crate::util::fs::RepoPath::from_path(r).to_string();
                        if worktree::covering_conn(&cat.conn, &root_str)?.is_some() {
                            results.push(json!({
                                "root": r.to_string_lossy(),
                                "skipped": "active worktree registration — merge_worktree first",
                            }));
                            continue;
                        }
                        let (a, c) = prune_dead_root(&cat.conn, r)?;
                        ta += a;
                        tc += c;
                        results.push(json!({
                            "root": r.to_string_lossy(),
                            "artifact_rows": a, "commit_rows": c,
                        }));
                    }
                    Ok(json!({
                        "fix": "prune_missing", "mode": "applied",
                        "pruned": results,
                        "totals": { "artifact_rows": ta, "commit_rows": tc },
                    }))
                }
            }
        }
        "reseat_worktree" => reseat_worktree(ctx),
        other => Err(RecoverableError::new(format!(
            "unknown fix '{other}' — expected 'prune_missing' or 'reseat_worktree'"
        ))),
    }
}

/// `fix=reseat_worktree`: consume `scan_worktree_scoped` violations. Rows
/// where an ACTIVE `worktree_registration` covers the worktree root
/// (`detail.registered == true`) are SKIPPED — they are pending
/// `librarian(action="merge_worktree")`, which folds registered shadows onto
/// their main-repo counterparts via the overlay, not this legacy reseat
/// path; reseating them here would sever the row from the registration
/// bookkeeping `merge_worktree` depends on. Skipped rows are reported under
/// `skipped`, not silently dropped.
///
/// For each remaining (unregistered) `no_collision` row (`id_w`, at the
/// worktree path), durably re-seed a row at the main-repo path instead of a
/// bare `abs_path` UPDATE. Catalog identity is `id ==
/// artifact_id_from_abs(abs_path)`; keeping `id_w` while pointing `abs_path`
/// at the main path would leave that invariant broken, and the next
/// MAIN-repo reindex's [`artifact::upsert`] pre-clean (`DELETE FROM
/// artifact WHERE abs_path=? AND id != ?`) would delete the row — cascading
/// away its events / links / event_edges / augmentation (the git-invisible
/// `append_entry` history this feature exists to preserve).
///
/// Instead: seed a fresh row at `id_m = artifact_id_from_abs(main_path)`
/// (`no_collision` means nothing lives there yet, so the pre-clean deletes
/// nothing), then [`graft::graft_rows`] folds `id_w`'s entire history —
/// including the augmentation — onto `id_m` and deletes `id_w`. A subsequent
/// reindex now hits `ON CONFLICT(id)` (id already matches path) instead of
/// the pre-clean `DELETE`, so nothing is lost. `collision` rows are left
/// untouched and reported for a manual `graft`.
fn reseat_worktree(ctx: &ToolContext) -> Result<Value> {
    let mut cat = ctx.catalog.lock();
    // Owned Vec: the immutable borrow of `cat.conn` ends here, before the
    // mutable `graft_rows` calls below.
    let violations = scan_worktree_scoped(&cat.conn)?;
    let mut reseated = Vec::new();
    let mut collisions = Vec::new();
    let mut skipped = Vec::new();
    for v in &violations {
        let Some(id_w) = v.artifact_id.as_deref() else {
            continue;
        };
        let detail: Value = serde_json::from_str(&v.detail).unwrap_or_default();
        if detail["registered"].as_bool() == Some(true) {
            skipped.push(json!({
                "id": id_w,
                "main_path": detail["main_path"].clone(),
                "reason": "registered — pending librarian(action=\"merge_worktree\"), not reseat_worktree",
            }));
            continue;
        }
        let main_path = detail["main_path"].as_str().unwrap_or_default();
        match detail["classification"].as_str() {
            Some("no_collision") => {
                let Some(row_w) = artifact::get(&cat, id_w)? else {
                    continue; // race: row vanished since the scan; nothing to reseat
                };
                let id_m = ids::artifact_id_from_abs(Path::new(main_path));
                let row_m = ArtifactRow {
                    id: id_m.clone(),
                    abs_path: PathBuf::from(main_path),
                    ..row_w
                };
                // Two separate transactions (`upsert` autocommits; `graft_rows`
                // runs its own IMMEDIATE tx) — acceptable for a manual
                // diagnostic. A crash between them is recoverable, not data
                // loss: either an orphan `id_m` row with no history yet, or an
                // un-grafted `id_w` that the next run's scan reports as a
                // `collision` against `id_m` for a manual `graft`.
                artifact::upsert(&cat, &row_m)?;
                graft::graft_rows(&mut cat, id_w, &id_m)?;
                reseated.push(json!({
                    "old_id": id_w,
                    "new_id": id_m,
                    "new_path": main_path,
                }));
            }
            _ => collisions.push(json!({
                "id": id_w,
                "main_path": main_path,
                "into_id": detail["collision_with"].clone(),
            })),
        }
    }
    drop(cat);
    Ok(json!({
        "fix": "reseat_worktree",
        "reseated": reseated,
        "collisions": collisions,
        "skipped": skipped,
    }))
}

/// Delete every catalog row anchored under a dead repo `root`: `artifact` rows
/// whose `abs_path` is `root` or under `root/`, and `commits` rows whose
/// `git_root` is `root` or under `root/`. Runs through codescout's own
/// (vec0-linked, trusted-schema) connection, so the `artifact_vec` cascade
/// trigger and the FK `ON DELETE CASCADE`s (augmentation / links / events) all
/// fire — a bare `sqlite3` CLI cannot (7ca71bf7). Returns (artifact_rows,
/// commit_rows) removed.
fn prune_dead_root(conn: &rusqlite::Connection, root: &std::path::Path) -> Result<(usize, usize)> {
    let root_fwd = format!("{}", crate::util::fs::RepoPath::from_path(root));
    let under = format!("{root_fwd}/%");
    let artifact_rows = conn.execute(
        "DELETE FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2",
        rusqlite::params![root_fwd, under],
    )?;
    let commit_rows = conn.execute(
        "DELETE FROM commits WHERE git_root = ?1 OR git_root LIKE ?2",
        rusqlite::params![root_fwd, under],
    )?;
    Ok((artifact_rows, commit_rows))
}

/// Distinct DEAD ROOTS to prune, derived from the catalog's missing rows. A
/// missing artifact is included ONLY if its parent directory is ALSO missing (a
/// whole subtree is gone, not a single file under a live dir — single-file
/// deletions under a live repo are reindex's job). The dead root is the highest
/// nonexistent ancestor whose parent still exists. Returns a sorted, de-duped list.
fn derive_dead_roots(conn: &rusqlite::Connection) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut stmt = conn.prepare("SELECT abs_path FROM artifact")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut roots = std::collections::BTreeSet::new();
    for p in &paths {
        let path = std::path::Path::new(p);
        // Malformed (non-absolute) abs_path rows are abs_path_must_be_absolute's
        // eviction job, not a dead-root: skip them so the climb never bottoms out
        // at an empty PathBuf (whose prune WHERE would match every absolute row).
        if !path.is_absolute() {
            continue;
        }
        if path.exists() {
            continue; // not a missing row
        }
        match path.parent() {
            Some(parent) if parent.exists() => continue, // single file under a live dir
            None => continue,
            _ => {}
        }
        // Walk up to the highest nonexistent ancestor whose parent exists.
        let mut dead = path.to_path_buf();
        while let Some(parent) = dead.parent() {
            if parent.exists() {
                break;
            }
            dead = parent.to_path_buf();
        }
        roots.insert(dead);
    }
    Ok(roots.into_iter().collect())
}

/// Read-only count of `(artifact_rows, commit_rows)` under `root`, mirroring the
/// WHERE clauses `prune_dead_root` deletes with.
fn count_dead_root(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
) -> anyhow::Result<(usize, usize)> {
    let root_fwd = format!("{}", crate::util::fs::RepoPath::from_path(root));
    let under = format!("{root_fwd}/%");
    let arts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    let commits: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commits WHERE git_root = ?1 OR git_root LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    Ok((arts.max(0) as usize, commits.max(0) as usize))
}

/// Pulls every `(id, abs_path)` row once and runs five per-row checks
/// (abs_path_must_be_absolute / backslash / ads_colon / dotdot /
/// missing_file). Single SQL fetch + in-memory passes is cheaper than five
/// separate queries. `abs_path_must_be_absolute` runs first because it is
/// the gating shape check — a relative-path row should be evicted, not
/// further analyzed.
fn scan_artifact_paths(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    for (id, abs_path) in &rows {
        if let Some(v) = check_abs_path_must_be_absolute(id, abs_path) {
            violations.push(v);
        }
        if let Some(v) = check_backslash(id, abs_path, "backslash_in_abs_path") {
            violations.push(v);
        }
        if let Some(v) = check_ads_colon(id, abs_path) {
            violations.push(v);
        }
        if let Some(v) = check_dotdot_segment(id, abs_path) {
            violations.push(v);
        }
        if let Some(v) = check_missing_file(id, abs_path) {
            violations.push(v);
        }
    }
    Ok(violations)
}

fn scan_commits_git_root(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    // `commits.git_root` carries normalized paths (since #66). A backslash
    // here is pre-migration drift, same shape as the artifact-side check
    // but without an artifact_id anchor.
    let mut stmt = conn.prepare("SELECT DISTINCT git_root FROM commits")?;
    let roots: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    for root in &roots {
        if let Some(pos) = root.find('\\') {
            violations.push(Violation::new(
                "backslash_in_git_root",
                None,
                root.clone(),
                format!("backslash at byte position {pos}"),
            ));
        }
    }
    Ok(violations)
}

/// Computes the would-be path of a worktree-scoped row in the MAIN repo:
/// `abs_path` re-rooted from `worktree_root` onto `main_root`. `None` if
/// `abs_path` is not actually under `worktree_root` — defensive; the only
/// caller invokes this after confirming ancestry via `is_linked_worktree`.
fn main_path_for(abs_path: &Path, worktree_root: &Path, main_root: &Path) -> Option<PathBuf> {
    let rel = abs_path.strip_prefix(worktree_root).ok()?;
    Some(main_root.join(rel))
}

/// Reads `artifact_augmentation.entry_collection` + `params` for `artifact_id`.
/// `None` if the row is unaugmented.
fn augmentation_entry_collection(
    conn: &rusqlite::Connection,
    artifact_id: &str,
) -> Result<Option<(Option<String>, String)>> {
    let row = conn
        .query_row(
            "SELECT entry_collection, params FROM artifact_augmentation WHERE artifact_id = ?1",
            rusqlite::params![artifact_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    Ok(row)
}

/// Extracts the `id` field of every object in `params_json[collection]`
/// (skipping entries with no string `id`). Best-effort: malformed JSON or a
/// missing/non-array collection yields an empty list rather than an error —
/// this feeds diagnostic detail, not the collision classification itself.
fn entry_ids(params_json: &str, collection: &str) -> Vec<String> {
    let Ok(parsed) = serde_json::from_str::<Value>(params_json) else {
        return Vec::new();
    };
    parsed
        .get(collection)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("id").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Given both sides of a `worktree_scoped_row` collision, returns the
/// overlapping entry ids IF both rows are augmented with the SAME non-null
/// `entry_collection` name. `None` means that precondition isn't met (either
/// row unaugmented, or the collection names differ/are absent) — distinct
/// from `Some(vec![])`, which means the precondition held but nothing
/// actually overlapped.
fn shared_entry_overlap(
    conn: &rusqlite::Connection,
    row_id: &str,
    other_id: &str,
) -> Result<Option<Vec<String>>> {
    let Some((row_collection, row_params)) = augmentation_entry_collection(conn, row_id)? else {
        return Ok(None);
    };
    let Some((other_collection, other_params)) = augmentation_entry_collection(conn, other_id)?
    else {
        return Ok(None);
    };
    let (Some(rc), Some(oc)) = (row_collection, other_collection) else {
        return Ok(None);
    };
    if rc != oc {
        return Ok(None);
    }
    let row_ids = entry_ids(&row_params, &rc);
    let other_ids = entry_ids(&other_params, &oc);
    Ok(Some(
        row_ids
            .into_iter()
            .filter(|i| other_ids.contains(i))
            .collect(),
    ))
}

/// Flags artifact rows whose `abs_path` lives inside a linked git worktree.
/// For each such row, computes the row's would-be path in the MAIN repo and
/// classifies whether a catalog row already exists there:
///
/// - `no_collision` — no row at the main-repo path; the worktree-scoped row
///   is merely absent from the main catalog view, not conflicting with it.
/// - `collision` — a row already exists at the main-repo path (same
///   [`ids::artifact_id_from_abs`]). If both rows are augmented with the
///   SAME `entry_collection`, the overlapping entry ids are surfaced too —
///   `fix=reseat_worktree` (a follow-up change) will need this to merge
///   safely instead of clobbering.
///
/// Every row also carries `registered`: whether an ACTIVE
/// `worktree_registration` covers the row's worktree root. Registered rows
/// are pending a `librarian(action="merge_worktree")`, not a `reseat` —
/// `fix=reseat_worktree` skips them (see [`reseat_worktree`]) and the detail
/// carries a `hint` pointing at `merge_worktree` instead.
///
/// Filesystem-only: walks each `abs_path`'s ancestor directories looking for
/// one [`current_project::is_linked_worktree`] recognizes (a `.git` *file*
/// containing a `gitdir: .../worktrees/<name>` pointer) — no `git` subprocess.
fn scan_worktree_scoped(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    for (id, abs_path) in &rows {
        let path = Path::new(abs_path);
        let Some(worktree_root) = path
            .ancestors()
            .find(|a| current_project::is_linked_worktree(a))
        else {
            continue;
        };
        let Some(main_root) = current_project::worktree_main_root(worktree_root) else {
            continue;
        };
        let Some(main_path) = main_path_for(path, worktree_root, &main_root) else {
            continue;
        };
        let main_id = ids::artifact_id_from_abs(&main_path);
        let main_path_str = crate::util::fs::RepoPath::from_path(&main_path).to_string();

        let exists_at_main: bool = conn
            .query_row(
                "SELECT 1 FROM artifact WHERE id = ?1",
                rusqlite::params![main_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();

        let worktree_root_str = crate::util::fs::RepoPath::from_path(worktree_root).to_string();
        let registered = worktree::covering_conn(conn, &worktree_root_str)?.is_some();

        let mut detail = json!({
            "main_path": main_path_str,
            "classification": if exists_at_main { "collision" } else { "no_collision" },
            "registered": registered,
        });

        if exists_at_main {
            detail["collision_with"] = json!(main_id);
            if let Some(overlap) = shared_entry_overlap(conn, id, &main_id)? {
                detail["id_overlap"] = json!(overlap);
            }
        }

        if registered {
            detail["hint"] = json!("pending merge — use librarian(action=\"merge_worktree\")");
        }

        violations.push(Violation::new(
            "worktree_scoped_row",
            Some(id.clone()),
            abs_path.clone(),
            detail.to_string(),
        ));
    }
    Ok(violations)
}

fn check_backslash(id: &str, abs_path: &str, check_name: &str) -> Option<Violation> {
    abs_path.find('\\').map(|pos| {
        Violation::new(
            check_name,
            Some(id.to_string()),
            abs_path,
            format!("backslash at byte position {pos}"),
        )
    })
}

fn check_ads_colon(id: &str, abs_path: &str) -> Option<Violation> {
    // Exempt the Windows drive-letter slot (`C:`) from being flagged as an
    // NTFS alternate-data-stream selector. `drive_letter_prefix_len` already
    // accounts for the `//?/` verbatim marker `fs::canonicalize` prepends on
    // Windows, so the drive-letter colon isn't mistaken for a real ADS colon
    // (false positive on every Windows-indexed row otherwise).
    let prefix_len = crate::util::fs::drive_letter_prefix_len(abs_path).unwrap_or(0);
    let tail = &abs_path[prefix_len..];
    tail.find(':').map(|pos_in_tail| {
        let absolute_pos = pos_in_tail + prefix_len;
        Violation::new(
            "ads_colon_in_abs_path",
            Some(id.to_string()),
            abs_path,
            format!("colon at byte position {absolute_pos} (outside drive prefix)"),
        )
    })
}

fn check_dotdot_segment(id: &str, abs_path: &str) -> Option<Violation> {
    if abs_path.split('/').any(|seg| seg == "..") {
        Some(Violation::new(
            "dotdot_segment_in_abs_path",
            Some(id.to_string()),
            abs_path,
            "path contains a '..' segment",
        ))
    } else {
        None
    }
}

fn check_missing_file(id: &str, abs_path: &str) -> Option<Violation> {
    if std::path::Path::new(abs_path).exists() {
        None
    } else {
        Some(Violation::new(
            "missing_file",
            Some(id.to_string()),
            abs_path,
            "file does not exist on disk",
        ))
    }
}

fn check_abs_path_must_be_absolute(id: &str, abs_path: &str) -> Option<Violation> {
    // Schema declares `abs_path TEXT NOT NULL UNIQUE` but does not enforce
    // absoluteness. Pre-#66 code paths stored relative strings here in some
    // cases; the doctor catches the wrong-shape rows so they can be migrated
    // (or evicted via reindex) rather than masquerading as `missing_file`
    // false positives (Path::exists resolves them against the caller's cwd).
    //
    // Absolute on the platforms we care about:
    //   - POSIX: leading `/` (also covers the Windows verbatim-prefix form
    //     `//?/C:/...`, which starts with `/`).
    //   - Windows: leading `<drive>:` (`C:`, `D:`, …), bare or verbatim-prefixed
    //     — see `drive_letter_prefix_len`.
    //   - Windows UNC `\\server\share` is allowed in theory but extremely
    //     unusual in our content corpus; if it ever appears the
    //     `backslash_in_abs_path` check catches it first.
    let starts_with_posix_root = abs_path.as_bytes().first() == Some(&b'/');
    let starts_with_drive = crate::util::fs::drive_letter_prefix_len(abs_path).is_some();
    if starts_with_posix_root || starts_with_drive {
        return None;
    }
    Some(Violation::new(
        "abs_path_must_be_absolute",
        Some(id.to_string()),
        abs_path,
        "abs_path is relative — schema requires absolute form (leading '/' or '<drive>:')",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, upsert as art_upsert, TestArtifactRowBuilder};
    use crate::librarian::catalog::augmentation;
    use crate::librarian::catalog::events::{self, TestEventRowBuilder};
    use crate::librarian::catalog::worktree as reg;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use rusqlite::params;

    fn seed_artifact(cat: &Catalog, id: &str, abs_path: &str) {
        cat.conn
            .execute(
                "INSERT INTO artifact \
                 (id, abs_path, kind, status, created_at, updated_at, file_mtime, file_sha256) \
                 VALUES (?1, ?2, 'spec', 'active', 0, 0, 0, '')",
                params![id, abs_path],
            )
            .unwrap();
    }

    fn seed_commit(cat: &Catalog, hash: &str, git_root: &str) {
        cat.conn
            .execute(
                "INSERT INTO commits (hash, git_root) VALUES (?1, ?2)",
                params![hash, git_root],
            )
            .unwrap();
    }

    #[test]
    fn check_backslash_finds_byte_position() {
        let v = check_backslash("a1", "C:/foo\\bar.md", "backslash_in_abs_path").unwrap();
        assert_eq!(v.check, "backslash_in_abs_path");
        assert_eq!(v.artifact_id.as_deref(), Some("a1"));
        assert_eq!(v.path, "C:/foo\\bar.md");
        assert!(v.detail.contains("position 6"));
    }

    #[test]
    fn check_backslash_skips_clean_path() {
        assert!(check_backslash("a1", "/home/x/foo.md", "backslash_in_abs_path").is_none());
        assert!(check_backslash("a1", "C:/users/x/foo.md", "backslash_in_abs_path").is_none());
    }

    #[test]
    fn check_ads_colon_exempts_drive_prefix() {
        assert!(check_ads_colon("a1", "C:/Users/marius/foo.md").is_none());
        assert!(check_ads_colon("a1", "/home/marius/foo.md").is_none());
    }

    #[test]
    fn check_ads_colon_flags_post_drive_colon() {
        let v = check_ads_colon("a1", "C:/foo.txt:stream").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
        assert!(v.detail.contains("position"));
    }

    #[test]
    fn check_ads_colon_flags_colon_without_drive_prefix() {
        // POSIX path with a literal colon would be exotic but legal; treat
        // as suspicious because on a cross-platform catalog it almost
        // always means corruption.
        let v = check_ads_colon("a1", "/home/foo:bar").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
    }

    #[test]
    fn check_ads_colon_exempts_verbatim_prefix_drive_colon() {
        // fs::canonicalize on Windows yields the extended-length verbatim
        // form; stored here in forward-slash rendering. The drive-letter
        // colon at byte 5 must not be flagged.
        assert!(check_ads_colon("a1", "//?/C:/Users/marius/foo.md").is_none());
    }

    #[test]
    fn check_ads_colon_flags_ads_colon_after_verbatim_prefix() {
        let v = check_ads_colon("a1", "//?/C:/foo.txt:stream").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
        assert!(v.detail.contains("position 14"));
    }

    #[test]
    fn check_dotdot_segment_flags_only_segment_dotdot() {
        assert!(check_dotdot_segment("a1", "/home/x/../etc").is_some());
        assert!(check_dotdot_segment("a1", "/home/x/..").is_some());
        assert!(check_dotdot_segment("a1", "..").is_some());
        // Filename with two dots is NOT a path escape — must not flag.
        assert!(check_dotdot_segment("a1", "/home/x/foo..bar.md").is_none());
        assert!(check_dotdot_segment("a1", "/home/x/.hidden").is_none());
    }

    #[test]
    fn check_missing_file_for_obviously_absent_path() {
        let v = check_missing_file("a1", "/nonexistent/path/that/will/never/exist.md").unwrap();
        assert_eq!(v.check, "missing_file");
    }

    #[test]
    fn check_abs_path_must_be_absolute_accepts_posix_and_drive() {
        assert!(check_abs_path_must_be_absolute("a1", "/home/x/foo.md").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "/").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "C:/Users/x/foo.md").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "z:/").is_none());
    }

    #[test]
    fn check_abs_path_must_be_absolute_flags_relative() {
        let v = check_abs_path_must_be_absolute("a1", "docs/foo.md").unwrap();
        assert_eq!(v.check, "abs_path_must_be_absolute");
        assert_eq!(v.path, "docs/foo.md");
        assert!(v.detail.contains("relative"));

        // Relative with drive-shape but missing colon at pos 1 — still wrong
        assert!(check_abs_path_must_be_absolute("a1", "Cusers/foo.md").is_some());
        // Empty string is not absolute (no leading slash)
        assert!(check_abs_path_must_be_absolute("a1", "").is_some());
    }

    #[tokio::test]
    async fn doctor_call_surfaces_seeded_drift() {
        let cat = Catalog::open_in_memory().unwrap();
        // 6 seeded artifact rows + 1 commit row. Each fault triggers ALL
        // applicable checks (e.g. a backslash path also fails `missing_file`
        // because no host file lives at the bogus path) — so we assert
        // by per-check counts, not by total.
        seed_artifact(&cat, "bad-backslash", "C:/users\\marius\\foo.md");
        seed_artifact(&cat, "bad-ads", "C:/users/foo.txt:stream");
        seed_artifact(&cat, "bad-dotdot", "/home/marius/../etc/passwd");
        seed_artifact(&cat, "bad-missing", "/definitely/not/a/real/path.md");
        // Wrong-shape row — relative string stored where abs is required.
        // Found in the wild during the post-#69 live-catalog smoke test.
        seed_artifact(&cat, "bad-relative", "docs/issues/foo.md");
        // Clean path: absolute, exists, forward-slash form, no backslash / ADS
        // colon / `..` — so it trips none of the checks (notably not missing_file).
        // Must exist on the host running the suite, so it is platform-specific:
        // `/tmp` on unix, `C:/Windows` on Windows (the drive colon is not an ADS
        // colon — same reason the `C:/` seeds above fire only their other checks).
        #[cfg(unix)]
        let clean_path = "/tmp";
        #[cfg(windows)]
        let clean_path = "C:/Windows";
        seed_artifact(&cat, "clean", clean_path);
        seed_commit(&cat, "abc123", "C:/users\\marius");

        let v = scan_artifact_paths(&cat.conn).unwrap();
        let mut by_check: std::collections::BTreeMap<&str, usize> = Default::default();
        for x in &v {
            *by_check.entry(x.check.as_str()).or_insert(0) += 1;
        }
        assert_eq!(by_check.get("backslash_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("ads_colon_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("dotdot_segment_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("abs_path_must_be_absolute").copied(), Some(1));
        // 5 missing-file hits: bad-backslash, bad-ads, bad-dotdot, bad-missing,
        // and bad-relative (Path::exists on "docs/issues/foo.md" resolves
        // against the test runner's cwd and finds nothing). clean_path exists, so
        // it does not fire.
        assert_eq!(by_check.get("missing_file").copied(), Some(5));

        let r = scan_commits_git_root(&cat.conn).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].check, "backslash_in_git_root");
    }

    #[test]
    fn validate_prune_request_gates() {
        let cat = Catalog::open_in_memory().unwrap();

        // unknown fix → refused (rejected before any path check)
        assert!(validate_prune_request("zap", Some("/gone"), &cat.conn).is_err());
        // missing root → refused
        assert!(validate_prune_request("prune_missing", None, &cat.conn).is_err());
        // relative root → refused (relative on every platform)
        assert!(validate_prune_request("prune_missing", Some("relative/path"), &cat.conn).is_err());

        // live root refused — an existing absolute dir is not an orphan. Derive it from
        // the OS temp dir so the path is absolute AND present on every platform (Unix
        // /tmp, Windows C:\…\Temp); a hard-coded "/tmp" is not absolute on Windows and
        // broke this test under wine / windows (BUG 36d475f3).
        let live = std::env::temp_dir();
        let live = live.to_str().expect("temp_dir path is valid UTF-8");
        assert!(validate_prune_request("prune_missing", Some(live), &cat.conn).is_err());

        // dead absolute root → accepted. Build a temp-dir-rooted path that does not
        // exist, so it is absolute on every platform.
        let dead = std::env::temp_dir().join("codescout-nonexistent-root-6f3a1c9e");
        assert!(!dead.exists(), "test fixture path must not exist");
        let dead = dead.to_str().expect("temp path is valid UTF-8");
        assert!(validate_prune_request("prune_missing", Some(dead), &cat.conn).is_ok());
    }

    #[test]
    fn prune_dead_root_removes_rows_under_root_only() {
        let cat = Catalog::open_in_memory().unwrap();
        // Rows under a dead root /gone/repo (exact + nested).
        seed_artifact(&cat, "g1", "/gone/repo");
        seed_artifact(&cat, "g2", "/gone/repo/a.md");
        seed_artifact(&cat, "g3", "/gone/repo/docs/b.md");
        // A sibling row that merely shares a path PREFIX string but is a
        // different repo — must NOT be pruned (no false LIKE match).
        seed_artifact(&cat, "sib", "/gone/repo-other/c.md");
        // An unrelated live row.
        seed_artifact(&cat, "keep", "/tmp/keep.md");
        seed_commit(&cat, "deadc0de", "/gone/repo");
        seed_commit(&cat, "livecdef", "/tmp");

        let (arts, commits) =
            prune_dead_root(&cat.conn, std::path::Path::new("/gone/repo")).unwrap();
        assert_eq!(arts, 3, "the 3 rows at/under /gone/repo are removed");
        assert_eq!(commits, 1, "the /gone/repo commit is removed");

        // Survivors: the prefix-sibling and the unrelated row remain.
        let exists = |id: &str| -> i64 {
            cat.conn
                .query_row("SELECT COUNT(*) FROM artifact WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert_eq!(
            exists("sib"),
            1,
            "/gone/repo-other not matched by the prefix"
        );
        assert_eq!(exists("keep"), 1);
        let n_com: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM commits", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n_com, 1, "only the /tmp commit remains");
    }

    #[test]
    fn derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files() {
        let cat = Catalog::open_in_memory().unwrap();
        let live = tempfile::tempdir().unwrap(); // exists on disk

        // Boundary that "still exists" (e.g. a real ~/work/ parent dir); the
        // deleted repo lives one level below it and is never created.
        let dead_parent = tempfile::tempdir().unwrap();
        let dead_root = dead_parent.path().join("repo");

        // (a) whole subtree gone: parent dir does not exist -> included.
        seed_artifact(&cat, "a1", &dead_root.join("docs/x.md").to_string_lossy());
        seed_artifact(&cat, "a2", &dead_root.join("docs/y.md").to_string_lossy());
        // (b) single missing file under a LIVE dir -> excluded (reindex's job).
        let missing_under_live = live.path().join("gone.md");
        seed_artifact(&cat, "b1", &missing_under_live.to_string_lossy());
        // (c) a live file -> not missing, excluded.
        let live_file = live.path().join("here.md");
        std::fs::write(&live_file, "x").unwrap();
        seed_artifact(&cat, "c1", &live_file.to_string_lossy());

        let roots = derive_dead_roots(&cat.conn).unwrap();
        assert_eq!(
            roots,
            vec![dead_root],
            "only the gone subtree's highest-nonexistent-ancestor is a dead root"
        );
    }

    #[test]
    fn derive_dead_roots_skips_non_absolute_paths() {
        // A malformed relative abs_path row must NOT yield a dead root — otherwise
        // the climb bottoms out at an empty PathBuf whose prune matches everything.
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "rel", "relative/does/not/exist.md");
        let roots = derive_dead_roots(&cat.conn).unwrap();
        assert!(
            roots.is_empty(),
            "non-absolute row must not yield a dead root, got: {roots:?}"
        );
    }

    #[test]
    fn count_dead_root_counts_rows_under_root() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "a1", "/nonexistent-root/repo/docs/x.md");
        seed_artifact(&cat, "a2", "/nonexistent-root/repo/y.md");
        seed_artifact(&cat, "z1", "/nonexistent-root/other/z.md");
        // Prefix sibling: /nonexistent-root/repo-other must NOT match the
        // LIKE '/nonexistent-root/repo/%' clause scoped to .../repo.
        seed_artifact(&cat, "sibling", "/nonexistent-root/repo-other/z.md");
        seed_commit(&cat, "deadbeef", "/nonexistent-root/repo");
        seed_commit(&cat, "cafef00d", "/nonexistent-root/repo-other");
        let (arts, commits) =
            count_dead_root(&cat.conn, std::path::Path::new("/nonexistent-root/repo")).unwrap();
        assert_eq!(
            arts, 2,
            "prefix-sibling repo-other rows must not be counted under repo"
        );
        assert_eq!(
            commits, 1,
            "prefix-sibling commit git_root must not be counted under repo"
        );
    }

    #[tokio::test]
    async fn prune_missing_batch_dry_run_lists_dead_roots_without_deleting() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "a1", "/nonexistent-root/repo/docs/x.md");
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing" })).await.unwrap(); // no root, no confirm
        assert_eq!(v["mode"], "dry_run");
        assert_eq!(v["totals"]["artifact_rows"].as_u64().unwrap(), 1);
        // Nothing deleted.
        assert!(artifact::get(&ctx.catalog.lock(), "a1").unwrap().is_some());
    }

    #[tokio::test]
    async fn prune_missing_batch_confirm_prunes_dead_roots_only() {
        let cat = Catalog::open_in_memory().unwrap();
        let live = tempfile::tempdir().unwrap();
        let live_file = live.path().join("here.md");
        std::fs::write(&live_file, "x").unwrap();
        seed_artifact(&cat, "dead", "/nonexistent-root/repo/x.md"); // gone subtree
        seed_artifact(&cat, "live", &live_file.to_string_lossy()); // live file
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing", "confirm": true }))
            .await
            .unwrap();
        assert_eq!(v["mode"], "applied");
        assert_eq!(v["totals"]["artifact_rows"].as_u64().unwrap(), 1);
        assert!(
            artifact::get(&ctx.catalog.lock(), "dead")
                .unwrap()
                .is_none(),
            "dead row pruned"
        );
        assert!(
            artifact::get(&ctx.catalog.lock(), "live")
                .unwrap()
                .is_some(),
            "live row kept"
        );
    }

    #[test]
    fn scan_worktree_scoped_empty_when_no_worktree_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        // Plain rows with no linked-worktree ancestor anywhere on disk —
        // the scan must not flag anything (safe default).
        seed_artifact(&cat, "plain", "/tmp/plain/doc.md");
        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert!(violations.is_empty());
    }

    /// Builds a real `<tmp>/main` + linked-worktree-under-main layout on disk
    /// (a `.git` FILE at the worktree root pointing `gitdir:` back at the
    /// main repo's `.git/worktrees/<name>`), matching exactly what
    /// `is_linked_worktree` / `worktree_main_root` read. Returns
    /// `(tmp, main_root, worktree_root)`; `tmp` must stay alive for the
    /// duration of the test (dropping it deletes the directory).
    fn make_worktree_fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let main_root = tmp.path().join("main");
        std::fs::create_dir_all(main_root.join(".git")).unwrap();
        let worktree_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(&worktree_root).unwrap();
        std::fs::write(
            worktree_root.join(".git"),
            format!(
                "gitdir: {}/main/.git/worktrees/feat\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        (tmp, main_root, worktree_root)
    }

    #[test]
    fn scan_worktree_scoped_classifies_no_collision() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        // Proves is_linked_worktree/worktree_main_root actually fired: with
        // no real .git-file layout on disk this would be empty (see the
        // no-worktree-rows test above). Here it must find exactly the one
        // seeded row.
        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(violations.len(), 1, "the worktree-scoped row is flagged");
        let v = &violations[0];
        assert_eq!(v.check, "worktree_scoped_row");
        assert_eq!(v.artifact_id.as_deref(), Some("wt-row"));

        let detail: serde_json::Value = serde_json::from_str(&v.detail).unwrap();
        assert_eq!(detail["classification"], "no_collision");
        assert!(detail.get("collision_with").is_none());
        let main_doc = main_root.join("docs/x.md");
        assert_eq!(
            detail["main_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );
    }

    #[test]
    fn scan_worktree_scoped_classifies_collision_and_overlap() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        // The collision classifier computes the main-path id via
        // artifact_id_from_abs and checks whether a row with that id
        // exists — so the seeded main-side row's id MUST be exactly that.
        let main_id = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        let main_row = TestArtifactRowBuilder::new(&main_id)
            .with_abs_path(main_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &main_row).unwrap();

        // Augment both sides with the SAME entry_collection name but
        // partially-overlapping id sets — "b" is the only shared id.
        augmentation::upsert(&cat, &aug_row("wt-row", "items", &["a", "b"])).unwrap();
        augmentation::upsert(&cat, &aug_row(&main_id, "items", &["b", "c"])).unwrap();

        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(
            violations.len(),
            1,
            "only the worktree-side row is scanned; the main-side row has no \
             linked-worktree ancestor and is skipped"
        );
        let v = &violations[0];
        assert_eq!(v.artifact_id.as_deref(), Some("wt-row"));

        let detail: serde_json::Value = serde_json::from_str(&v.detail).unwrap();
        assert_eq!(detail["classification"], "collision");
        assert_eq!(detail["collision_with"], main_id);
        assert_eq!(
            detail["main_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );
        let overlap: Vec<String> = detail["id_overlap"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(overlap, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn reseat_worktree_repoints_no_collision_row_without_rename() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let id_m = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, false).await.unwrap();
        assert_eq!(out["fix"], "reseat_worktree");
        assert_eq!(out["reseated"].as_array().unwrap().len(), 1);
        assert!(out["collisions"].as_array().unwrap().is_empty());
        assert_eq!(out["reseated"][0]["old_id"], "wt-row");
        assert_eq!(out["reseated"][0]["new_id"], id_m);

        // The row is durably re-seeded at id_m (== hash(main_path)) rather than
        // merely re-pointed under the stale worktree-derived id: no filesystem
        // rename (the merged file already lives there; only the catalog
        // moved), but the catalog id DOES change so identity
        // (id == hash(abs_path)) holds and the worktree-id row is gone.
        let expected_main = crate::util::fs::RepoPath::from_path(&main_doc).to_string();
        let cat = ctx.catalog.lock();
        let abs_path: String = cat
            .conn
            .query_row(
                "SELECT abs_path FROM artifact WHERE id = ?1",
                params![id_m],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(abs_path, expected_main);
        assert!(artifact::get(&cat, "wt-row").unwrap().is_none());
    }

    #[tokio::test]
    async fn reseat_worktree_leaves_collisions_for_graft() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let main_id = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        let main_row = TestArtifactRowBuilder::new(&main_id)
            .with_abs_path(main_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &main_row).unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, false).await.unwrap();
        assert!(out["reseated"].as_array().unwrap().is_empty());
        assert_eq!(out["collisions"].as_array().unwrap().len(), 1);

        // Collision rows are reported, never mutated: the worktree row's
        // abs_path must still point at the worktree, not the main path.
        let expected_wt = crate::util::fs::RepoPath::from_path(&wt_doc).to_string();
        let cat = ctx.catalog.lock();
        let abs_path: String = cat
            .conn
            .query_row(
                "SELECT abs_path FROM artifact WHERE id = ?1",
                params!["wt-row"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(abs_path, expected_wt);
    }

    /// The durability proof: a `no_collision` reseat must survive the NEXT
    /// main-repo reindex without losing history. Seeds a worktree-scoped row
    /// WITH an augmentation (`entry_collection` + params) and an event, runs
    /// `reseat_worktree`, then simulates the next reindex's `artifact::upsert`
    /// at the main path — before this fix, that upsert's abs_path-collision
    /// pre-clean (`DELETE FROM artifact WHERE abs_path=? AND id != ?`) would
    /// fire against the stale worktree-derived id and cascade-drop the
    /// augmentation; this test would have failed on `main` prior to the fix.
    #[tokio::test]
    async fn reseat_worktree_durably_reseeds_and_survives_reindex() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let id_m = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        augmentation::upsert(&cat, &aug_row("wt-row", "items", &["a", "b"])).unwrap();
        events::insert(
            &cat,
            &TestEventRowBuilder::new("wt-row", "note")
                .with_id("ev-1")
                .build(),
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, false).await.unwrap();
        assert_eq!(out["reseated"].as_array().unwrap().len(), 1);
        assert!(out["collisions"].as_array().unwrap().is_empty());
        assert_eq!(out["reseated"][0]["old_id"], "wt-row");
        assert_eq!(out["reseated"][0]["new_id"], id_m);
        assert_eq!(
            out["reseated"][0]["new_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );

        {
            let cat = ctx.catalog.lock();

            // Catalog identity restored: a row lives at id_m, the stale
            // worktree-id row is gone.
            assert!(artifact::get(&cat, &id_m).unwrap().is_some());
            assert!(artifact::get(&cat, "wt-row").unwrap().is_none());

            // The augmentation (git-invisible append_entry history) migrated.
            let aug = augmentation::get(&cat, &id_m).unwrap().unwrap();
            let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
            let ids: Vec<&str> = params["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_str().unwrap())
                .collect();
            assert_eq!(ids, vec!["a", "b"]);

            // The event followed too.
            let ev_artifact_id: String = cat
                .conn
                .query_row(
                    "SELECT artifact_id FROM events WHERE id = ?1",
                    params!["ev-1"],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ev_artifact_id, id_m);
        }

        // Simulate the NEXT main-repo reindex walk: a fresh ArtifactRow at the
        // same id a real walk would compute (hash(main_path)) upserts onto the
        // row this fix just reseeded.
        {
            let cat = ctx.catalog.lock();
            let reindexed_row = TestArtifactRowBuilder::new(&id_m)
                .with_abs_path(main_doc.clone())
                .with_kind("tracker")
                .build();
            art_upsert(&cat, &reindexed_row).unwrap();

            // The durability guarantee: the augmentation is STILL there. Before
            // the fix, id_m would have still been id_w at this point, so the
            // upsert's abs_path pre-clean would have deleted the row (and its
            // cascaded augmentation) out from under this reindex.
            let aug = augmentation::get(&cat, &id_m).unwrap().unwrap();
            let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
            assert_eq!(
                params["items"].as_array().unwrap().len(),
                2,
                "augmentation survives the next reindex"
            );
        }
    }

    /// A worktree-scoped row covered by an ACTIVE `worktree_registration` is
    /// pending merge, not a legacy orphan — `scan_worktree_scoped` must flag
    /// it as `registered` (with a hint pointing at `merge_worktree`), and
    /// `reseat_worktree` must SKIP it rather than reseating it: reseating
    /// would sever the row from the registration's overlay bookkeeping that
    /// `merge_worktree` depends on.
    #[tokio::test]
    async fn worktree_scoped_row_marks_registered_rows_pending_merge() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        let worktree_root_str = crate::util::fs::RepoPath::from_path(&worktree_root).to_string();
        let main_root_str = crate::util::fs::RepoPath::from_path(&main_root).to_string();
        reg::upsert_active(&cat, &worktree_root_str, &main_root_str, None, 1000).unwrap();

        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(violations.len(), 1);
        let detail: serde_json::Value = serde_json::from_str(&violations[0].detail).unwrap();
        assert_eq!(
            detail["registered"], true,
            "an ACTIVE registration covers this worktree root"
        );
        assert!(
            detail["hint"]
                .as_str()
                .unwrap_or_default()
                .contains("merge_worktree"),
            "registered rows point at merge_worktree, not reseat: {detail}"
        );

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = run_fix(&ctx, "reseat_worktree", None, false).await.unwrap();
        assert!(
            out["reseated"].as_array().unwrap().is_empty(),
            "a registered row must not be reseated — it belongs to merge_worktree: {out}"
        );
        assert_eq!(
            out["skipped"].as_array().unwrap().len(),
            1,
            "the registered row is reported as skipped: {out}"
        );
    }

    /// `prune_missing` must refuse a dead root that an ACTIVE registration
    /// still covers — the worktree was `git worktree remove`d before its
    /// shadow rows were merged, and pruning would delete the catalog's only
    /// remaining record of that unmerged history.
    #[test]
    fn prune_missing_refuses_root_with_active_registration() {
        let cat = Catalog::open_in_memory().unwrap();
        let dead_root = std::env::temp_dir().join("codescout-nonexistent-registered-root-9c2e7b1a");
        assert!(!dead_root.exists(), "test fixture path must not exist");
        let dead_root_str = dead_root.to_str().expect("temp path is valid UTF-8");
        let normalized = crate::util::fs::RepoPath::from_path(&dead_root).to_string();
        reg::upsert_active(&cat, &normalized, &normalized, None, 1000).unwrap();

        let err =
            validate_prune_request("prune_missing", Some(dead_root_str), &cat.conn).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("merge_worktree"),
            "hint names merge_worktree: {msg}"
        );
        assert!(msg.contains("abandon"), "hint names abandon=true: {msg}");
    }

    fn aug_row(
        artifact_id: &str,
        entry_collection: &str,
        ids: &[&str],
    ) -> crate::librarian::catalog::augmentation::AugmentationRow {
        let items: Vec<serde_json::Value> =
            ids.iter().map(|i| serde_json::json!({ "id": i })).collect();
        crate::librarian::catalog::augmentation::AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: "test prompt".to_string(),
            params: serde_json::json!({ entry_collection: items }).to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some(entry_collection.to_string()),
            refreshed_at_commit: None,
        }
    }
}
