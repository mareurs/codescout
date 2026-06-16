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

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};

/// One violation of a doctor invariant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    /// Which check fired. One of: `backslash_in_abs_path`,
    /// `ads_colon_in_abs_path`, `dotdot_segment_in_abs_path`,
    /// `missing_file`, `backslash_in_git_root`.
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
        return run_fix(ctx, fix, args.get("root").and_then(Value::as_str)).await;
    }

    let cat = ctx.catalog.lock();
    let mut all_violations: Vec<Violation> = Vec::new();

    all_violations.extend(scan_artifact_paths(&cat.conn)?);
    all_violations.extend(scan_commits_git_root(&cat.conn)?);

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

/// Validate a `prune_missing` request without touching the catalog. Returns the
/// validated dead-root path, or a `RecoverableError` for an unsupported fix, a
/// missing/relative root, or a root that still exists (a live root's rows are
/// not orphans — per-file deletions belong to reindex's walk, not a bulk prune).
fn validate_prune_request<'a>(fix: &str, root: Option<&'a str>) -> Result<&'a std::path::Path> {
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
    Ok(root_path)
}

/// Opt-in catalog repair. Currently one fix: `prune_missing` — remove every row
/// anchored under a dead/renamed repo `root`.
async fn run_fix(ctx: &ToolContext, fix: &str, root: Option<&str>) -> Result<Value> {
    let root_path = validate_prune_request(fix, root)?;
    let cat = ctx.catalog.lock();
    let (artifact_rows, commit_rows) = prune_dead_root(&cat.conn, root_path)?;
    drop(cat);
    Ok(json!({
        "fix": "prune_missing",
        "root": root_path.to_string_lossy(),
        "pruned": { "artifact_rows": artifact_rows, "commit_rows": commit_rows },
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
    // Exempt the Windows drive-letter slot at positions 0..2 (`C:`).
    // After that any colon is an NTFS alternate-data-stream selector.
    let bytes = abs_path.as_bytes();
    let starts_with_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let tail = if starts_with_drive {
        &abs_path[2..]
    } else {
        abs_path
    };
    tail.find(':').map(|pos_in_tail| {
        let absolute_pos = pos_in_tail + if starts_with_drive { 2 } else { 0 };
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
    //   - POSIX: leading `/`
    //   - Windows: leading `<drive>:` (`C:`, `D:`, …), with `[a-zA-Z]:` byte
    //     pattern at positions 0..2.
    //   - Windows UNC `\\server\share` is allowed in theory but extremely
    //     unusual in our content corpus; if it ever appears the
    //     `backslash_in_abs_path` check catches it first.
    let bytes = abs_path.as_bytes();
    let starts_with_posix_root = bytes.first() == Some(&b'/');
    let starts_with_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
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
    use crate::librarian::catalog::Catalog;
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
        // unknown fix → refused
        assert!(validate_prune_request("zap", Some("/gone")).is_err());
        // missing root → refused
        assert!(validate_prune_request("prune_missing", None).is_err());
        // relative root → refused
        assert!(validate_prune_request("prune_missing", Some("relative/path")).is_err());
        // live root refused (/tmp exists on the test host) — not an orphan
        assert!(validate_prune_request("prune_missing", Some("/tmp")).is_err());
        // dead absolute root → accepted
        assert!(
            validate_prune_request("prune_missing", Some("/definitely/not/a/real/root/xyz"))
                .is_ok()
        );
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
}
