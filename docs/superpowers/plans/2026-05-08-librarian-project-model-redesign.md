# Librarian Project-Model Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace librarian's string-name "root" model with absolute-path projects driven by the host's active project, fixing cross-project tracker leakage.

**Architecture:** Schema v6 migration backfills `abs_path`/`git_root` columns from the existing `[[roots]]` lookup, drops `(repo, rel_path)`. `LibrarianAdapter` rewires to read codescout's active project per call. Scope filters become path-prefix predicates with a 4-tier ladder (project / repo / umbrella / all). Two-release deprecation window for `[[roots]]` parsing.

**Tech Stack:** Rust 2024, rusqlite (SQLite ≥ 3.35 required for `ALTER DROP COLUMN`), tokio, anyhow, serde_json, globset.

**Spec:** `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md`. Reference for non-goals, risks, and the full rationale.

---

## Pre-flight

Read once before starting:

- `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md` — the design spec
- `crates/librarian-mcp/src/catalog/schema.sql` — current schema (v1-v3 baseline)
- `crates/librarian-mcp/src/catalog/mod.rs` — `run_migrations`, `Catalog::open`, `column_exists` helper
- `crates/librarian-mcp/src/tools/scope.rs` — current scope resolver
- `crates/librarian-mcp/src/current_project.rs` — current `CurrentProject` + resolver
- `crates/librarian-mcp/src/lib.rs:27-109` — `build_tool_context()` (boot-time wiring)
- `src/librarian.rs` — `LibrarianAdapter` (the host-side wrapper)
- `src/agent/mod.rs:54-149` — `Agent`, `AgentInner`, `ActiveProject`
- `src/tools/onboarding.rs:21` — `ONBOARDING_VERSION` constant
- `CLAUDE.md` "Prompt Surface Consistency" — three-surface rule + `prompt_surfaces_reference_only_real_tools` test

Verify environment before starting:

```bash
cargo --version           # rustc/cargo present
sqlite3 --version         # need >= 3.35.0 for ALTER DROP COLUMN
cargo fmt -- --check      # baseline clean
cargo clippy --all-targets -- -D warnings  # baseline clean
cargo test                # baseline passing
```

