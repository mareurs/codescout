# Catalog Hygiene: Temp-Write Prevention + Batch Dead-Root Cleanup — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop temp-dir artifacts from polluting the real shared librarian catalog, and add a safe batch dry-run cleanup of the existing dead rows.

**Architecture:** A shared prevention guard, called from `create` and `reindex`, refuses a write when the workspace root is under the OS temp dir AND the catalog is the real/shared one (file-backed, outside temp). A batch mode on `doctor(fix="prune_missing")` (no `root=`) derives dead roots by a "parent-also-gone" rule and reuses the existing `prune_dead_root` primitive, dry-run by default.

**Tech Stack:** Rust, rusqlite (SQLite), the codescout librarian catalog, `tempfile` for tests.

**Design spec:** `docs/superpowers/specs/2026-07-17-catalog-hygiene-prevention-cleanup-design.md`

## Global Constraints

- Pre-task gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` must all pass before each commit. (Copied verbatim from CLAUDE.md.)
- Correctable, user-facing failures use `crate::tools::RecoverableError` (routes to `isError:false` so sibling parallel tool calls survive); programmer errors use `anyhow::bail!`.
- Structural Rust edits (new fns/items) via `edit_code`; intra-body text edits via `edit_file`.
- Branch: `experiments` (`master` is protected). Commit style: Conventional Commits, and every commit message ends with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- Librarian unit tests default to `Catalog::open_in_memory()`; the persistence probe and its consumers use a file-backed catalog via `tempfile`.
- **Out of scope (do NOT implement):** `missing_since` timestamping, hide-from-find grace, time-based auto-prune, `workspace(status)`/SessionStart surfacing. Those are a deferred follow-up spec.

---

## File Structure

- **Modify** `src/librarian/catalog/mod.rs` — add `catalog_db_path(conn) -> Option<PathBuf>` (Task 1).
- **Create** `src/librarian/tools/temp_write_guard.rs` — `should_refuse` (pure), `guard_temp_workspace_write` (wrapper) (Task 2).
- **Modify** `src/librarian/tools/mod.rs` — register the new module (Task 2).
- **Modify** `src/librarian/tools/create.rs` — call the guard in `call` (Task 3).
- **Modify** `src/librarian/tools/reindex.rs` — call the guard in `call` (Task 4).
- **Modify** `src/librarian/tools/doctor.rs` — `derive_dead_roots` + `count_dead_root` (Task 5); batch dry-run/apply in `run_fix` (Task 6).
- **Modify** `src/librarian/tools/librarian.rs` — extend the `root` schema description for batch mode (Task 6).

---

## Task 1: `catalog_db_path` — detect the catalog's backing file

**Files:**
- Modify: `src/librarian/catalog/mod.rs` (add a free fn near the `Catalog` impl + a test)

**Interfaces:**
- Produces: `pub(crate) fn catalog_db_path(conn: &rusqlite::Connection) -> Option<std::path::PathBuf>` — `None` for an in-memory connection, `Some(path)` for a file-backed one.

- [ ] **Step 1: Write the failing tests** (add to the existing `#[cfg(test)] mod tests` in `src/librarian/catalog/mod.rs`)

```rust
#[test]
fn catalog_db_path_none_for_in_memory() {
    let cat = Catalog::open_in_memory().unwrap();
    assert!(catalog_db_path(&cat.conn).is_none());
}

#[test]
fn catalog_db_path_some_for_file_backed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.db");
    let cat = Catalog::open(&path).unwrap();
    let got = catalog_db_path(&cat.conn).expect("file-backed catalog must report a path");
    // SQLite may hand back a canonicalized form; compare on the file name.
    assert_eq!(got.file_name(), path.file_name());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib catalog_db_path`
Expected: FAIL — `cannot find function 'catalog_db_path'`.

- [ ] **Step 3: Implement `catalog_db_path`** (add near the `impl Catalog` block, e.g. after `open_with_workspace`)

```rust
/// The catalog's backing DB file, or `None` for an in-memory connection.
/// `PRAGMA database_list` yields rows `(seq, name, file)`; the `main` database's
/// `file` column is `""` for an in-memory/temp connection and an absolute path
/// for a file-backed one.
pub(crate) fn catalog_db_path(conn: &rusqlite::Connection) -> Option<std::path::PathBuf> {
    let file: String = conn
        .query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        .ok()?;
    if file.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(file))
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib catalog_db_path`
Expected: PASS (2 tests).

