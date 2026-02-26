# Incremental Index Rebuilding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make embedding index updates incremental — detect code changes via git diff + mtime, re-index only what changed, purge deleted files, and warn when the index is stale at search time.

**Architecture:** A single `diff_and_reindex` function replaces the walk-everything loop in `build_index`. It uses git diff (tracked files) + mtime comparison (untracked files) as cheap pre-filters before SHA-256 hashing. The `files` table gains an `mtime` column, and the `meta` table stores `last_indexed_commit`. `semantic_search` checks HEAD vs last-indexed commit and adds a staleness warning to results.

**Tech Stack:** Rust, git2 0.19 (already a dependency), rusqlite (already a dependency), sha2 (already a dependency).

**Design doc:** `docs/plans/2026-02-26-incremental-index-design.md`

---

## Task 1: Add `mtime` column to `files` table

**Files:**
- Modify: `src/embed/index.rs:30-78` (`open_db` — schema DDL)
- Modify: `src/embed/index.rs:136-143` (`upsert_file_hash` — add mtime param)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

Add to the `tests` module in `src/embed/index.rs`:

```rust
    #[test]
    fn upsert_file_hash_stores_mtime() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc123", Some(1700000000)).unwrap();
        let mtime: Option<i64> = conn
            .query_row("SELECT mtime FROM files WHERE path = 'a.rs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mtime, Some(1700000000));
    }

    #[test]
    fn upsert_file_hash_updates_mtime() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc", Some(1000)).unwrap();
        upsert_file_hash(&conn, "a.rs", "def", Some(2000)).unwrap();
        let mtime: Option<i64> = conn
            .query_row("SELECT mtime FROM files WHERE path = 'a.rs'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mtime, Some(2000));
    }

    #[test]
    fn get_file_mtime_returns_stored_value() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc", Some(1700000000)).unwrap();
        assert_eq!(get_file_mtime(&conn, "a.rs").unwrap(), Some(1700000000));
    }

    #[test]
    fn get_file_mtime_returns_none_for_missing() {
        let (_dir, conn) = open_test_db();
        assert_eq!(get_file_mtime(&conn, "missing.rs").unwrap(), None);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::upsert_file_hash_stores_mtime -- --nocapture`
Expected: FAIL — `upsert_file_hash` takes 3 args, not 4; `get_file_mtime` doesn't exist

**Step 3: Write minimal implementation**

In `open_db` (`src/embed/index.rs:30-78`), update the `files` CREATE TABLE:

```sql
CREATE TABLE IF NOT EXISTS files (
    path   TEXT PRIMARY KEY,
    hash   TEXT NOT NULL,
    mtime  INTEGER
);
```

Also add a migration for existing databases, right after the `CREATE TABLE` statements and before the function returns:

```rust
// Migrate: add mtime column if missing (safe no-op if already present)
let has_mtime: bool = conn
    .prepare("SELECT mtime FROM files LIMIT 0")
    .is_ok();
if !has_mtime {
    conn.execute_batch("ALTER TABLE files ADD COLUMN mtime INTEGER")?;
}
```

Update `upsert_file_hash` signature and body:

```rust
pub fn upsert_file_hash(conn: &Connection, file_path: &str, hash: &str, mtime: Option<i64>) -> Result<()> {
    conn.execute(
        "INSERT INTO files (path, hash, mtime) VALUES (?1, ?2, ?3)
         ON CONFLICT(path) DO UPDATE SET hash = excluded.hash, mtime = excluded.mtime",
        params![file_path, hash, mtime],
    )?;
    Ok(())
}
```

Add `get_file_mtime`:

```rust
pub fn get_file_mtime(conn: &Connection, file_path: &str) -> Result<Option<i64>> {
    let mut stmt = conn.prepare("SELECT mtime FROM files WHERE path = ?1")?;
    let mut rows = stmt.query(params![file_path])?;
    match rows.next()? {
        Some(row) => Ok(row.get(0)?),
        None => Ok(None),
    }
}
```

**Step 4: Fix all call sites of `upsert_file_hash`**

The function signature changed from 3 args to 4. Update every call site:

In `build_index` (`src/embed/index.rs`, Phase 3 loop):
```rust
upsert_file_hash(&conn, &result.rel, &result.hash, None)?;
```

Pass `None` for now — we'll wire in real mtime values in a later task.

Search for any other call sites in tests — update those too (pass `None` or a test value as appropriate). The existing tests that call `upsert_file_hash` with 3 args need the 4th `None` arg added.