If `sqlite3 --version` is < 3.35, install a newer SQLite. The `rusqlite` bundled feature ships its own; check `Cargo.toml` for the feature flags. (Today's flags include `bundled` per a recent commit.)

---

## File Structure

**Crates / files modified:**

| Path | Role | Tasks |
|---|---|---|
| `crates/librarian-mcp/src/catalog/schema.sql` | Bootstrap schema for fresh DBs | 1, 6 |
| `crates/librarian-mcp/src/catalog/mod.rs` | `run_migrations`, `Catalog::open`, `column_exists` | 1, 2, 6 |
| `crates/librarian-mcp/src/catalog/migrate_v6.rs` (NEW) | Migration logic + tests | 1, 2, 6 |
| `crates/librarian-mcp/src/catalog/artifact.rs` | `ArtifactRow` struct + insert/upsert SQL | 4, 6 |
| `crates/librarian-mcp/src/catalog/find.rs` | Find SQL builder | 4, 6 |
| `crates/librarian-mcp/src/catalog/commits.rs` | Commits SQL | 6 |
| `crates/librarian-mcp/src/catalog/augmentation.rs` | `ArtifactWithAugmentation.repo` field | 4 |
| `crates/librarian-mcp/src/current_project.rs` | `CurrentProject` struct + resolver | 4 |
| `crates/librarian-mcp/src/tools/scope.rs` | `apply_scope`, scope clauses | 3 |
| `crates/librarian-mcp/src/tools/find.rs` | Tool handler — uses scope | 3, 4 |
| `crates/librarian-mcp/src/tools/context.rs` | Tool handler — uses scope | 3, 4 |
| `crates/librarian-mcp/src/tools/workspace_state_at.rs` | Tool handler — uses scope | 3, 4 |
| `crates/librarian-mcp/src/tools/reindex.rs` | Tool handler — uses scope | 3, 4 |
| `crates/librarian-mcp/src/tools/refresh_stale.rs` | Tool handler — uses scope | 3 |
| `crates/librarian-mcp/src/tools/artifact.rs` | Tool description text | 8 |
| `crates/librarian-mcp/src/tools/librarian.rs` | Tool description text | 8 |
| `crates/librarian-mcp/src/workspace.rs` | `WorkspaceConfig` struct + load + deprecation warning | 7 |
| `crates/librarian-mcp/src/indexer.rs` | `index_repo_sync` signature → take `abs_path` instead of `repo_name + rel_path` | 4 |
| `crates/librarian-mcp/src/lib.rs` | `build_tool_context` standalone fallback path | 4, 5 |
| `src/librarian.rs` | `LibrarianAdapter::call` rewrite + tests | 5 |
| `src/prompts/server_instructions.md` | Codescout-side scope ladder docs | 8 |
| `src/prompts/onboarding_prompt.md` | Onboarding text | 8 |
| `src/prompts/builders.rs` | `build_system_prompt_draft` | 8 |
| `src/tools/onboarding.rs:21` | `ONBOARDING_VERSION` bump | 8 |
| `crates/librarian-mcp/src/prompts/server_instructions.md` | Librarian-side scope ladder docs | 8 |
| `crates/librarian-mcp/src/prompts/companion_hint.md` | Companion hint scope ladder docs | 8 |

**New file:** `crates/librarian-mcp/src/catalog/migrate_v6.rs` keeps the migration code separate from `catalog/mod.rs` (which is already growing). Module declared in `mod.rs` next to `artifact`/`commits`/etc.

---

## Task 1 — Schema v6 column scaffolding

**Goal:** Add `abs_path` and `git_root` columns alongside `repo`/`rel_path`. No data migration yet. Schema version unchanged at 5.

**Files:**
- Create: `crates/librarian-mcp/src/catalog/migrate_v6.rs`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs` (add module declaration; extend `run_migrations` to call `migrate_v6::add_columns`)
- Modify: `crates/librarian-mcp/src/catalog/schema.sql` (add the columns to the bootstrap schema for fresh DBs)

- [ ] **Step 1: Write failing test for column presence.** Add to `crates/librarian-mcp/src/catalog/mod.rs` `tests` module:

```rust
#[test]
fn migration_adds_abs_path_and_git_root_columns() {
    let cat = Catalog::open_in_memory().unwrap();
    assert!(column_exists(&cat.conn, "artifact", "abs_path").unwrap());
    assert!(column_exists(&cat.conn, "commits", "git_root").unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p librarian-mcp migration_adds_abs_path_and_git_root_columns
```

Expected: FAIL with `assertion failed: column_exists(..., "abs_path")`.

- [ ] **Step 3: Create the migration module** at `crates/librarian-mcp/src/catalog/migrate_v6.rs`:

```rust
//! Schema v6 migration: replace (repo, rel_path) with abs_path; rename
//! commits.repo → commits.git_root. See
//! docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md.

use anyhow::Result;
use rusqlite::Connection;

use crate::catalog::column_exists;

/// Step 1 of the migration: add new columns alongside legacy ones.
/// Idempotent — checks column presence first.
pub(super) fn add_columns(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "artifact", "abs_path")? {
        conn.execute("ALTER TABLE artifact ADD COLUMN abs_path TEXT", [])?;
    }
    if !column_exists(conn, "commits", "git_root")? {
        conn.execute("ALTER TABLE commits ADD COLUMN git_root TEXT", [])?;
    }
    Ok(())
}
```

- [ ] **Step 4: Wire into `run_migrations`.** In `crates/librarian-mcp/src/catalog/mod.rs`, add module declaration near the others:

```rust
mod migrate_v6;
```

Then extend `run_migrations` (after the existing v5 block):

```rust
// v6 migration step 1: add new columns alongside legacy ones.
// Backfill + drop legacy happens in later phases (Tasks 2 + 6).
migrate_v6::add_columns(conn)?;
```

- [ ] **Step 5: Update `SCHEMA_SQL`** at `crates/librarian-mcp/src/catalog/schema.sql`. Inside the `artifact` `CREATE TABLE`, add the column:

```sql
abs_path      TEXT,
```

(near the other TEXT columns; `NULL`-able for now). And in the `commits` `CREATE TABLE`:

```sql
git_root      TEXT,
```

- [ ] **Step 6: Run the test** — should now pass:

```
cargo test -p librarian-mcp migration_adds_abs_path_and_git_root_columns
```

- [ ] **Step 7: Run full test suite** to confirm no regression:

```
cargo test -p librarian-mcp
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/catalog/migrate_v6.rs \
        crates/librarian-mcp/src/catalog/mod.rs \
        crates/librarian-mcp/src/catalog/schema.sql
git commit -m "schema(v6): add abs_path/git_root columns + migration scaffolding"
```

---

## Task 2 — Backfill migration

**Goal:** Translate every existing `(repo, rel_path)` row into a non-NULL `abs_path` using `workspace.toml`'s `[[roots]]` as the lookup table. Same for `commits.repo` → `commits.git_root`. Backup the DB first. Fail loudly on orphans (rows whose `repo` doesn't match any declared root).

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/migrate_v6.rs` (add `backfill` function + tests)
- Modify: `crates/librarian-mcp/src/catalog/mod.rs` (call `backfill` after `add_columns`; pass workspace config)
- Modify: `crates/librarian-mcp/src/lib.rs` (`build_tool_context` already loads workspace before opening catalog → reorder to pass it down)

- [ ] **Step 1: Test — backfill succeeds with mapped roots.** Append to `crates/librarian-mcp/src/catalog/migrate_v6.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Root, WorkspaceConfig};
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn new_db_with_legacy_row(repo: &str, rel_path: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // v3 baseline schema (only the columns we touch).
        conn.execute_batch(r#"
            CREATE TABLE artifact (
                id TEXT PRIMARY KEY, repo TEXT NOT NULL, rel_path TEXT NOT NULL,
                kind TEXT NOT NULL, status TEXT NOT NULL, title TEXT,
                owners TEXT NOT NULL DEFAULT '[]', tags TEXT NOT NULL DEFAULT '[]',
                topic TEXT, time_scope TEXT, source TEXT,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                file_mtime INTEGER NOT NULL, file_sha256 TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE commits (
                hash TEXT PRIMARY KEY, repo TEXT NOT NULL,
                authored_at INTEGER, subject TEXT, topo_order INTEGER
            );
        "#).unwrap();
        conn.execute(
            "INSERT INTO artifact(id, repo, rel_path, kind, status, title,
                                  created_at, updated_at, file_mtime, file_sha256)
             VALUES ('a1', ?1, ?2, 'tracker', 'active', 't', 0, 0, 0, 'sha')",
            rusqlite::params![repo, rel_path],
        ).unwrap();
        // Apply v6 step 1 (add columns).
        add_columns(&conn).unwrap();
        conn
    }

    fn ws_with(root_name: &str, root_path: &str) -> WorkspaceConfig {
        WorkspaceConfig {
            roots: vec![Root { name: root_name.into(), path: PathBuf::from(root_path) }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }
    }

    #[test]
    fn migration_v6_translates_repo_to_abs_path() {
        let conn = new_db_with_legacy_row("code-explorer", "docs/trackers/foo.md");
        let ws = ws_with("code-explorer", "/home/u/work/code-explorer");
        backfill(&conn, &ws, false).unwrap();
        let abs: String = conn.query_row(
            "SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(abs, "/home/u/work/code-explorer/docs/trackers/foo.md");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p librarian-mcp migration_v6_translates_repo_to_abs_path
```

Expected: FAIL — `backfill` not defined.

- [ ] **Step 3: Implement `backfill`.** Add to `migrate_v6.rs`:

```rust
use crate::workspace::WorkspaceConfig;
use std::collections::HashMap;
use std::path::PathBuf;

/// Step 2 of the migration: backfill `abs_path` and `git_root` for every
/// legacy row, using the workspace.toml `[[roots]]` lookup. Idempotent —
/// rows that already have a non-NULL `abs_path` are skipped.
///
/// Errors with the orphan list when a row's `repo` value isn't declared in
/// the workspace, unless `drop_orphans` is true (the
/// `LIBRARIAN_MIGRATE_DROP_ORPHANS=1` opt-in).
pub(super) fn backfill(
    conn: &Connection,
    ws: &WorkspaceConfig,
    drop_orphans: bool,
) -> Result<()> {
    let lookup: HashMap<&str, &PathBuf> =
        ws.roots.iter().map(|r| (r.name.as_str(), &r.path)).collect();

    // Detect orphans (rows whose repo isn't in the lookup) BEFORE writing.
    let orphan_ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id, repo FROM artifact WHERE abs_path IS NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.filter_map(|row| {
            let (id, repo) = row.ok()?;
            (!lookup.contains_key(repo.as_str())).then_some(id)
        }).collect()
    };

    if !orphan_ids.is_empty() {
        if drop_orphans {
            for id in &orphan_ids {
                conn.execute("DELETE FROM artifact WHERE id = ?1", [id])?;
            }
        } else {
            let sample: Vec<&str> =
                orphan_ids.iter().take(5).map(String::as_str).collect();
            anyhow::bail!(
                "{} artifact(s) reference unknown root: {}{}. Either restore the \
                 root in workspace.toml or set LIBRARIAN_MIGRATE_DROP_ORPHANS=1 \
                 to discard them.",
                orphan_ids.len(),
                sample.join(", "),
                if orphan_ids.len() > 5 { ", …" } else { "" },
            );
        }
    }

    // Backfill artifact.abs_path.
    let mut stmt = conn.prepare(
        "SELECT id, repo, rel_path FROM artifact WHERE abs_path IS NULL",
    )?;
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?.collect::<Result<_, _>>()?;
    for (id, repo, rel_path) in rows {
        let root = lookup.get(repo.as_str()).expect("orphans rejected above");
        let abs = root.join(&rel_path);
        conn.execute(
            "UPDATE artifact SET abs_path = ?1 WHERE id = ?2",
            rusqlite::params![abs.to_string_lossy(), id],
        )?;
    }

    // Backfill commits.git_root.
    let mut stmt = conn.prepare(
        "SELECT hash, repo FROM commits WHERE git_root IS NULL",
    )?;
    let rows: Vec<(String, String)> = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?.collect::<Result<_, _>>()?;
    for (hash, repo) in rows {
        if let Some(root) = lookup.get(repo.as_str()) {
            conn.execute(
                "UPDATE commits SET git_root = ?1 WHERE hash = ?2",
                rusqlite::params![root.to_string_lossy(), hash],
            )?;
        }
    }

    Ok(())
}
```

- [ ] **Step 4: Run the test** — passes.

- [ ] **Step 5: Test — orphan rejection.** Add to the `tests` module:

```rust
#[test]
fn migration_v6_fails_loudly_on_orphans() {
    let conn = new_db_with_legacy_row("ghost", "x.md");
    let ws = ws_with("alive", "/abs/alive");
    let err = backfill(&conn, &ws, false).unwrap_err();
    assert!(err.to_string().contains("ghost") || err.to_string().contains("a1"));
    // Row was NOT deleted.
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE id = 'a1'", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}
```

Run: should pass.

- [ ] **Step 6: Test — drop orphans on opt-in.**

```rust
#[test]
fn migration_v6_drops_orphans_when_opt_in() {
    let conn = new_db_with_legacy_row("ghost", "x.md");
    let ws = ws_with("alive", "/abs/alive");
    backfill(&conn, &ws, true).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE id = 'a1'", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}
```

Run: should pass.

- [ ] **Step 7: Test — idempotency.**

```rust
#[test]
fn migration_v6_backfill_is_idempotent() {
    let conn = new_db_with_legacy_row("code-explorer", "docs/x.md");
    let ws = ws_with("code-explorer", "/abs/c");
    backfill(&conn, &ws, false).unwrap();
    let first: String = conn.query_row(
        "SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| r.get(0),
    ).unwrap();
    backfill(&conn, &ws, false).unwrap();
    let second: String = conn.query_row(
        "SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(first, second);
}
```

Run: should pass.

- [ ] **Step 8: Test — commits backfill.**

```rust
#[test]
fn migration_v6_handles_commits_table() {
    let conn = new_db_with_legacy_row("code-explorer", "x.md");
    conn.execute(
        "INSERT INTO commits(hash, repo, topo_order) VALUES ('abc', 'code-explorer', 1)",
        [],
    ).unwrap();
    let ws = ws_with("code-explorer", "/abs/c");
    backfill(&conn, &ws, false).unwrap();
    let git_root: String = conn.query_row(
        "SELECT git_root FROM commits WHERE hash = 'abc'", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(git_root, "/abs/c");
}
```

Run: should pass.

- [ ] **Step 9: Wire backfill into `run_migrations`.** It needs the `WorkspaceConfig`, which `run_migrations` doesn't currently receive. Add a new `Catalog::open` variant that takes a workspace, AND keep the old one for tests/migrations that don't need backfill.

In `crates/librarian-mcp/src/catalog/mod.rs`, change `run_migrations` signature:

```rust
fn run_migrations(conn: &Connection, ws: Option<&WorkspaceConfig>) -> Result<()> {
    // … existing v1-v5 migrations unchanged …
    migrate_v6::add_columns(conn)?;
    if let Some(ws) = ws {
        let drop_orphans =
            std::env::var("LIBRARIAN_MIGRATE_DROP_ORPHANS").as_deref() == Ok("1");
        migrate_v6::backfill(conn, ws, drop_orphans)?;
    }
    Ok(())
}
```

Add a new catalog opener:

```rust
impl Catalog {
    pub fn open_with_workspace(db_path: &Path, ws: &WorkspaceConfig) -> Result<Self> {
        // Same as `open`, but passes ws through to run_migrations.
        let mut conn = Connection::open(db_path).context("opening catalog")?;
        init_sqlite_vec();
        unsafe { sqlite_vec::sqlite3_vec_init() };
        // … same pragmas as `open` …
        run_migrations(&conn, Some(ws)).context("running migrations")?;
        Ok(Self { conn })
    }
}
```

Update `Catalog::open` to call `run_migrations(conn, None)` so it remains test-friendly. Update `Catalog::open_in_memory` likewise.

- [ ] **Step 10: Update `build_tool_context`** at `crates/librarian-mcp/src/lib.rs:48` (the `Catalog::open(&db_path)?` line). Reorder so the workspace is loaded first, then pass it:

```rust
let ws = workspace::load(&cfg_path).with_context(/* … existing message … */)?;
let ws_arc = std::sync::Arc::new(ws);

// Catalog migration uses the workspace to backfill abs_path; pass by ref.
let catalog = catalog::Catalog::open_with_workspace(&db_path, &ws_arc)?;
```

The `ws_arc` clone is cheap; reuse it for the rest of `build_tool_context`.

- [ ] **Step 11: Run all librarian tests** — `cargo test -p librarian-mcp`. All pass.

- [ ] **Step 12: Commit**

```bash
git add crates/librarian-mcp/src/catalog/migrate_v6.rs \
        crates/librarian-mcp/src/catalog/mod.rs \
        crates/librarian-mcp/src/lib.rs
git commit -m "catalog(v6): backfill abs_path from repo + workspace.toml lookup"
```

---

## Task 3 — Path-prefix scope clauses

**Goal:** Replace `repo_clause` + `project_clause` with a single `path_prefix_clause`. Rewrite `apply_scope` to use the four-tier ladder against `abs_path`/`git_root`. Drop the collapse rule.

This task does NOT change the `current_project::resolve` shape yet (still produces `root: String, subdir: String`). To keep the build green, the new `path_prefix_clause` reads `abs_path` as if it were `repo + "/" + rel_path`. We unify in Task 4.

Actually the cleanest order is to flip the data model in `current_project` first (Task 4), then rewrite scope (Task 3) against the new shape. **Reorder: do Task 4 before Task 3.** The plan keeps the spec's commit-message order; the work order is 1 → 2 → 4 → 3 → 5 → 6 → 7 → 8 → 9.

**Files:**
- Modify: `crates/librarian-mcp/src/tools/scope.rs`
- Modify: `crates/librarian-mcp/src/tools/find.rs` (`call`, `count_for_scope`, `build_hints`)
- Modify: `crates/librarian-mcp/src/tools/context.rs` (similar scope hookup)
- Modify: `crates/librarian-mcp/src/tools/workspace_state_at.rs` (similar)
- Modify: `crates/librarian-mcp/src/tools/reindex.rs` (similar)
- Modify: `crates/librarian-mcp/src/tools/refresh_stale.rs` (similar)

- [ ] **Step 1: Test — `path_prefix_clause` covers self and descendants.** In `tools/scope.rs` `tests` module:

```rust
#[test]
fn path_prefix_clause_matches_self_and_descendants() {
    let p = std::path::PathBuf::from("/a/b");
    let node = path_prefix_clause(&p);
    let json = serde_json::to_value(&node).unwrap();
    // Expect: OR( eq /a/b , prefix /a/b/ )
    let or = json.get("or").expect("or node");
    let arr = or.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let eq = arr[0].pointer("/leaf/abs_path/eq").unwrap().as_str().unwrap();
    let prefix = arr[1].pointer("/leaf/abs_path/prefix").unwrap().as_str().unwrap();
    assert_eq!(eq, "/a/b");
    assert_eq!(prefix, "/a/b/");
}
```

- [ ] **Step 2: Run — fails (`path_prefix_clause` not defined).**

- [ ] **Step 3: Implement `path_prefix_clause`** in `tools/scope.rs` (replacing both `repo_clause` and `project_clause`):

```rust
use std::path::Path;

fn path_prefix_clause(p: &Path) -> FilterNode {
    let s = p.to_string_lossy().to_string();
    let prefix = format!("{s}/");
    FilterNode::Or {
        or: vec![
            FilterNode::Leaf(
                [("abs_path".to_string(), json!({"eq": s.clone()}))]
                    .into_iter().collect(),
            ),
            FilterNode::Leaf(
                [("abs_path".to_string(), json!({"prefix": prefix}))]
                    .into_iter().collect(),
            ),
        ],
    }
}
```

Delete `repo_clause` and `project_clause`. Update imports.

- [ ] **Step 4: Run test** — passes.

- [ ] **Step 5: Rewrite `apply_scope`.** Replace the current body with:

```rust
pub fn apply_scope(
    user_filter: Option<FilterNode>,
    scope: Scope,
    ws: &WorkspaceConfig,
    current: Option<&CurrentProject>,
) -> Result<(Option<FilterNode>, ScopeApplied)> {
    fn require(current: Option<&CurrentProject>, scope: &str) -> Result<&CurrentProject> {
        current.ok_or_else(|| anyhow::anyhow!(
            "scope={} requires an active project. The host has not activated one \
             (call workspace(action='activate', path=...)).",
            scope
        ))
    }
    let scope_clause = match scope {
        Scope::All => None,
        Scope::Project => Some(path_prefix_clause(&require(current, "project")?.abs_path)),
        Scope::Repo => Some(path_prefix_clause(&require(current, "repo")?.git_root)),
        Scope::Umbrella => {
            let cp = require(current, "umbrella")?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella declared for {}. \
                     Add a [[umbrella]] block to workspace.toml or use scope=repo|all.",
                    cp.abs_path.display(),
                )
            })?;
            let umb = ws.umbrellas.iter().find(|u| u.name == umbrella_name)
                .ok_or_else(|| anyhow::anyhow!("umbrella `{umbrella_name}` not found"))?;
            if umb.members.is_empty() {
                anyhow::bail!("umbrella `{umbrella_name}` has no members");
            }
            Some(or_of_prefixes(&umb.members))
        }
    };
    let combined = match (user_filter, scope_clause) {
        (Some(u), Some(s)) => Some(FilterNode::And { and: vec![u, s] }),
        (Some(u), None) => Some(u),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    };
    let applied = ScopeApplied {
        scope,
        abs_path: current.map(|c| c.abs_path.clone()),
        git_root: current.map(|c| c.git_root.clone()),
        umbrella: current.and_then(|c| c.umbrella.clone()),
    };
    Ok((combined, applied))
}

fn or_of_prefixes(members: &[std::path::PathBuf]) -> FilterNode {
    FilterNode::Or {
        or: members.iter().map(|m| path_prefix_clause(m)).collect(),
    }
}
```

(Note: `ScopeApplied` field rewrite is part of this task — see Step 6.)

- [ ] **Step 6: Update `ScopeApplied` struct + `to_json`** in `tools/scope.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ScopeApplied {
    pub scope: Scope,
    pub abs_path: Option<std::path::PathBuf>,
    pub git_root: Option<std::path::PathBuf>,
    pub umbrella: Option<String>,
}

impl ScopeApplied {
    pub fn to_json(&self) -> Value {
        json!({
            "applied": match self.scope {
                Scope::All => "all", Scope::Project => "project",
                Scope::Repo => "repo", Scope::Umbrella => "umbrella",
            },
            "abs_path": self.abs_path.as_ref().map(|p| p.to_string_lossy()),
            "git_root": self.git_root.as_ref().map(|p| p.to_string_lossy()),
            "umbrella": self.umbrella,
        })
    }
}
```

- [ ] **Step 7: Add the regression tests** in `tools/scope.rs` `tests` module:

```rust
#[test]
fn project_scope_uses_abs_path_not_root_name() {
    let ws = ws(vec![], vec![]);
    let cp = CurrentProject {
        abs_path: PathBuf::from("/a/b"),
        git_root: PathBuf::from("/a"),
        umbrella: None,
    };
    let (filter, _) = apply_scope(None, Scope::Project, &ws, Some(&cp)).unwrap();
    let s = serde_json::to_string(&filter.unwrap()).unwrap();
    assert!(s.contains("\"abs_path\""));
    assert!(s.contains("\"/a/b\""));
}

#[test]
fn repo_scope_uses_git_root() {
    let ws = ws(vec![], vec![]);
    let cp = CurrentProject {
        abs_path: PathBuf::from("/a/b"),
        git_root: PathBuf::from("/a"),
        umbrella: None,
    };
    let (filter, _) = apply_scope(None, Scope::Repo, &ws, Some(&cp)).unwrap();
    let s = serde_json::to_string(&filter.unwrap()).unwrap();
    assert!(s.contains("\"/a\""));
    assert!(!s.contains("\"/a/b\""));
}

#[test]
fn umbrella_scope_ors_member_prefixes() {
    let ws = WorkspaceConfig {
        roots: vec![],
        ignore: vec![],
        rules: vec![],
        umbrellas: vec![Umbrella {
            name: "team".into(),
            members: vec![PathBuf::from("/x"), PathBuf::from("/y")],
        }],
    };
    let cp = CurrentProject {
        abs_path: PathBuf::from("/x/sub"),
        git_root: PathBuf::from("/x"),
        umbrella: Some("team".into()),
    };
    let (filter, _) = apply_scope(None, Scope::Umbrella, &ws, Some(&cp)).unwrap();
    let s = serde_json::to_string(&filter.unwrap()).unwrap();
    assert!(s.contains("\"/x\""));
    assert!(s.contains("\"/y\""));
}

#[test]
fn project_scope_without_active_errors_with_new_message() {
    let ws = ws(vec![], vec![]);
    let err = apply_scope(None, Scope::Project, &ws, None).unwrap_err();
    let m = err.to_string();
    assert!(m.contains("requires an active project"));
    assert!(m.contains("workspace(action='activate'"));
}
```

Update the existing test helpers (`ws`, `cp`) in the `tests` module to produce the new shapes (no `Root.name`, `CurrentProject` with new fields). Drop `project_scope_with_subdir_ands_repo_and_prefix` and `project_scope_with_empty_subdir_collapses_to_repo` — both reference the removed semantics.

- [ ] **Step 8: Update `Umbrella.members` shape.** This needs to happen here for the umbrella test to compile. In `crates/librarian-mcp/src/workspace.rs`:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Umbrella {
    pub name: String,
    #[serde(default)]
    pub members: Vec<std::path::PathBuf>,   // was Vec<String>
}
```

Update `umbrella_clause` callers — actually that helper is gone now (replaced by `or_of_prefixes`). Sweep for stale references with `cargo build` and fix.

- [ ] **Step 9: Run all `tools/scope.rs` tests**

```
cargo test -p librarian-mcp tools::scope
```

All pass.

- [ ] **Step 10: Update tool callers that consume `ScopeApplied`.** Each of `tools/find.rs`, `tools/context.rs`, `tools/workspace_state_at.rs`, `tools/reindex.rs`, `tools/refresh_stale.rs` reads `applied.root`/`applied.subdir`. Replace with `applied.abs_path`/`applied.git_root`. The `find.rs` `build_hints` function at line 107-185 has the most touch points — read it carefully and update each `applied.root`/`applied.subdir` reference. Also drop any logic that branches on `project_is_root` (no longer exists).

- [ ] **Step 11: Cargo build** — `cargo build -p librarian-mcp`. Fix compile errors layer by layer.

- [ ] **Step 12: Run all librarian tests**

```
cargo test -p librarian-mcp
```

Existing find/context tests will still work because they bypass `apply_scope` via the `scope=All` path (which is unchanged). New scope tests pass.

- [ ] **Step 13: Commit**

```bash
git add crates/librarian-mcp/src/tools/scope.rs \
        crates/librarian-mcp/src/tools/find.rs \
        crates/librarian-mcp/src/tools/context.rs \
        crates/librarian-mcp/src/tools/workspace_state_at.rs \
        crates/librarian-mcp/src/tools/reindex.rs \
        crates/librarian-mcp/src/tools/refresh_stale.rs \
        crates/librarian-mcp/src/workspace.rs
git commit -m "scope: rewrite path_prefix_clause + apply_scope for path-based filters"
```

---

## Task 4 — `CurrentProject` struct rewrite

**Execution order note:** Per Task 3's reorder note, **do Task 4 BEFORE Task 3**. This task changes the data model that Task 3 depends on. The commit-message order in the spec stays the same; only the work order shifts.

**Goal:** Replace `CurrentProject { root: String, subdir: String, path: PathBuf, umbrella: Option<String> }` with `CurrentProject { abs_path: PathBuf, git_root: PathBuf, umbrella: Option<String> }`. Rewrite `current_project::resolve`. Drop `member_key()`.

**Files:**
- Modify: `crates/librarian-mcp/src/current_project.rs`
- Modify: `crates/librarian-mcp/src/lib.rs:67-72` (standalone fallback resolution)

- [ ] **Step 1: Tests for the new resolver.** Replace the `tests` module in `current_project.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_from_active_path_returns_self() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().to_path_buf();
        let ws = WorkspaceConfig::default();
        let cp = resolve(&p, &ws).unwrap();
        assert_eq!(cp.abs_path, std::fs::canonicalize(&p).unwrap());
    }

    #[test]
    fn resolve_finds_git_root_when_nested() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let nested = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        let cp = resolve(&nested, &WorkspaceConfig::default()).unwrap();
        assert_eq!(cp.git_root, std::fs::canonicalize(tmp.path()).unwrap());
    }

    #[test]
    fn resolve_falls_back_to_abs_path_when_no_git() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().to_path_buf();
        let cp = resolve(&p, &WorkspaceConfig::default()).unwrap();
        assert_eq!(cp.git_root, cp.abs_path);
    }

    #[test]
    fn resolve_returns_none_for_non_existent_path() {
        let p = std::path::Path::new("/nonexistent/zzz/qqq");
        assert!(resolve(p, &WorkspaceConfig::default()).is_none());
    }

    #[test]
    fn umbrella_lookup_includes_descendants() {
        let tmp = TempDir::new().unwrap();
        let umb_root = tmp.path().to_path_buf();
        let nested = umb_root.join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        let ws = WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![Umbrella {
                name: "team".into(),
                members: vec![std::fs::canonicalize(&umb_root).unwrap()],
            }],
        };
        let cp = resolve(&nested, &ws).unwrap();
        assert_eq!(cp.umbrella, Some("team".to_string()));
    }
}
```

Add `tempfile = "3"` to `crates/librarian-mcp/Cargo.toml` `[dev-dependencies]` if not already present.

- [ ] **Step 2: Run the tests** — fail (struct fields not changed).

- [ ] **Step 3: Replace `CurrentProject` struct** at line 19-30:

```rust
#[derive(Debug, Clone)]
pub struct CurrentProject {
    /// Absolute path of the active project (canonicalized).
    pub abs_path: PathBuf,
    /// Nearest enclosing `.git/` ancestor; falls back to abs_path.
    pub git_root: PathBuf,
    /// Umbrella name if this project is a descendant of any umbrella member.
    pub umbrella: Option<String>,
}
```

Drop `impl CurrentProject` (the `member_key` method is gone with no callers — the only one was in scope.rs which we're rewriting).

- [ ] **Step 4: Replace `resolve`** with the path-driven version:

```rust
pub fn resolve(active_path: &Path, ws: &WorkspaceConfig) -> Option<CurrentProject> {
    let abs_path = std::fs::canonicalize(active_path).ok()?;
    let git_root = walk_up_for_git(&abs_path).unwrap_or_else(|| abs_path.clone());
    let umbrella = lookup_umbrella(&abs_path, ws);
    Some(CurrentProject { abs_path, git_root, umbrella })
}