- [ ] **Step 5: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib catalog_db_path
git add src/librarian/catalog/mod.rs
git commit -m "feat(catalog): add catalog_db_path to detect the backing DB file

Returns None for in-memory connections, Some(path) for file-backed ones,
via PRAGMA database_list. Foundation for the temp-write prevention guard.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: prevention guard module

**Files:**
- Create: `src/librarian/tools/temp_write_guard.rs`
- Modify: `src/librarian/tools/mod.rs` (register the module)

**Interfaces:**
- Consumes: `crate::librarian::catalog::catalog_db_path` (Task 1).
- Produces:
  - `fn should_refuse(root: &Path, catalog_db: Option<&Path>, temp_dir: &Path, opted_in: bool) -> bool` — pure decision.
  - `pub(crate) fn guard_temp_workspace_write(root: &std::path::Path, conn: &rusqlite::Connection) -> anyhow::Result<()>` — the wrapper `create`/`reindex` call.

- [ ] **Step 1: Write the module with failing tests** (create `src/librarian/tools/temp_write_guard.rs`)

```rust
//! Prevention guard: refuse artifact writes that would land a temp-dir-rooted
//! artifact in the real/shared (file-backed, outside-temp) catalog — the vector
//! that polluted the global catalog with /tmp probe rows.
//! See docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md.

use std::path::Path;

const ALLOW_ENV: &str = "CODESCOUT_ALLOW_TEMP_WORKSPACE";

/// Pure decision: refuse iff the workspace `root` is under `temp_dir`, the
/// catalog is the real one (file-backed AND its file is outside `temp_dir`), and
/// the caller has not opted in. All inputs are pre-resolved so this is testable
/// with fabricated absolute paths (no filesystem access).
fn should_refuse(root: &Path, catalog_db: Option<&Path>, temp_dir: &Path, opted_in: bool) -> bool {
    if opted_in {
        return false;
    }
    let under_temp = |p: &Path| p.starts_with(temp_dir);
    let catalog_is_real = catalog_db.is_some_and(|c| !under_temp(c));
    under_temp(root) && catalog_is_real
}

/// Refuse a write whose workspace `root` is under the OS temp dir when the
/// catalog is the real/shared one. Canonicalizes the real inputs, then defers to
/// [`should_refuse`].
pub(crate) fn guard_temp_workspace_write(
    root: &Path,
    conn: &rusqlite::Connection,
) -> anyhow::Result<()> {
    let opted_in = std::env::var_os(ALLOW_ENV).is_some();
    let temp = std::fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    let root_c = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let catalog_db = crate::librarian::catalog::catalog_db_path(conn)
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p));
    if should_refuse(&root_c, catalog_db.as_deref(), &temp, opted_in) {
        return Err(crate::tools::RecoverableError::with_hint(
            format!(
                "refusing to write an artifact rooted under the system temp dir ({}) into the \
                 shared persistent catalog — this is how probe/test runs pollute the catalog",
                temp.display()
            ),
            format!(
                "Use an isolated catalog (under the temp dir, or in-memory) for tests, or set \
                 {ALLOW_ENV}=1 if this scratch workspace is intentional."
            ),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use std::path::Path;

    #[test]
    fn should_refuse_temp_root_into_real_catalog() {
        assert!(should_refuse(
            Path::new("/tmp/scratch"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_when_catalog_is_also_under_temp() {
        // The server/integration-test shape: file-backed catalog under the
        // test's own TempDir. Must be allowed.
        assert!(!should_refuse(
            Path::new("/tmp/ws"),
            Some(Path::new("/tmp/testrun/librarian.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_in_memory_catalog() {
        assert!(!should_refuse(Path::new("/tmp/ws"), None, Path::new("/tmp"), false));
    }

    #[test]
    fn should_allow_non_temp_root() {
        assert!(!should_refuse(
            Path::new("/home/u/proj"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_when_opted_in() {
        assert!(!should_refuse(
            Path::new("/tmp/ws"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            true,
        ));
    }

    #[test]
    fn wrapper_allows_temp_workspace_with_temp_catalog() {
        // A file-backed catalog under temp (the test shape) must be allowed by
        // the real wrapper, proving it wires the probes correctly.
        let dir = tempfile::tempdir().unwrap(); // under the OS temp dir
        let cat = Catalog::open(&dir.path().join("librarian.db")).unwrap();
        let ws = dir.path().join("workspace");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(guard_temp_workspace_write(&ws, &cat.conn).is_ok());
    }

    #[test]
    fn wrapper_allows_in_memory_catalog_with_temp_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        assert!(guard_temp_workspace_write(dir.path(), &cat.conn).is_ok());
    }
}
```