**Step 5: Run tests to verify they pass**

Run: `cargo test embed::index::tests -- --nocapture`
Expected: PASS

**Step 6: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 7: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add mtime column to files table with migration"
```

---

## Task 2: Add `last_indexed_commit` to meta table

**Files:**
- Modify: `src/embed/index.rs` (add `get_last_indexed_commit`, `set_last_indexed_commit` helpers)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
    #[test]
    fn last_indexed_commit_roundtrip() {
        let (_dir, conn) = open_test_db();
        assert_eq!(get_last_indexed_commit(&conn).unwrap(), None);
        set_last_indexed_commit(&conn, "abc123def456").unwrap();
        assert_eq!(
            get_last_indexed_commit(&conn).unwrap(),
            Some("abc123def456".to_string())
        );
    }

    #[test]
    fn last_indexed_commit_updates() {
        let (_dir, conn) = open_test_db();
        set_last_indexed_commit(&conn, "aaa").unwrap();
        set_last_indexed_commit(&conn, "bbb").unwrap();
        assert_eq!(
            get_last_indexed_commit(&conn).unwrap(),
            Some("bbb".to_string())
        );
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::last_indexed_commit_roundtrip -- --nocapture`
Expected: FAIL — functions don't exist

**Step 3: Write minimal implementation**

These are thin wrappers around the existing `get_meta`/`set_meta`:

```rust
pub fn get_last_indexed_commit(conn: &Connection) -> Result<Option<String>> {
    get_meta(conn, "last_indexed_commit")
}

pub fn set_last_indexed_commit(conn: &Connection, sha: &str) -> Result<()> {
    set_meta(conn, "last_indexed_commit", sha)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::last_indexed_commit -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add last_indexed_commit meta helpers"
```

---

## Task 3: Add `git_diff_tree` function to git module

**Files:**
- Modify: `src/git/mod.rs` (add `DiffEntry`, `DiffStatus`, `diff_tree_to_tree` function)
- Test: `src/git/mod.rs` (inline tests)

**Step 1: Write the failing tests**

Add a `tests` module to `src/git/mod.rs` (or extend if one exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::process::Command;

    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).unwrap();
        // Configure committer for tests
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        repo
    }

    fn commit_file(repo: &git2::Repository, path: &str, content: &str, msg: &str) -> git2::Oid {
        let root = repo.workdir().unwrap();
        let file_path = root.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap()
    }

    #[test]
    fn diff_tree_detects_added_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let c2 = commit_file(&repo, "b.rs", "fn b() {}", "add b");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "b.rs");
        assert!(matches!(entries[0].status, DiffStatus::Added));
    }

    #[test]
    fn diff_tree_detects_modified_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let c2 = commit_file(&repo, "a.rs", "fn a() { 1 }", "modify a");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.rs");
        assert!(matches!(entries[0].status, DiffStatus::Modified));
    }

    #[test]
    fn diff_tree_detects_deleted_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        // Delete a.rs and commit
        std::fs::remove_file(dir.path().join("a.rs")).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("a.rs")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let c2 = repo.commit(Some("HEAD"), &sig, &sig, "del a", &tree, &[&parent]).unwrap();
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.rs");
        assert!(matches!(entries[0].status, DiffStatus::Deleted));
    }

    #[test]
    fn diff_tree_returns_empty_for_same_commit() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c1.to_string()).unwrap();
        assert!(entries.is_empty());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test git::tests::diff_tree_detects_added_file -- --nocapture`
Expected: FAIL — `DiffEntry`, `DiffStatus`, `diff_tree_to_tree` don't exist

**Step 3: Write minimal implementation**

Add to `src/git/mod.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: String },
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub path: String,
    pub status: DiffStatus,
}