fn walk_up_for_git(start: &Path) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join(".git").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

pub fn lookup_umbrella(abs_path: &Path, ws: &WorkspaceConfig) -> Option<String> {
    ws.umbrellas.iter().find_map(|u| {
        u.members.iter().any(|m| abs_path.starts_with(m))
            .then(|| u.name.clone())
    })
}
```

Drop `nearest_git_root` (replaced by `walk_up_for_git`) and `find_umbrella` (replaced by `lookup_umbrella`, signature change).

- [ ] **Step 5: Update `WorkspaceConfig` in workspace.rs**. The `roots` field stays (needed for the v6 migration backfill), but is otherwise unused at query time:

```rust
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub roots: Vec<Root>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub umbrellas: Vec<Umbrella>,
}
```

(`Default` derive is added to support the test helpers.)

- [ ] **Step 6: Run the new tests** — pass. Build hint: any callers still referencing `cp.root`/`cp.subdir` won't compile. Fix them as you go (Task 3 sweeps these systematically).

- [ ] **Step 7: Update standalone fallback** in `crates/librarian-mcp/src/lib.rs:67-72`:

```rust
let current_project = std::env::var("LIBRARIAN_CWD")
    .map(PathBuf::from)
    .ok()
    .or_else(|| std::env::current_dir().ok())
    .and_then(|cwd| current_project::resolve(&cwd, &ws_arc))
    .map(std::sync::Arc::new);