- [ ] **Step 2: Register the module** in `src/librarian/tools/mod.rs`

Add alongside the other `mod <tool>;` declarations:

```rust
pub(crate) mod temp_write_guard;
```

- [ ] **Step 3: Run to verify** (module compiles, tests pass — there is no failing-first step here because the module is new and self-contained; the pure-fn tests exercise every branch)

Run: `cargo test --lib temp_write_guard`
Expected: PASS (7 tests).

- [ ] **Step 4: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib temp_write_guard
git add src/librarian/tools/temp_write_guard.rs src/librarian/tools/mod.rs
git commit -m "feat(librarian): add temp-workspace write guard

should_refuse() is a pure decision over (root, catalog_db, temp_dir,
opted_in); guard_temp_workspace_write() canonicalizes real inputs and
defers to it. Refuses only temp-rooted writes into a real (file-backed,
outside-temp) catalog, so isolated/in-memory test catalogs are unaffected.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: wire the guard into `create`

**Files:**
- Modify: `src/librarian/tools/create.rs` (`call` + a test)

**Interfaces:**
- Consumes: `super::temp_write_guard::guard_temp_workspace_write` (Task 2).

- [ ] **Step 1: Write the failing test** (add to `create.rs`'s `#[cfg(test)] mod tests`)

```rust
#[tokio::test]
async fn create_refuses_temp_workspace_into_real_catalog() {
    // The real pollution shape: catalog OUTSIDE the OS temp dir, workspace UNDER
    // it. `TempDir::new_in(current_dir())` puts the catalog under the repo cwd
    // (outside /tmp) and auto-cleans on drop — the only way to construct an
    // outside-temp catalog in a test without leaking files. (Assumes the repo
    // checkout is not itself under the OS temp dir, which holds here.)
    let outside = tempfile::TempDir::new_in(std::env::current_dir().unwrap()).unwrap();
    let cat = Catalog::open(&outside.path().join("catalog.db")).unwrap();
    let ws = TempDir::new().unwrap(); // under the OS temp dir
    let ctx = TestToolContextBuilder::new(cat)
        .with_root(Root { name: "r".into(), path: ws.path().to_path_buf() })
        .build();

    let err = call(&ctx, json!({
        "repo": "r", "rel_path": "docs/specs/x.md",
        "kind": "spec", "title": "X", "body": "hi",
    }))
    .await
    .expect_err("temp workspace + real (outside-temp) catalog must be refused");
    assert!(err.to_string().contains("temp dir"), "unexpected error: {err}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib create_refuses_temp_workspace_into_real_catalog`
Expected: FAIL — without the guard, `create` succeeds and `expect_err` panics.

- [ ] **Step 3: Wire the guard into `create::call`** — insert immediately after `base_dir` is resolved (right before `validate_rel_path(&a.rel_path)?;`), using `edit_file`:

Insert:
```rust
    // Prevention: refuse writing a temp-dir-rooted artifact into the real shared
    // catalog. See docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md.
    super::temp_write_guard::guard_temp_workspace_write(&base_dir, &ctx.catalog.lock().conn)?;

```
so it reads:
```rust
    };  // <- end of the `let base_dir = match a.repo.as_deref() { ... };`

    super::temp_write_guard::guard_temp_workspace_write(&base_dir, &ctx.catalog.lock().conn)?;

    validate_rel_path(&a.rel_path)?;
```

- [ ] **Step 4: Run to verify pass + no regressions**

Run: `cargo test --lib librarian::tools::create`
Expected: PASS — the new refuse test passes, AND every existing create test (each uses an in-memory catalog, so the guard is a no-op) still passes.

- [ ] **Step 5: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib librarian::tools::create
git add src/librarian/tools/create.rs
git commit -m "feat(librarian): guard artifact create against temp-catalog pollution

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
---

## Task 4: wire the guard into `reindex`

**Files:**
- Modify: `src/librarian/tools/reindex.rs` (`call` + a test)

**Interfaces:**
- Consumes: `super::temp_write_guard::guard_temp_workspace_write` (Task 2).

- [ ] **Step 1: Write the failing test** (add to `reindex.rs`'s `#[cfg(test)] mod tests`)

```rust
#[tokio::test]
async fn reindex_refuses_temp_root_into_real_catalog() {
    // Catalog OUTSIDE the OS temp dir; workspace root UNDER it. With no current
    // project, reindex defaults to scope=All and walks the workspace roots — so
    // the guard fires on the temp root before any file walk. (No rules / fixtures
    // needed: the refusal happens before classification.)
    let outside = tempfile::TempDir::new_in(std::env::current_dir().unwrap()).unwrap();
    let cat = Catalog::open(&outside.path().join("catalog.db")).unwrap();
    let ws = TempDir::new().unwrap(); // under the OS temp dir
    let ctx = TestToolContextBuilder::new(cat)
        .with_root(Root { name: "r".into(), path: ws.path().to_path_buf() })
        .build();

    let err = call(&ctx, json!({}))
        .await
        .expect_err("reindexing a temp root into a real (outside-temp) catalog must be refused");
    assert!(err.to_string().contains("temp dir"), "unexpected error: {err}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib reindex_refuses_temp_root_into_real_catalog`
Expected: FAIL — without the guard, `reindex` walks the temp root and returns Ok, so `expect_err` panics.

- [ ] **Step 3: Wire the guard into `reindex::call`** — insert right after the `let targets: Vec<std::path::PathBuf> = match effective_scope { ... };` block and before the `// NOTE: previously, force=true ...` comment, using `edit_file`:

```rust
    // Prevention: refuse indexing a temp-dir root into the real shared catalog.
    {
        let cat = ctx.catalog.lock();
        for target in &targets {
            super::temp_write_guard::guard_temp_workspace_write(target, &cat.conn)?;
        }
    }

```

- [ ] **Step 4: Run the reindex suite**

Run: `cargo test --lib librarian::tools::reindex`
Expected: PASS — the new refuse test passes, AND all existing reindex tests (in-memory catalogs → guard is a no-op) still pass.

- [ ] **Step 5: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib librarian::tools::reindex
git add src/librarian/tools/reindex.rs
git commit -m "feat(librarian): guard reindex against temp-catalog pollution

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
---

## Task 5: `derive_dead_roots` + `count_dead_root`

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (two helper fns + tests)

**Interfaces:**
- Produces:
  - `fn derive_dead_roots(conn: &rusqlite::Connection) -> anyhow::Result<Vec<std::path::PathBuf>>`
  - `fn count_dead_root(conn: &rusqlite::Connection, root: &std::path::Path) -> anyhow::Result<(usize, usize)>` — read-only `(artifact_rows, commit_rows)` under a root.

- [ ] **Step 1: Write the failing tests** (add to `doctor.rs`'s `#[cfg(test)] mod tests`; reuse the existing `seed_artifact` helper)

```rust
#[test]
fn derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files() {
    let cat = Catalog::open_in_memory().unwrap();
    let live = tempfile::tempdir().unwrap(); // exists on disk

    // (a) whole subtree gone: parent dir does not exist -> included.
    seed_artifact(&cat, "a1", "/nonexistent-root/repo/docs/x.md");
    seed_artifact(&cat, "a2", "/nonexistent-root/repo/docs/y.md");
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
        vec![std::path::PathBuf::from("/nonexistent-root/repo")],
        "only the gone subtree's highest-nonexistent-ancestor is a dead root"
    );
}

#[test]
fn count_dead_root_counts_rows_under_root() {
    let cat = Catalog::open_in_memory().unwrap();
    seed_artifact(&cat, "a1", "/nonexistent-root/repo/docs/x.md");
    seed_artifact(&cat, "a2", "/nonexistent-root/repo/y.md");
    seed_artifact(&cat, "z1", "/nonexistent-root/other/z.md");
    let (arts, _commits) =
        count_dead_root(&cat.conn, std::path::Path::new("/nonexistent-root/repo")).unwrap();
    assert_eq!(arts, 2);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files count_dead_root_counts_rows_under_root`
Expected: FAIL — functions not found. (Run each name separately if the harness rejects two positional filters.)

- [ ] **Step 3: Implement the helpers** (add near `prune_dead_root` in `doctor.rs`)

```rust
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
    let arts: usize = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    let commits: usize = conn.query_row(
        "SELECT COUNT(*) FROM commits WHERE git_root = ?1 OR git_root LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    Ok((arts, commits))
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files` then `cargo test --lib count_dead_root_counts_rows_under_root`
Expected: PASS.

- [ ] **Step 5: Gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib librarian::tools::doctor
git add src/librarian/tools/doctor.rs
git commit -m "feat(doctor): derive_dead_roots + count_dead_root helpers

parent-also-gone rule identifies whole-subtree dead roots and excludes
single missing files under live repos (reindex's job). Foundation for
the batch prune mode.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: batch dry-run / apply mode on `doctor(fix="prune_missing")`

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (`call` extracts `confirm`; `run_fix` gains batch branch)
- Modify: `src/librarian/tools/librarian.rs` (schema description for `root` / batch mode)

**Interfaces:**
- Consumes: `derive_dead_roots`, `count_dead_root`, `prune_dead_root`, `worktree::covering_conn` (existing), `crate::util::fs::RepoPath` (existing).

- [ ] **Step 1: Write the failing tests** (add to `doctor.rs`'s test module)

```rust
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

    let v = call(&ctx, json!({ "fix": "prune_missing", "confirm": true })).await.unwrap();
    assert_eq!(v["mode"], "applied");
    assert_eq!(v["totals"]["artifact_rows"].as_u64().unwrap(), 1);
    assert!(artifact::get(&ctx.catalog.lock(), "dead").unwrap().is_none(), "dead row pruned");
    assert!(artifact::get(&ctx.catalog.lock(), "live").unwrap().is_some(), "live row kept");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib prune_missing_batch_dry_run_lists_dead_roots_without_deleting` then `cargo test --lib prune_missing_batch_confirm_prunes_dead_roots_only`
Expected: FAIL — the single-root `validate_prune_request` currently errors on a missing `root=` (`requires root=...`), so `mode` is absent.

- [ ] **Step 3a: Thread `confirm` into `run_fix`** — in `doctor.rs::call`, change the fix dispatch line:

From:
```rust
    if let Some(fix) = args.get("fix").and_then(Value::as_str) {
        return run_fix(ctx, fix, args.get("root").and_then(Value::as_str)).await;
    }
```
To:
```rust
    if let Some(fix) = args.get("fix").and_then(Value::as_str) {
        let confirm = args.get("confirm").and_then(Value::as_bool).unwrap_or(false);
        return run_fix(ctx, fix, args.get("root").and_then(Value::as_str), confirm).await;
    }
```

- [ ] **Step 3b: Add the batch branch to `run_fix`** — change its signature and the `"prune_missing"` arm (use `edit_code` to replace the `run_fix` fn):

```rust
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib librarian::tools::doctor`
Expected: PASS (existing single-root tests + the two new batch tests).

- [ ] **Step 5: Update the `root` schema description** in `src/librarian/tools/librarian.rs` (the `"root"` property description under the librarian tool schema) with `edit_file`:

From (the doctor part of the description):
```
doctor fix=prune_missing: absolute path of the dead/renamed repo root to prune. Refused if the path still exists on disk.
```
To:
```
doctor fix=prune_missing: absolute path of the dead/renamed repo root to prune (refused if the path still exists on disk). OMIT root to run BATCH mode: dry-run lists every dead root (whole-subtree-gone) with row counts; pass confirm=true to prune them all.
```

Also add a `"confirm"` property to the librarian tool schema `properties` (boolean, described: "doctor fix=prune_missing batch mode: pass true to apply the prune; omitted/false = dry-run").

- [ ] **Step 6: Full gate + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs src/librarian/tools/librarian.rs
git commit -m "feat(doctor): batch dead-root prune_missing (dry-run default, confirm to apply)

fix=prune_missing without root= now runs batch mode over derive_dead_roots:
dry-run reports dead roots + counts; confirm=true prunes each via the
existing guarded prune_dead_root (worktree-registered roots skipped).
Enables the one-time cleanup of the 179 dead + 28 /tmp rows.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Post-implementation (NOT part of the coding tasks)

- **Gated real-catalog cleanup:** after all tasks land + full `cargo rb` + `/mcp` reconnect, run `librarian(action="doctor", fix="prune_missing")` (dry-run) against the REAL catalog, present the dead roots + counts to the user, and apply with `confirm=true` only on explicit approval.
- **Bug bookkeeping:** flip `docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md` to `fixed` (prevention shipped; cleanup executed). Update `docs/issues/archive/2026-07-17-catalog-dead-rows-no-gc.md` — cleanup half done; note the ongoing-GC-lifecycle remainder stays open, deferred to a follow-up spec.