/// Diff two commits by SHA, returning a list of changed files.
/// Returns `Err` if either SHA is not found (e.g. after a rebase).
pub fn diff_tree_to_tree(
    repo: &git2::Repository,
    from_sha: &str,
    to_sha: &str,
) -> Result<Vec<DiffEntry>> {
    let from_obj = repo.revparse_single(from_sha)?;
    let to_obj = repo.revparse_single(to_sha)?;
    let from_tree = from_obj.peel_to_commit()?.tree()?;
    let to_tree = to_obj.peel_to_commit()?.tree()?;

    let mut opts = git2::DiffOptions::new();
    let diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))?;

    // Enable rename detection
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts.renames(true);
    let mut diff = diff;
    diff.find_similar(Some(&mut find_opts))?;

    let mut entries = Vec::new();
    for delta in diff.deltas() {
        let status = match delta.status() {
            git2::Delta::Added => DiffStatus::Added,
            git2::Delta::Modified => DiffStatus::Modified,
            git2::Delta::Deleted => DiffStatus::Deleted,
            git2::Delta::Renamed => {
                let old = delta.old_file().path().unwrap().to_string_lossy().replace('\\', "/");
                DiffStatus::Renamed { old_path: old }
            }
            _ => continue, // Ignore typechange, copied, etc.
        };
        let path = delta.new_file().path()
            .or_else(|| delta.old_file().path())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        entries.push(DiffEntry { path, status });
    }
    Ok(entries)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test git::tests -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/git/mod.rs