if let Some(cp) = current_project.as_deref() {
    tracing::info!(
        "current project resolved: abs_path={} git_root={} umbrella={:?}",
        cp.abs_path.display(), cp.git_root.display(), cp.umbrella,
    );
} else {
    tracing::info!("current project unresolved — defaulting to workspace-wide scope");
}
```

- [ ] **Step 8: Update indexer signature**. `crates/librarian-mcp/src/indexer.rs::index_repo_sync` takes `repo_name: &str, repo_root: &Path, subdir: Option<&str>`. The new model: `abs_root: &Path` (project root or repo root, depending on caller). Replace the SQL inserts that write `(repo, rel_path)` with inserts writing `(abs_path)` only:

```rust
pub fn index_repo_sync(
    cat: &Catalog,
    rules: &[CompiledRule],
    abs_root: &Path,             // was repo_name + repo_root + subdir
    ignore: &globset::GlobSet,
    want_embeddings: bool,
) -> Result<(IndexReport, Vec<EmbedQueueItem>)> {
    // … walk the tree under abs_root …
    // For each markdown file, compute abs_path = file path (already absolute),
    // and write the row with (id, abs_path, kind, status, …).
}
```

(Update existing `INSERT … VALUES (?repo, ?rel_path, …)` calls in `catalog/artifact.rs` to take `abs_path` instead.)

This is invasive; expect 30-50 minutes here. Use `cargo build -p librarian-mcp` as a guide — fix each compile error in order.

- [ ] **Step 9: Update `ArtifactRow`** at `crates/librarian-mcp/src/catalog/artifact.rs:9-15`:

```rust
pub struct ArtifactRow {
    pub id: String,
    pub abs_path: PathBuf,    // was repo: String + rel_path: String
    pub kind: String,
    // … remaining fields unchanged …
}
```

Update insert/upsert/select SQL accordingly.

- [ ] **Step 10: Update find.rs SQL**. `crates/librarian-mcp/src/catalog/find.rs:432` constructs items with `repo: repo.into()` — update to `abs_path: PathBuf::from(abs_path_str)`. Sweep `repo: "r".into()` test helpers.

- [ ] **Step 11: Update augmentation.rs**. `crates/librarian-mcp/src/catalog/augmentation.rs:178` `ArtifactWithAugmentation.repo` field → `abs_path`. Adjust the `WHERE` clause at line 208 that filters by `repo`.

- [ ] **Step 12: Run full librarian build + tests**

```
cargo build -p librarian-mcp
cargo test -p librarian-mcp
```

If any tests still seed `repo: "r".into()` rows, update them to `abs_path: "/some/abs".into()`.

- [ ] **Step 13: Commit**

```bash
git add crates/librarian-mcp/src/current_project.rs \
        crates/librarian-mcp/src/workspace.rs \
        crates/librarian-mcp/src/lib.rs \
        crates/librarian-mcp/src/indexer.rs \
        crates/librarian-mcp/src/catalog/artifact.rs \
        crates/librarian-mcp/src/catalog/find.rs \
        crates/librarian-mcp/src/catalog/augmentation.rs \
        crates/librarian-mcp/src/preview/