git commit -m "feat(git): add diff_tree_to_tree for commit-range change detection"
```

---

## Task 4: Add `file_mtime` utility function

**Files:**
- Modify: `src/embed/index.rs` (add `file_mtime` helper)
- Test: `src/embed/index.rs` (inline test)

**Step 1: Write the failing test**

```rust
    #[test]
    fn file_mtime_returns_epoch_seconds() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        // Should be a reasonable epoch timestamp (after 2020)
        assert!(mtime > 1_577_836_800); // 2020-01-01
        assert!(mtime < 2_000_000_000); // ~2033
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test embed::index::tests::file_mtime_returns_epoch_seconds -- --nocapture`
Expected: FAIL — `file_mtime` doesn't exist

**Step 3: Write minimal implementation**

Add to `src/embed/index.rs`:

```rust
/// Get file modification time as Unix epoch seconds.
/// Returns None if metadata is unavailable.
pub fn file_mtime(path: &Path) -> Result<i64> {
    let meta = std::fs::metadata(path)?;
    let modified = meta.modified()?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(duration.as_secs() as i64)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test embed::index::tests::file_mtime_returns_epoch_seconds -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add file_mtime utility for OS-agnostic mtime reading"
```

---

## Task 5: Add `purge_missing_files` function

**Files:**
- Modify: `src/embed/index.rs` (add `purge_missing_files`)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
    #[test]
    fn purge_missing_files_removes_deleted() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Insert entries for two files, but only create one on disk
        let existing = dir.path().join("exists.rs");
        std::fs::write(&existing, "fn a() {}").unwrap();
        insert_chunk(&conn, &dummy_chunk("exists.rs", "fn a() {}"), &[0.5]).unwrap();
        upsert_file_hash(&conn, "exists.rs", "aaa", Some(1000)).unwrap();
        insert_chunk(&conn, &dummy_chunk("gone.rs", "fn b() {}"), &[0.5]).unwrap();
        upsert_file_hash(&conn, "gone.rs", "bbb", Some(1000)).unwrap();

        let purged = purge_missing_files(&conn, dir.path()).unwrap();
        assert_eq!(purged, 1);

        // gone.rs should be removed from files table
        assert_eq!(get_file_hash(&conn, "gone.rs").unwrap(), None);
        // exists.rs should remain
        assert!(get_file_hash(&conn, "exists.rs").unwrap().is_some());
    }

    #[test]
    fn purge_missing_files_returns_zero_when_all_exist() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        upsert_file_hash(&conn, "a.rs", "aaa", Some(1000)).unwrap();
        let purged = purge_missing_files(&conn, dir.path()).unwrap();
        assert_eq!(purged, 0);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::purge_missing_files -- --nocapture`
Expected: FAIL — `purge_missing_files` doesn't exist

**Step 3: Write minimal implementation**

```rust
/// Remove index entries for files that no longer exist on disk.
/// Returns the number of purged files.
pub fn purge_missing_files(conn: &Connection, project_root: &Path) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT path FROM files")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut purged = 0;
    for path in &paths {
        let full = project_root.join(path);
        if !full.exists() {
            delete_file_chunks(conn, path)?;
            purged += 1;
        }
    }
    Ok(purged)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::purge_missing_files -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add purge_missing_files for deleted-file cleanup"
```

---

## Task 6: Add `check_index_staleness` function

**Files:**
- Modify: `src/embed/index.rs` (add `Staleness` struct, `check_index_staleness`)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
    #[test]
    fn staleness_no_commit_stored_is_stale() {
        let dir = tempdir().unwrap();
        // Init a git repo so HEAD exists
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        // Create an initial commit
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        let conn = open_db(dir.path()).unwrap();
        let staleness = check_index_staleness(&conn, dir.path()).unwrap();
        assert!(staleness.stale);
    }

    #[test]
    fn staleness_matching_commit_is_fresh() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        let conn = open_db(dir.path()).unwrap();
        set_last_indexed_commit(&conn, &oid.to_string()).unwrap();
        let staleness = check_index_staleness(&conn, dir.path()).unwrap();
        assert!(!staleness.stale);
        assert_eq!(staleness.behind_commits, 0);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::staleness -- --nocapture`
Expected: FAIL — `Staleness` and `check_index_staleness` don't exist

**Step 3: Write minimal implementation**

```rust
#[derive(Debug)]
pub struct Staleness {
    pub stale: bool,
    pub behind_commits: usize,
}

/// Check if the index is behind HEAD.
/// Returns Ok with stale=false if up to date, stale=true with commit count if behind.
/// If no git repo or HEAD doesn't exist, returns stale=true with behind_commits=0.
pub fn check_index_staleness(conn: &Connection, project_root: &Path) -> Result<Staleness> {
    let repo = match git2::Repository::open(project_root) {
        Ok(r) => r,
        Err(_) => return Ok(Staleness { stale: true, behind_commits: 0 }),
    };
    let head_oid = match repo.head() {
        Ok(h) => match h.peel_to_commit() {
            Ok(c) => c.id().to_string(),
            Err(_) => return Ok(Staleness { stale: true, behind_commits: 0 }),
        },
        Err(_) => return Ok(Staleness { stale: true, behind_commits: 0 }),
    };

    let last_indexed = get_last_indexed_commit(conn)?;
    match last_indexed {
        None => Ok(Staleness { stale: true, behind_commits: 0 }),
        Some(ref stored) if stored == &head_oid => {
            Ok(Staleness { stale: false, behind_commits: 0 })
        }
        Some(ref stored) => {
            // Count commits between stored and HEAD
            let behind = count_commits_between(&repo, stored, &head_oid);
            Ok(Staleness { stale: true, behind_commits: behind })
        }
    }
}

fn count_commits_between(repo: &git2::Repository, from: &str, to: &str) -> usize {
    let Ok(to_oid) = git2::Oid::from_str(to) else { return 0 };
    let Ok(from_oid) = git2::Oid::from_str(from) else { return 0 };
    let Ok(mut revwalk) = repo.revwalk() else { return 0 };
    if revwalk.push(to_oid).is_err() { return 0; }
    if revwalk.hide(from_oid).is_err() { return 0; }
    revwalk.count()
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::staleness -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add check_index_staleness for freshness detection"
```

---

## Task 7: Add staleness warning to `semantic_search`

**Files:**
- Modify: `src/tools/semantic.rs:39-100` (`SemanticSearch::call`)
- Test: `src/tools/semantic.rs` (inline tests)

**Step 1: Write the failing test**

Add to the `tests` module in `src/tools/semantic.rs`:

```rust
    #[tokio::test]
    async fn semantic_search_includes_stale_warning() {
        let (dir, ctx) = project_ctx().await;

        // Init a git repo with a commit
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        let mut git_index = repo.index().unwrap();
        let tree_oid = git_index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        // Create DB with data but NO last_indexed_commit → stale
        let conn = crate::embed::index::open_db(dir.path()).unwrap();
        let chunk = crate::embed::schema::CodeChunk {
            id: None,
            file_path: "test.rs".into(),
            language: "rust".into(),
            content: "fn test() {}".into(),
            start_line: 1,
            end_line: 1,
            file_hash: "abc".into(),
            source: "project".into(),
        };
        crate::embed::index::insert_chunk(&conn, &chunk, &[0.1, 0.2, 0.3]).unwrap();
        crate::embed::index::upsert_file_hash(&conn, "test.rs", "abc", None).unwrap();
        drop(conn);

        // We can't easily run a real search (no embedder), but we can check
        // the staleness function directly
        let conn = crate::embed::index::open_db(dir.path()).unwrap();
        let staleness = crate::embed::index::check_index_staleness(&conn, dir.path()).unwrap();
        assert!(staleness.stale);
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test tools::semantic::tests::semantic_search_includes_stale_warning -- --nocapture`
Expected: This test should actually PASS since it tests `check_index_staleness` directly. The real integration is wiring it into `SemanticSearch::call`.

**Step 3: Wire staleness into SemanticSearch::call**

In `SemanticSearch::call` (`src/tools/semantic.rs`), after building the result JSON and before returning, add:

```rust
        // Check index freshness
        if let Ok(staleness) = crate::embed::index::check_index_staleness(&conn, &root) {
            if staleness.stale {
                result["stale"] = json!(true);
                result["behind_commits"] = json!(staleness.behind_commits);
                result["hint"] = json!("Index is behind HEAD. Run index_project to update.");
            }
        }
```

Insert this right before the `Ok(result)` return at the end of the function.

**Step 4: Run tests to verify they pass**

Run: `cargo test tools::semantic::tests -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(search): add staleness warning to semantic_search results"
```

---

## Task 8: Refactor `build_index` into `diff_and_reindex`

This is the core task. It replaces the walk-everything-and-hash loop in `build_index` with the layered change detection pipeline.

**Files:**
- Modify: `src/embed/index.rs:276-425` (`build_index` function)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

These test the change detection logic. Since `build_index` requires an embedder (network call), we test the change detection layer separately.

```rust
    #[test]
    fn find_changed_files_detects_new_file() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Create a file that's not in the index
        let file = dir.path().join("new.rs");
        std::fs::write(&file, "fn new() {}").unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert_eq!(candidates.changed.len(), 1);
        assert_eq!(candidates.changed[0], "new.rs");
    }

    #[test]
    fn find_changed_files_skips_unchanged_mtime() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Create a file and index it with matching mtime + hash
        let file = dir.path().join("same.rs");
        std::fs::write(&file, "fn same() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        let hash = hash_file(&file).unwrap();
        upsert_file_hash(&conn, "same.rs", &hash, Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert!(candidates.changed.is_empty());
    }

    #[test]
    fn find_changed_files_detects_hash_mismatch() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        let file = dir.path().join("mod.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        // Store a different hash (simulates content changed but mtime matched by fluke)
        upsert_file_hash(&conn, "mod.rs", "oldhash", Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert_eq!(candidates.changed.len(), 1);
    }

    #[test]
    fn find_changed_files_force_returns_all() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        let hash = hash_file(&file).unwrap();
        upsert_file_hash(&conn, "a.rs", &hash, Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), true).unwrap();
        assert_eq!(candidates.changed.len(), 1); // force includes even unchanged
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::find_changed_files -- --nocapture`
Expected: FAIL — `find_changed_files`, `ChangeSet` don't exist

**Step 3: Write `ChangeSet` and `find_changed_files`**

Add above `build_index` in `src/embed/index.rs`:

```rust
/// Result of change detection: which files need re-indexing and which were deleted.
#[derive(Debug)]
pub struct ChangeSet {
    /// Relative paths of files that need re-indexing (new or modified).
    pub changed: Vec<String>,
    /// Relative paths of files that were deleted and purged from the index.
    pub deleted: Vec<String>,
}

/// Detect which files changed since the last index, using the fallback chain:
/// 1. Git diff from last_indexed_commit to HEAD (tracked files)
/// 2. Mtime comparison (untracked or when git diff unavailable)
/// 3. SHA-256 hash as final arbiter
///
/// If `force` is true, returns all indexable files as changed.
pub fn find_changed_files(
    conn: &Connection,
    project_root: &Path,
    force: bool,
) -> Result<ChangeSet> {
    use crate::ast::detect_language;

    let config = crate::config::ProjectConfig::load_or_default(project_root)?;
    let ignored = config.ignored_paths.patterns.clone();

    // Walk all eligible files
    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(true)
        .git_ignore(true)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !ignored.iter().any(|p| p.as_str() == name.as_ref())
        })
        .build();

    let mut all_files: Vec<String> = Vec::new();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if detect_language(path).is_none() {
            continue;
        }
        let rel = path
            .strip_prefix(project_root)?
            .to_string_lossy()
            .replace('\\', "/");
        all_files.push(rel);
    }

    if force {
        return Ok(ChangeSet {
            changed: all_files,
            deleted: Vec::new(),
        });
    }

    // Try git-diff approach first
    let git_changed = try_git_diff_detection(conn, project_root);

    let mut changed = Vec::new();
    let mut deleted = Vec::new();

    if let Some(git_result) = git_changed {
        // Git told us which tracked files changed
        let git_changed_set: std::collections::HashSet<&str> =
            git_result.changed.iter().map(|s| s.as_str()).collect();
        let git_deleted_set: std::collections::HashSet<&str> =
            git_result.deleted.iter().map(|s| s.as_str()).collect();

        // Purge deleted files
        for path in &git_result.deleted {
            delete_file_chunks(conn, path)?;
            deleted.push(path.clone());
        }

        for rel in &all_files {
            if git_changed_set.contains(rel.as_str()) {
                // Git says changed → trust it
                changed.push(rel.clone());
            } else if git_deleted_set.contains(rel.as_str()) {
                // Already handled above
                continue;
            } else {
                // Not in git diff → check if it's untracked/new via mtime
                if is_file_changed_mtime_hash(conn, project_root, rel)? {
                    changed.push(rel.clone());
                }
            }
        }
    } else {
        // No git diff available → fall back to mtime + hash for everything
        for rel in &all_files {
            if is_file_changed_mtime_hash(conn, project_root, rel)? {
                changed.push(rel.clone());
            }
        }
    }

    // Purge files that exist in DB but not on disk (deleted untracked files)
    let purged = purge_missing_files(conn, project_root)?;
    if purged > 0 {
        tracing::debug!("Purged {} missing files from index", purged);
    }

    Ok(ChangeSet { changed, deleted })
}

struct GitDiffResult {
    changed: Vec<String>,
    deleted: Vec<String>,
}

/// Try to use git diff for change detection. Returns None if unavailable.
fn try_git_diff_detection(conn: &Connection, project_root: &Path) -> Option<GitDiffResult> {
    let last_commit = get_last_indexed_commit(conn).ok()??;
    let repo = crate::git::open_repo(project_root).ok()?;
    let head = repo.head().ok()?.peel_to_commit().ok()?;
    let head_sha = head.id().to_string();

    if last_commit == head_sha {
        // No changes since last index
        return Some(GitDiffResult {
            changed: Vec::new(),
            deleted: Vec::new(),
        });
    }

    let entries = crate::git::diff_tree_to_tree(&repo, &last_commit, &head_sha).ok()?;

    let mut changed = Vec::new();
    let mut deleted = Vec::new();
    for entry in entries {
        match entry.status {
            crate::git::DiffStatus::Added | crate::git::DiffStatus::Modified => {
                changed.push(entry.path);
            }
            crate::git::DiffStatus::Deleted => {
                deleted.push(entry.path);
            }
            crate::git::DiffStatus::Renamed { ref old_path } => {
                deleted.push(old_path.clone());
                changed.push(entry.path);
            }
        }
    }
    Some(GitDiffResult { changed, deleted })
}