git commit -m "current_project: replace root/subdir with abs_path/git_root"
```

(After Task 4 completes, return to Task 3 to rewrite scope clauses against the new model.)

---

## Task 5 — Dynamic `LibrarianAdapter`

**Goal:** `LibrarianAdapter::call` reads codescout's active project on every invocation and rebuilds `LibToolContext` with a fresh `current_project`. Closes the original cross-project tracker leak.

**Files:**
- Modify: `src/librarian.rs`

- [ ] **Step 1: Test — adapter uses live active project per call.** Add a test module to `src/librarian.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;

    async fn test_adapter() -> Arc<LibrarianAdapter> {
        let lib_ctx = librarian_mcp::build_tool_context().await
            .expect("test ctx");
        let lib_ctx = Arc::new(lib_ctx);
        let inner = librarian_mcp::tools::all_tools()
            .into_iter().find(|t| t.name() == "artifact").unwrap();
        Arc::new(LibrarianAdapter { inner, ctx: lib_ctx })
    }

    #[tokio::test]
    async fn adapter_uses_active_project_per_call() {
        let agent = Agent::new_test();   // assumes a test helper exists
        agent.activate("/path/to/project_a").await.unwrap();
        let adapter = test_adapter().await;
        let ctx = crate::tools::ToolContext::test_with_agent(&agent);
        let resp = adapter.call(json!({"action":"find","scope":"project"}), &ctx).await.unwrap();
        let abs = resp.pointer("/scope/abs_path").and_then(|v| v.as_str()).unwrap();
        assert_eq!(abs, "/path/to/project_a");

        agent.activate("/path/to/project_b").await.unwrap();
        let resp = adapter.call(json!({"action":"find","scope":"project"}), &ctx).await.unwrap();
        let abs = resp.pointer("/scope/abs_path").and_then(|v| v.as_str()).unwrap();
        assert_eq!(abs, "/path/to/project_b");
    }
}
```

(If `Agent::new_test` and `ToolContext::test_with_agent` don't exist, look at `src/agent/mod.rs::tests` for the existing pattern and reuse it. The test exists to prove the bug is fixed; the exact construction is whatever the codebase uses for agent tests today.)

- [ ] **Step 2: Run** — fails (assertion mismatch — adapter still frozen).

- [ ] **Step 3: Rewrite the adapter `call`** at `src/librarian.rs:62-64`:

```rust
async fn call(&self, input: Value, ctx: &crate::tools::ToolContext) -> Result<Value> {
    let active_root: Option<std::path::PathBuf> = {
        let inner = ctx.agent.inner.read().await;
        inner.active_project().map(|p| p.root.clone())
    };

    let lib_ctx = self.derive_ctx(active_root.as_deref()).await;
    self.inner.call(&lib_ctx, input).await
}
```

Add the helper method on the impl:

```rust
impl LibrarianAdapter {
    async fn derive_ctx(&self, active: Option<&std::path::Path>) -> Arc<librarian_mcp::tools::ToolContext> {
        let current_project = active.and_then(|p| {
            match std::fs::canonicalize(p) {
                Ok(abs_path) => {
                    let git_root = librarian_mcp::current_project::lookup_git_root(&abs_path)
                        .unwrap_or_else(|| abs_path.clone());
                    let umbrella = librarian_mcp::current_project::lookup_umbrella(
                        &abs_path, &self.ctx.workspace,
                    );
                    Some(Arc::new(librarian_mcp::current_project::CurrentProject {
                        abs_path, git_root, umbrella,
                    }))
                }
                Err(err) => {
                    tracing::warn!(
                        "active project path unresolvable: {} ({err})",
                        p.display()
                    );
                    None
                }
            }
        });

        Arc::new(librarian_mcp::tools::ToolContext {
            catalog: Arc::clone(&self.ctx.catalog),
            workspace: Arc::clone(&self.ctx.workspace),
            rules: Arc::clone(&self.ctx.rules),
            embedding: self.ctx.embedding.clone(),
            current_project,
        })
    }
}
```

(`lookup_git_root` is the public name of `walk_up_for_git`. Rename it in `current_project.rs` from Task 4 if you used the private name — make it `pub` so the adapter can call it.)

- [ ] **Step 4: Make `walk_up_for_git` and `lookup_umbrella` public** in `crates/librarian-mcp/src/current_project.rs`. Rename `walk_up_for_git` → `lookup_git_root` (clearer name; called from outside the module now).

- [ ] **Step 5: Run the test** — passes.

- [ ] **Step 6: Add the fallback test.**

```rust
#[tokio::test]
async fn adapter_falls_back_to_none_when_no_active_project() {
    let agent = Agent::new_test();   // no project activated
    let adapter = test_adapter().await;
    let ctx = crate::tools::ToolContext::test_with_agent(&agent);
    let resp = adapter.call(json!({"action":"find","scope":"all"}), &ctx).await.unwrap();
    let abs = resp.pointer("/scope/abs_path");
    assert!(abs.unwrap_or(&Value::Null).is_null());
}
```

Pass.

- [ ] **Step 7: Add the unresolvable-path test.**

```rust
#[tokio::test]
async fn adapter_falls_back_to_none_when_active_path_does_not_exist() {
    let agent = Agent::new_test();
    agent.activate("/this/path/does/not/exist/zzz").await.unwrap_or(());
    let adapter = test_adapter().await;
    let ctx = crate::tools::ToolContext::test_with_agent(&agent);
    let resp = adapter.call(json!({"action":"find","scope":"all"}), &ctx).await.unwrap();
    let abs = resp.pointer("/scope/abs_path");
    assert!(abs.unwrap_or(&Value::Null).is_null());
}
```

(If `agent.activate` itself errors on non-existent path before the adapter is even called, skip this test or stub the agent inner directly.)

- [ ] **Step 8: Run all adapter tests** — pass.

- [ ] **Step 9: Run full project tests + clippy + fmt**

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 10: Commit**

```bash
git add src/librarian.rs crates/librarian-mcp/src/current_project.rs
git commit -m "librarian: dynamic LibToolContext per call from active project"
```

---

## Task 6 — Drop legacy columns + bump schema_version

**Goal:** Drop `artifact.repo`, `artifact.rel_path`, `commits.repo`. Add `UNIQUE(abs_path)` constraint. Bump schema_version to 6. **Irreversible.** Backup before any drop.

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/migrate_v6.rs` (add `drop_legacy_and_stamp` function + tests)
- Modify: `crates/librarian-mcp/src/catalog/mod.rs` (call it; verify SQLite ≥ 3.35; create backup)
- Modify: `crates/librarian-mcp/src/catalog/schema.sql` (final shape — drop legacy columns from bootstrap, add `UNIQUE(abs_path)`)