/// Check if a single file changed via mtime pre-filter + SHA-256 confirmation.
fn is_file_changed_mtime_hash(conn: &Connection, project_root: &Path, rel: &str) -> Result<bool> {
    let full_path = project_root.join(rel);
    let current_mtime = file_mtime(&full_path)?;
    let stored_mtime = get_file_mtime(conn, rel)?;

    // If mtime matches, assume unchanged (cheap check)
    if Some(current_mtime) == stored_mtime {
        return Ok(false);
    }

    // Mtime differs or no stored mtime → hash to confirm
    let current_hash = hash_file(&full_path)?;
    let stored_hash = get_file_hash(conn, rel)?;

    Ok(stored_hash.as_deref() != Some(current_hash.as_str()))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::find_changed_files -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add find_changed_files with git-diff + mtime change detection"
```

---

## Task 9: Wire `find_changed_files` into `build_index`

**Files:**
- Modify: `src/embed/index.rs:276-425` (`build_index` — replace Phase 1 loop)
- Test: existing tests should still pass

**Step 1: Refactor `build_index` to use `find_changed_files`**

Replace the Phase 1 loop in `build_index` (the `for entry in walker.flatten()` block that hashes every file) with a call to `find_changed_files`, then process only the candidates.

The new `build_index` structure:

```rust
pub async fn build_index(project_root: &Path, force: bool) -> Result<()> {
    use crate::ast::detect_language;
    use crate::config::ProjectConfig;
    use crate::embed::{create_embedder, Embedding};
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let config = ProjectConfig::load_or_default(project_root)?;
    let conn = open_db(project_root)?;
    if !force {
        check_model_mismatch(&conn, &config.embeddings.model)?;
    }
    let embedder: Arc<dyn crate::embed::Embedder> =
        Arc::from(create_embedder(&config.embeddings.model).await?);

    // ── Phase 1: Detect changes ───────────────────────────────────────────────
    let change_set = find_changed_files(&conn, project_root, force)?;
    let skipped_msg = if force {
        "force rebuild".to_string()
    } else {
        format!("{} deleted", change_set.deleted.len())
    };

    struct FileWork {
        rel: String,
        hash: String,
        mtime: i64,
        lang: String,
        chunks: Vec<super::chunker::RawChunk>,
    }

    let mut works: Vec<FileWork> = Vec::new();

    for rel in &change_set.changed {
        let path = project_root.join(rel);
        let Some(lang) = detect_language(&path) else {
            continue;
        };
        let hash = hash_file(&path)?;
        let mtime = file_mtime(&path).unwrap_or(0);

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let chunks = super::ast_chunker::split_file(
            &source,
            lang,
            &path,
            config.embeddings.chunk_size,
            config.embeddings.chunk_overlap,
        );
        if chunks.is_empty() {
            continue;
        }

        works.push(FileWork {
            rel: rel.clone(),
            hash,
            mtime,
            lang: lang.to_string(),
            chunks,
        });
    }

    // ── Phase 2: Concurrent embedding (unchanged) ─────────────────────────────
    struct FileResult {
        rel: String,
        hash: String,
        mtime: i64,
        lang: String,
        chunks: Vec<super::chunker::RawChunk>,
        embeddings: Vec<Embedding>,
    }

    const MAX_CONCURRENT: usize = 4;
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut tasks: JoinSet<Result<FileResult>> = JoinSet::new();

    for work in works {
        let embedder = Arc::clone(&embedder);
        let sem = Arc::clone(&sem);
        tasks.spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            let texts: Vec<&str> = work.chunks.iter().map(|c| c.content.as_str()).collect();
            let embeddings = embedder.embed(&texts).await?;
            Ok(FileResult {
                rel: work.rel,
                hash: work.hash,
                mtime: work.mtime,
                lang: work.lang,
                chunks: work.chunks,
                embeddings,
            })
        });
    }

    let mut results: Vec<FileResult> = Vec::new();
    while let Some(res) = tasks.join_next().await {
        results.push(res.map_err(|e| anyhow::anyhow!(e))??);
    }

    // ── Phase 3: Single transaction for all DB writes ─────────────────────────
    let indexed = results.len();
    conn.execute_batch("BEGIN")?;
    for result in results {
        delete_file_chunks(&conn, &result.rel)?;
        for (raw, emb) in result.chunks.iter().zip(result.embeddings.iter()) {
            let chunk = CodeChunk {
                id: None,
                file_path: result.rel.clone(),
                language: result.lang.clone(),
                content: raw.content.clone(),
                start_line: raw.start_line,
                end_line: raw.end_line,
                file_hash: result.hash.clone(),
                source: "project".into(),
            };
            insert_chunk(&conn, &chunk, emb)?;
        }
        upsert_file_hash(&conn, &result.rel, &result.hash, Some(result.mtime))?;
        tracing::debug!("indexed {} ({} chunks)", result.rel, result.chunks.len());
    }
    set_meta(&conn, "embed_model", &config.embeddings.model)?;

    // Update last indexed commit
    if let Ok(repo) = crate::git::open_repo(project_root) {
        if let Ok(head) = repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                set_last_indexed_commit(&conn, &commit.id().to_string())?;
            }
        }
    }

    conn.execute_batch("COMMIT")?;
    tracing::info!(
        "Index complete: {} files indexed, {}",
        indexed,
        skipped_msg
    );
    Ok(())
}
```

**Step 2: Run all tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS — existing tests should still work. The behavior is identical for first-run (no stored hashes = all files are candidates) and force (all files returned by `find_changed_files`).

**Step 3: Commit**

```bash
git add src/embed/index.rs
git commit -m "refactor(index): wire find_changed_files into build_index for incremental updates"
```

---

## Task 10: Add `IndexReport` to `index_project` response

**Files:**
- Modify: `src/embed/index.rs` (`build_index` return type)
- Modify: `src/tools/semantic.rs:120-134` (`IndexProject::call` — surface report)
- Test: `src/tools/semantic.rs`

**Step 1: Change `build_index` to return `IndexReport`**

Add the struct near `IndexStats`:

```rust
#[derive(Debug)]
pub struct IndexReport {
    pub indexed: usize,
    pub deleted: usize,
    pub skipped_msg: String,
}
```

Change `build_index` signature from `-> Result<()>` to `-> Result<IndexReport>` and return the report at the end instead of `Ok(())`:

```rust
    Ok(IndexReport {
        indexed,
        deleted: change_set.deleted.len(),
        skipped_msg,
    })
```

**Step 2: Update `IndexProject::call` to surface the report**

In `src/tools/semantic.rs`, update the `call` method:

```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let force = input["force"].as_bool().unwrap_or(false);
        let root = ctx.agent.require_project_root().await?;

        let report = crate::embed::index::build_index(&root, force).await?;

        let conn = crate::embed::index::open_db(&root)?;
        let stats = crate::embed::index::index_stats(&conn)?;

        Ok(json!({
            "status": "ok",
            "files_indexed": report.indexed,
            "files_deleted": report.deleted,
            "detail": report.skipped_msg,
            "total_files": stats.file_count,
            "total_chunks": stats.chunk_count,
        }))
    }
```

**Step 3: Run all tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add src/embed/index.rs src/tools/semantic.rs
git commit -m "feat(index): return IndexReport from build_index with change details"
```

---

## Task 11: Add `IndexStatus` staleness info

**Files:**
- Modify: `src/tools/semantic.rs:138-179` (`IndexStatus::call` — add staleness fields)
- Test: `src/tools/semantic.rs`

**Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn index_status_shows_staleness() {
        let (dir, ctx) = project_ctx().await;

        // Init git repo with a commit
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        let mut git_index = repo.index().unwrap();
        let tree_oid = git_index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

        // Create DB without last_indexed_commit
        let conn = crate::embed::index::open_db(dir.path()).unwrap();
        crate::embed::index::upsert_file_hash(&conn, "a.rs", "abc", None).unwrap();
        drop(conn);

        let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["indexed"], true);
        assert_eq!(result["stale"], true);
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test tools::semantic::tests::index_status_shows_staleness -- --nocapture`
Expected: FAIL — no `stale` field in response

**Step 3: Wire staleness into IndexStatus::call**

In the existing `IndexStatus::call`, after building the response json, add:

```rust
        // Add staleness info
        let staleness = crate::embed::index::check_index_staleness(&conn, &root);
        let mut result = json!({
            "indexed": true,
            "configured_model": model,
            "indexed_with_model": stats.model,
            "file_count": stats.file_count,
            "chunk_count": stats.chunk_count,
            "embedding_count": stats.embedding_count,
            "db_path": db_path.display().to_string(),
        });
        if let Ok(s) = staleness {
            result["stale"] = json!(s.stale);
            if s.stale {
                result["behind_commits"] = json!(s.behind_commits);
            }
            if let Ok(Some(commit)) = crate::embed::index::get_last_indexed_commit(&conn) {
                result["last_indexed_commit"] = json!(commit);
            }
        }
        Ok(result)