- [ ] **Step 1: Test — backup file created.**

```rust
#[test]
fn migration_v6_creates_backup_file() {
    use std::fs;
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("catalog.db");
    // Seed a v3 DB at db_path with one row, then…
    seed_v3_db(&db_path);
    let ws = ws_with("r", tmp.path().to_str().unwrap());
    let _ = crate::catalog::Catalog::open_with_workspace(&db_path, &ws);
    let entries: Vec<_> = fs::read_dir(tmp.path()).unwrap()
        .map(|e| e.unwrap().file_name()).collect();
    assert!(
        entries.iter().any(|n| n.to_string_lossy().starts_with("catalog.db.pre-v6-bak.")),
        "backup file not created; entries: {:?}", entries
    );
}
```

- [ ] **Step 2: Run** — fails (no backup yet).

- [ ] **Step 3: Implement backup + drop in migrate_v6.rs**. Add:

```rust
use std::fs;
use std::path::Path;

/// Step 3 of the migration: drop legacy columns and stamp v6. Caller MUST
/// have already run `add_columns` and `backfill`. Backup is the caller's
/// responsibility and happens BEFORE this is called (in `Catalog::open_with_workspace`).
pub(super) fn drop_legacy_and_stamp(conn: &Connection) -> Result<()> {
    // SQLite version check
    let v: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    if !sqlite_version_supports_drop_column(&v) {
        anyhow::bail!(
            "SQLite {v} does not support ALTER DROP COLUMN (need ≥ 3.35). \
             Upgrade SQLite or restore the .pre-v6-bak file and downgrade librarian-mcp."
        );
    }

    // Idempotency: skip if already at v6.
    let current_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0),
    )?;
    if current_version >= 6 {
        return Ok(());
    }

    conn.execute_batch(r#"
        ALTER TABLE artifact DROP COLUMN repo;
        ALTER TABLE artifact DROP COLUMN rel_path;
        ALTER TABLE commits DROP COLUMN repo;
        DROP INDEX IF EXISTS idx_artifact_repo;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_artifact_abs_path ON artifact(abs_path);
        DROP INDEX IF EXISTS idx_commits_repo_topo;
        CREATE INDEX IF NOT EXISTS idx_commits_git_root ON commits(git_root, topo_order);
    "#)?;
    conn.execute("INSERT OR IGNORE INTO schema_version (version) VALUES (6)", [])?;
    Ok(())
}

fn sqlite_version_supports_drop_column(v: &str) -> bool {
    let parts: Vec<u32> = v.split('.').filter_map(|s| s.parse().ok()).collect();
    matches!(parts.as_slice(), [maj, min, ..] if (*maj, *min) >= (3, 35))
}
```