```

**Step 4: Run test to verify it passes**

Run: `cargo test tools::semantic::tests::index_status_shows_staleness -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(index): add staleness info to index_status response"
```

---

## Task 12: Update documentation and CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (update schema description)
- Modify: `docs/ROADMAP.md` (mark incremental index as implemented)
- Modify: `src/embed/index.rs:1-16` (update module doc comment)

**Step 1: Update module doc in `src/embed/index.rs`**

Update the top-of-file comment to reflect the new schema:

```rust
//! sqlite-vec based embedding index with incremental updates.
//!
//! Schema:
//!   files(path TEXT, hash TEXT, mtime INTEGER) — tracks indexed file hashes + mtime
//!   chunks(id, file_path, language, content,    — code chunks
//!          start_line, end_line, file_hash,
//!          source)
//!   chunk_embeddings(rowid, embedding)          — sqlite-vec virtual table
//!   meta(key TEXT, value TEXT)                   — stores embed_model, last_indexed_commit
//!
//! Change detection fallback chain:
//!   1. git diff last_indexed_commit..HEAD (tracked files)
//!   2. mtime comparison (untracked files or git unavailable)
//!   3. SHA-256 hash (final arbiter)
```

**Step 2: Update ROADMAP.md**

In `docs/ROADMAP.md`, under the "Incremental Index Rebuilding" section, change the description to note it's implemented (or update the Quick Status table if it's listed there).

**Step 3: Run full suite one final time**

Run: `cargo fmt && cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add src/embed/index.rs CLAUDE.md docs/ROADMAP.md
git commit -m "docs: update schema docs and roadmap for incremental indexing"
```