- [ ] **Step 4: Wire into `Catalog::open_with_workspace`** — backup happens here, before `run_migrations`:

```rust
pub fn open_with_workspace(db_path: &Path, ws: &WorkspaceConfig) -> Result<Self> {
    let needs_v6 = catalog_needs_v6_migration(db_path)?;
    if needs_v6 {
        backup_db(db_path)?;
    }

    let mut conn = Connection::open(db_path).context("opening catalog")?;
    init_sqlite_vec();
    unsafe { sqlite_vec::sqlite3_vec_init() };
    // … existing pragmas …

    run_migrations(&conn, Some(ws)).context("running migrations")?;

    if needs_v6 {
        migrate_v6::drop_legacy_and_stamp(&conn)?;
    }

    Ok(Self { conn })
}

fn catalog_needs_v6_migration(db_path: &Path) -> Result<bool> {
    if !db_path.exists() { return Ok(false); }
    let conn = Connection::open(db_path)?;
    // schema_version may not exist on truly fresh DBs; default to 0.
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0),
    ).unwrap_or(0);
    Ok(version < 6)
}

fn backup_db(db_path: &Path) -> Result<()> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let bak = db_path.with_extension(format!("db.pre-v6-bak.{ts}"));
    fs::copy(db_path, &bak).with_context(|| {
        format!("backing up catalog before v6 migration: {} → {}", db_path.display(), bak.display())
    })?;
    tracing::info!("v6 migration backup created at {}", bak.display());
    Ok(())
}
```

- [ ] **Step 5: Update bootstrap `SCHEMA_SQL`** at `crates/librarian-mcp/src/catalog/schema.sql`. For fresh DBs (no migration needed), the schema starts at v6. Replace the artifact/commits CREATE TABLEs:

```sql
CREATE TABLE IF NOT EXISTS artifact (
  id            TEXT PRIMARY KEY,
  abs_path      TEXT NOT NULL UNIQUE,
  kind          TEXT NOT NULL,
  status        TEXT NOT NULL,
  title         TEXT,
  owners        TEXT NOT NULL DEFAULT '[]',
  tags          TEXT NOT NULL DEFAULT '[]',
  topic         TEXT,
  time_scope    TEXT,
  source        TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  file_mtime    INTEGER NOT NULL,
  file_sha256   TEXT NOT NULL,
  confidence    REAL NOT NULL DEFAULT 1.0
);
-- … other tables unchanged …

CREATE TABLE IF NOT EXISTS commits (
  hash         TEXT PRIMARY KEY,
  git_root     TEXT NOT NULL,
  authored_at  INTEGER,
  subject      TEXT,
  topo_order   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_commits_git_root ON commits(git_root, topo_order);
```

Drop the legacy `idx_artifact_repo` line. Bump the final `INSERT OR IGNORE INTO schema_version (version) VALUES (3)` to `(6)` (or add a (4), (5), (6) trio if you want to keep the lineage explicit).

- [ ] **Step 6: Run** the test — passes.

- [ ] **Step 7: Test — full migration end-to-end.**

```rust
#[test]
fn migration_v6_full_path_translates_and_drops() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("catalog.db");
    seed_v3_db(&db_path);   // helper that sets up a v3 DB with one artifact row
    let ws = ws_with("r", tmp.path().to_str().unwrap());

    let cat = crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
    let count: i64 = cat.conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE abs_path IS NOT NULL", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
    // Legacy columns gone:
    let has_repo = column_exists(&cat.conn, "artifact", "repo").unwrap();
    assert!(!has_repo);
    // Schema bumped:
    let v: i64 = cat.conn.query_row(
        "SELECT MAX(version) FROM schema_version", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(v, 6);
}
```

Run, pass.

- [ ] **Step 8: Test — idempotency end-to-end.**

```rust
#[test]
fn migration_v6_full_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("catalog.db");
    seed_v3_db(&db_path);
    let ws = ws_with("r", tmp.path().to_str().unwrap());
    drop(crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap());
    // Second open is a no-op (schema_version already 6).
    let cat = crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
    let v: i64 = cat.conn.query_row(
        "SELECT MAX(version) FROM schema_version", [], |r| r.get(0),
    ).unwrap();
    assert_eq!(v, 6);
}
```

- [ ] **Step 9: Run full test suite + clippy + fmt**

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 10: Manual verification.** Build release, restart MCP:

```
cargo build --release
```

Restart Claude Code's MCP connection (`/mcp` slash command). Trigger an `artifact(find, kind=tracker)` call. Verify response includes `scope.abs_path` and `scope.git_root`, no `scope.root`/`subdir`. Activate a sub-project with `workspace(action="activate", ...)` and verify `artifact(find)` from a different project returns its own artifacts.

- [ ] **Step 11: Commit**

```bash
git add crates/librarian-mcp/src/catalog/migrate_v6.rs \
        crates/librarian-mcp/src/catalog/mod.rs \
        crates/librarian-mcp/src/catalog/schema.sql
git commit -m "schema(v6): drop repo/rel_path columns + bump schema_version"
```

---

## Task 7 — Deprecation warning for `[[roots]]`

**Goal:** Boot-time warning when workspace.toml still has `[[roots]]` blocks. Parsing remains functional for one release so the migration's lookup table works.

**Files:**
- Modify: `crates/librarian-mcp/src/workspace.rs::load`

- [ ] **Step 1: Test — warning emitted.** In `workspace.rs` `tests`:

```rust
#[test]
fn load_warns_on_legacy_roots() {
    use tempfile::NamedTempFile;
    let mut f = NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(f, r#"
        [[roots]]
        name = "x"
        path = "/abs/x"
    "#).unwrap();
    // Capture tracing output via tracing-test or just verify the warning is logged.
    // (Use whatever tracing assertion the codebase already uses; if none, drop this
    // test in favor of a manual stderr observation noted in the PR.)
    let cfg = load(f.path()).unwrap();
    assert_eq!(cfg.roots.len(), 1, "still parsed");
}
```

If the codebase has no test-tracing helper, replace this with a simple "still parsed" smoke test and verify the warning manually.

- [ ] **Step 2: Add the warning** at `workspace.rs::load`:

```rust
pub fn load(path: &Path) -> Result<WorkspaceConfig> {
    let s = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: WorkspaceConfig = toml::from_str(&s).with_context(|| format!("parsing {}", path.display()))?;
    if !cfg.roots.is_empty() {
        tracing::warn!(
            "[[roots]] is deprecated; safe to remove from {} after the v6 migration completes. \
             Roots are no longer consulted at query time. See the v6 release notes.",
            path.display()
        );
    }
    Ok(cfg)
}
```

- [ ] **Step 3: Run tests** — pass.

- [ ] **Step 4: Manual smoke** — start librarian-mcp standalone with a workspace.toml that still has `[[roots]]`. Verify the deprecation warning appears in stderr.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/workspace.rs
git commit -m "workspace.toml: deprecate [[roots]], emit warning"
```

---

## Task 8 — Three-surface prompt updates + ONBOARDING_VERSION bump

**Goal:** Update tool descriptions and prompt surfaces to reflect the new scope ladder and removed terminology. Bump `ONBOARDING_VERSION`. Ensure `prompt_surfaces_reference_only_real_tools` test passes.

**Files:**
- Modify: `crates/librarian-mcp/src/tools/artifact.rs` (description text references "scope=project (current sub-project only)")
- Modify: `crates/librarian-mcp/src/tools/librarian.rs` (similar)
- Modify: `crates/librarian-mcp/src/prompts/server_instructions.md`
- Modify: `crates/librarian-mcp/src/prompts/companion_hint.md`
- Modify: `src/prompts/server_instructions.md` (codescout-side; see CLAUDE.md "Prompt Surface Consistency")
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs::build_system_prompt_draft`
- Modify: `src/tools/onboarding.rs:21` — bump `ONBOARDING_VERSION` from 23 to 24

- [ ] **Step 1: Sweep tool descriptions for stale terms.** Run:

```
grep -nE "sub-project|current sub-project|repo == |\[\[roots\]\]" crates/librarian-mcp/src/tools/
```

Each hit needs a rewrite. Pattern: replace "current sub-project" → "active project" and "repo" (when used in scope-doc context) → "the active project's enclosing git repo".

- [ ] **Step 2: Rewrite the three librarian-mcp prompts.** In `crates/librarian-mcp/src/prompts/server_instructions.md` find the "Default scope (project, archived hidden)" section and rewrite the scope ladder:

```markdown
## Default scope (project, archived hidden)

`artifact(find, …)` defaults to `scope="project"` — only artifacts under the
active project's path are returned. Sibling projects under the same git repo
are excluded. Override:

- `scope="repo"` — widen to the entire enclosing git checkout
- `scope="umbrella"` — widen to all projects in the active project's umbrella
  (declared in workspace.toml `[[umbrella]]`)
- `scope="all"` — query the whole catalog (only allowed when no umbrella is
  declared; otherwise use `scope="umbrella"`)

The host's active project is always the reference path; activate via
`workspace(action="activate", path=...)`.
```

Apply analogous changes in `companion_hint.md`.

- [ ] **Step 3: Rewrite codescout-side prompts.** In `src/prompts/server_instructions.md` and `src/prompts/onboarding_prompt.md`, replace stale references to librarian's `repo`/`root` parameters with the new `scope` ladder language. Don't duplicate the librarian-side prose — keep the codescout-side prompts compact, e.g.: "Librarian filters default to your active project; pass scope=repo|umbrella|all to widen."

- [ ] **Step 4: Update `build_system_prompt_draft`** at `src/prompts/builders.rs`. Find the section that describes librarian scope; mirror the same compact text from Step 3.

- [ ] **Step 5: Bump `ONBOARDING_VERSION`** at `src/tools/onboarding.rs:21`:

```rust
pub(crate) const ONBOARDING_VERSION: u32 = 24;
```

- [ ] **Step 6: Run the prompt-surface guard test**

```
cargo test prompt_surfaces_reference_only_real_tools
```

If it fails, the failure message lists tokens it considered "stale tool names". Either fix the surface or extend the test's allowlist if the token is a parameter name (not a tool name) — see existing allowlist in the test for examples.

- [ ] **Step 7: Run full test suite + clippy + fmt**

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 8: Manual verification.** Rebuild release; restart MCP; run `onboarding(action="refresh_prompt")`; check the regenerated `system-prompt.md` reflects the new scope ladder.

- [ ] **Step 9: Commit**

```bash
git add crates/librarian-mcp/src/tools/ \
        crates/librarian-mcp/src/prompts/ \
        src/prompts/ \
        src/tools/onboarding.rs
git commit -m "prompts: update server_instructions + companion_hint + onboarding for new scope ladder"
```

---

## Task 9 — Spec note + final verification

**Goal:** Final sweep. Add a follow-up note in the spec referring to the implementation PR. Run all checks. Manual end-to-end verification of the original bug.

**Files:**
- Modify: `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md` (add a "## Implementation status" section at the bottom)
- Modify: `docs/RELEASE-TODO.md` (add release-notes bullets per spec rollout section)

- [ ] **Step 1: Add implementation status to the spec.** Append:

```markdown
## Implementation status

Implemented in commits b…h on `experiments` branch (commit shas filled in
after merge). Verified manually:

- Activated `code-explorer` → `artifact(find, kind=tracker)` returns only
  code-explorer artifacts.
- Activated `tests/fixtures/rust-library` → `artifact(find, kind=tracker)`
  returns no rows (none indexed under that path; sub-project trackers no
  longer leak from siblings).
- Activated `tests/fixtures/rust-library` with `scope=repo` → returns the
  full code-explorer artifact set (correctly widened).
- `scope=umbrella` errors with "no umbrella declared" since the workspace
  has none.
- Catalog backup file `catalog.db.pre-v6-bak.<ts>` exists in
  `~/.local/share/librarian/`.
```

- [ ] **Step 2: Add release notes** to `docs/RELEASE-TODO.md` per spec § Release coordination. (Schema migration runs on first launch; backup file is created automatically; `[[roots]]` deprecated; new scope ladder.)

- [ ] **Step 3: Final pre-completion checks** (CLAUDE.md "Always run … before completing any task"):

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

- [ ] **Step 4: Manual end-to-end verification of the original bug.** Restart MCP. From `code-explorer`:

```
artifact(action="find", kind="tracker", scope="project")
```

Note the count.

```
workspace(action="activate", path="tests/fixtures/rust-library", read_only=true)
artifact(action="find", kind="tracker", scope="project")
```

Expect: different (likely empty) result set, with `scope.abs_path` reflecting `rust-library`. Restore `code-explorer` via `workspace(action="activate", path="code-explorer")`.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md \
        docs/RELEASE-TODO.md
git commit -m "docs: record librarian project-model redesign in docs/superpowers/specs/"
```

- [ ] **Step 6: Cherry-pick to `master` per CLAUDE.md "Standard Ship Sequence".**

```bash
# All commits in this feature land on experiments first.
# When the work is verified end-to-end on experiments, ship to master
# via cherry-pick (see CLAUDE.md § Git Workflow > Standard Ship Sequence).
# This step is OUT OF SCOPE for the implementation plan — defer until
# manual verification confirms zero regressions across the full test suite
# AND a session of real use in Claude Code.
```

(This step is a reminder, not an action — defer cherry-pick to master until the user explicitly approves.)

---

## Self-review checklist

Before marking this plan complete, ensure:

- [ ] Every task corresponds to a spec section (rollout, migration, scope, wiring, prompts).
- [ ] No "TBD"/"TODO"/"implement based on spec" placeholders.
- [ ] Type names are consistent across tasks (`abs_path` not `absPath`/`abs-path`; `git_root` not `gitRoot`; `CurrentProject` fields match across Task 4 and Task 5).
- [ ] Function signatures stay stable across the tasks that reference them.
- [ ] Each task ends with a commit using the message from the spec § Rollout.
- [ ] The order of work (1, 2, **4, 3**, 5, 6, 7, 8, 9) is documented in Task 3's note.

---

## Notes for the executor

- **Task 4 must run before Task 3** even though spec lists scope before current_project. Without the new `CurrentProject` shape, the new `apply_scope` won't compile.
- **Backup files accumulate.** Each migration run creates a new `catalog.db.pre-v6-bak.<ts>`. After successful manual verification, the user can delete old backups; we don't auto-rotate.
- **`import-codescout` is obsolete in this model.** The spec defers its removal to a follow-up PR. If you encounter test failures referencing `import-codescout`, either skip them (annotate with a TODO referencing the follow-up) or update them to the new model.
- **SQLite version dependency.** If `cargo test` fails with "ALTER DROP COLUMN not supported", the bundled `rusqlite` is stale. Bump the dependency in `Cargo.toml` or enable `bundled` feature for sqlite ≥ 3.35.
- **`commits.git_root` index naming.** `idx_commits_git_root` replaces `idx_commits_repo_topo`; secondary key remains `topo_order`. Don't drop the secondary key.
