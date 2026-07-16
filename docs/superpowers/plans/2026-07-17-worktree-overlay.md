# Worktree Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Worktree sessions read the main repo's librarian catalog through an overlay, write to shadow rows with lineage recorded at write time, and merge via a first-class `librarian(action="merge_worktree")`.

**Architecture:** One new SQLite table (`worktree_registration`) in the existing machine-global catalog; shadow artifact rows at worktree paths (identity model unchanged); lineage = bare `worktree_of` link + `worktree_fork` event carrying the base snapshot; merge = delta extraction against the base, folded through graft primitives extracted into reusable helpers.

**Tech Stack:** Rust, rusqlite, serde_json, ulid, chrono. All work in `src/librarian/`.

**Spec:** `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md`
**Session log:** `docs/trackers/worktree-overlay-session-log.md` (F-1, F-2 — both encoded as tasks below)

## Global Constraints

- Branch: `experiments` only. `master` is protected.
- Gates before any commit: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --lib` (fast unit cycle) — full `cargo test` in Task 9.
- Errors a user/agent can fix → `RecoverableError::new(...)` (maps to `isError:false`); internal invariant breaks → `anyhow::bail!`.
- All catalog paths are stored forward-slash-normalized. Always convert with `crate::util::fs::RepoPath::from(path).into_string()` before storing or comparing against `abs_path`/root columns.
- Artifact identity is `crate::librarian::ids::artifact_id_from_abs(abs_path)` — sha256 of the forward-slash path, 16 hex chars. Never invent ids.
- Commit style: conventional commits (`feat(librarian): ...`), each ending with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- **Spec deviation (decided at plan time, already reflected below):** the fork event does NOT carry a `base_event_seq`. A shadow row is *born* at fork, so every event on it except the `worktree_fork` event itself is worktree-born by construction; merge re-points them all. Three-way detection for scalars uses value comparison against `base_params`/`base_frontmatter`, which needs no sequence number. Task 9 syncs this into the spec.

---

### Task 1: `worktree_registration` table + catalog module

**Files:**
- Modify: `src/librarian/catalog/schema.sql` (append table DDL)
- Create: `src/librarian/catalog/worktree.rs`
- Modify: `src/librarian/catalog/mod.rs` (add `pub mod worktree;` next to the other modules, lines 7–17)

**Interfaces:**
- Produces: `RegistrationRow { worktree_root: String, main_root: String, branch: Option<String>, created_at: i64, status: String, closed_at: Option<i64> }`
- Produces: `upsert_active(cat, worktree_root, main_root, branch, now) -> Result<()>`, `get(cat, worktree_root) -> Result<Option<RegistrationRow>>`, `covering(cat, abs_path) -> Result<Option<RegistrationRow>>`, `active_roots(cat) -> Result<Vec<String>>`, `set_status(cat, worktree_root, status, now) -> Result<bool>`
- All path arguments are forward-slash strings (callers normalize).

- [ ] **Step 1: Append DDL to `schema.sql`** (idempotent — schema runs on every `Catalog::open`):

```sql
-- Worktree overlay: durable registration of linked git worktrees that have
-- written to the catalog. Survives `git worktree remove`; the merge flow
-- (librarian action=merge_worktree) closes it. See
-- docs/superpowers/specs/2026-07-17-worktree-overlay-design.md.
CREATE TABLE IF NOT EXISTS worktree_registration (
    worktree_root TEXT PRIMARY KEY,
    main_root     TEXT NOT NULL,
    branch        TEXT,
    created_at    INTEGER NOT NULL,
    status        TEXT NOT NULL DEFAULT 'active',
    closed_at     INTEGER
);
```

- [ ] **Step 2: Write failing tests** in `src/librarian/catalog/worktree.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;

    #[test]
    fn upsert_get_roundtrip_and_reactivation() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", Some("feat-x"), 1000).unwrap();
        let r = get(&cat, "/repo/.worktrees/feat").unwrap().unwrap();
        assert_eq!(r.main_root, "/repo");
        assert_eq!(r.status, "active");
        assert_eq!(r.branch.as_deref(), Some("feat-x"));
        // Re-upsert after close flips it back to active (a re-used worktree path).
        assert!(set_status(&cat, "/repo/.worktrees/feat", "merged", 2000).unwrap());
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", Some("feat-x"), 3000).unwrap();
        assert_eq!(get(&cat, "/repo/.worktrees/feat").unwrap().unwrap().status, "active");
    }

    #[test]
    fn covering_matches_paths_under_active_root_only() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", None, 1000).unwrap();
        assert!(covering(&cat, "/repo/.worktrees/feat/docs/t.md").unwrap().is_some());
        assert!(covering(&cat, "/repo/.worktrees/feat").unwrap().is_some());
        assert!(covering(&cat, "/repo/.worktrees/feature-other/x.md").unwrap().is_none());
        assert!(covering(&cat, "/repo/docs/t.md").unwrap().is_none());
        set_status(&cat, "/repo/.worktrees/feat", "abandoned", 2000).unwrap();
        assert!(covering(&cat, "/repo/.worktrees/feat/docs/t.md").unwrap().is_none());
    }

    #[test]
    fn active_roots_lists_only_active() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/a", "/repo", None, 1).unwrap();
        upsert_active(&cat, "/repo/.worktrees/b", "/repo", None, 2).unwrap();
        set_status(&cat, "/repo/.worktrees/b", "merged", 3).unwrap();
        assert_eq!(active_roots(&cat).unwrap(), vec!["/repo/.worktrees/a".to_string()]);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib librarian::catalog::worktree`
Expected: compile error (module functions not defined)

- [ ] **Step 4: Implement**

```rust
//! Worktree overlay registration rows. One row per linked worktree that has
//! written to the catalog. `covering` is the hot lookup: "is this abs_path
//! inside an active worktree overlay?"
use anyhow::Result;
use rusqlite::{params, OptionalExtension};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationRow {
    pub worktree_root: String,
    pub main_root: String,
    pub branch: Option<String>,
    pub created_at: i64,
    pub status: String,
    pub closed_at: Option<i64>,
}

fn row_from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<RegistrationRow> {
    Ok(RegistrationRow {
        worktree_root: r.get(0)?,
        main_root: r.get(1)?,
        branch: r.get(2)?,
        created_at: r.get(3)?,
        status: r.get(4)?,
        closed_at: r.get(5)?,
    })
}

const COLS: &str = "worktree_root, main_root, branch, created_at, status, closed_at";

/// Insert or re-activate a registration. Re-upserting a closed row (a re-used
/// worktree path) resets status to `active` and clears `closed_at`; the
/// original `created_at` is preserved on conflict.
pub fn upsert_active(
    cat: &Catalog,
    worktree_root: &str,
    main_root: &str,
    branch: Option<&str>,
    now: i64,
) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO worktree_registration (worktree_root, main_root, branch, created_at, status, closed_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', NULL) \
         ON CONFLICT(worktree_root) DO UPDATE SET \
           main_root=excluded.main_root, branch=excluded.branch, status='active', closed_at=NULL",
        params![worktree_root, main_root, branch, now],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, worktree_root: &str) -> Result<Option<RegistrationRow>> {
    Ok(cat
        .conn
        .query_row(
            &format!("SELECT {COLS} FROM worktree_registration WHERE worktree_root=?1"),
            [worktree_root],
            row_from_sql,
        )
        .optional()?)
}

/// The ACTIVE registration whose root is `abs_path` or an ancestor of it.
pub fn covering(cat: &Catalog, abs_path: &str) -> Result<Option<RegistrationRow>> {
    Ok(cat
        .conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM worktree_registration \
                 WHERE status='active' AND (?1 = worktree_root OR ?1 LIKE worktree_root || '/%')"
            ),
            [abs_path],
            row_from_sql,
        )
        .optional()?)
}

/// Roots of every ACTIVE registration (for scope exclusion clauses).
pub fn active_roots(cat: &Catalog) -> Result<Vec<String>> {
    let mut stmt = cat
        .conn
        .prepare("SELECT worktree_root FROM worktree_registration WHERE status='active' ORDER BY worktree_root")?;
    let roots = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(roots)
}

/// Returns false when no row exists for `worktree_root`.
pub fn set_status(cat: &Catalog, worktree_root: &str, status: &str, now: i64) -> Result<bool> {
    let n = cat.conn.execute(
        "UPDATE worktree_registration SET status=?2, closed_at=?3 WHERE worktree_root=?1",
        params![worktree_root, status, now],
    )?;
    Ok(n > 0)
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib librarian::catalog::worktree`
Expected: 3 passed

- [ ] **Step 6: Commit**

```bash
git add src/librarian/catalog/schema.sql src/librarian/catalog/worktree.rs src/librarian/catalog/mod.rs
git commit -m "feat(librarian): worktree_registration table + catalog module

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `CurrentProject.main_root` + umbrella via main root

**Files:**
- Modify: `src/librarian/current_project.rs` (struct at lines 19–26, `resolve` at 28–37)
- Modify (compiler-driven, add `main_root: None` to `CurrentProject` struct literals): `src/librarian/tools/scope.rs` (tests `cp()` helper, line ~167), `src/librarian/tools/delete.rs:207`, `src/librarian/tools/mv.rs:233,281`, `src/librarian/tools/reindex.rs:365,430`, plus test literals in `audit_doc_refs/mod.rs`, `legibility_scan/mod.rs`, `tracker_design.rs`, and any others the compiler flags.

**Interfaces:**
- Produces: `CurrentProject.main_root: Option<PathBuf>` — `Some(<main repo root>)` iff the session's git root is a linked worktree.
- Umbrella resolution unchanged in signature; behavior: resolves against `main_root` when present.

- [ ] **Step 1: Write failing tests** (append inside the existing `tests` module of `current_project.rs`; reuse the `.git`-file fixture idiom from `is_linked_worktree_detects_worktree_not_submodule_or_main`, lines 191–234):

```rust
#[test]
fn resolve_populates_main_root_for_linked_worktree() {
    let tmp = tempfile::TempDir::new().unwrap();
    let main = tmp.path().join("main");
    std::fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
    let wt = tmp.path().join("wt");
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(
        wt.join(".git"),
        format!("gitdir: {}/.git/worktrees/feat\n", main.display()),
    )
    .unwrap();
    let ws = crate::librarian::workspace::WorkspaceConfig::default();
    let cp = resolve(&wt, &ws).unwrap();
    assert_eq!(
        cp.main_root.as_deref(),
        Some(std::fs::canonicalize(&main).unwrap().as_path())
    );
}

#[test]
fn resolve_main_root_none_for_plain_repo() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
    let ws = crate::librarian::workspace::WorkspaceConfig::default();
    let cp = resolve(tmp.path(), &ws).unwrap();
    assert!(cp.main_root.is_none());
}

#[test]
fn umbrella_resolves_via_main_root_from_worktree() {
    let tmp = tempfile::TempDir::new().unwrap();
    let main = tmp.path().join("main");
    std::fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
    let wt = tmp.path().join("wt");
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::write(
        wt.join(".git"),
        format!("gitdir: {}/.git/worktrees/feat\n", main.display()),
    )
    .unwrap();
    let canon_main = std::fs::canonicalize(&main).unwrap();
    let ws = crate::librarian::workspace::WorkspaceConfig {
        umbrellas: vec![crate::librarian::workspace::Umbrella {
            name: "u".into(),
            members: vec![canon_main.to_string_lossy().into_owned()],
        }],
        ..Default::default()
    };
    let cp = resolve(&wt, &ws).unwrap();
    assert_eq!(cp.umbrella.as_deref(), Some("u"));
}
```

Adjust `Umbrella`/`WorkspaceConfig` construction to their real field shapes if they differ (check with `symbols(name="Umbrella")`) — the assertion targets stay the same.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib librarian::current_project`
Expected: compile error (`main_root` not a field)

- [ ] **Step 3: Implement.** In the struct (after `git_root`):

```rust
/// Main checkout root when `git_root` is a *linked* git worktree
/// (`git worktree add`); `None` for a plain checkout. Drives overlay
/// reads and fork-on-first-write in `librarian::tools`.
pub main_root: Option<PathBuf>,
```

`resolve` becomes:

```rust
pub fn resolve(active_path: &Path, ws: &WorkspaceConfig) -> Option<CurrentProject> {
    let abs_path = std::fs::canonicalize(active_path).ok()?;
    let git_root = lookup_git_root(&abs_path).unwrap_or_else(|| abs_path.clone());
    let main_root = if is_linked_worktree(&git_root) {
        worktree_main_root(&git_root)
            .and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)))
    } else {
        None
    };
    // Umbrella membership is a property of the PROJECT, not the checkout —
    // resolve it against the main root so worktree sessions keep umbrella scope.
    let umbrella = lookup_umbrella(main_root.as_deref().unwrap_or(&abs_path), ws);
    Some(CurrentProject {
        abs_path,
        git_root,
        main_root,
        umbrella,
    })
}
```

Then fix every struct-literal construction site the compiler flags with `main_root: None`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib librarian`
Expected: all pass (new 3 + no regressions)

- [ ] **Step 5: Commit**

```bash
git add -A src/librarian
git commit -m "feat(librarian): CurrentProject.main_root — worktree-aware resolve + umbrella via main root

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: overlay scope clauses + main-session shadow exclusion

**Files:**
- Modify: `src/librarian/tools/scope.rs` (`apply_scope` lines 54–123, tests)
- Modify: `src/librarian/tools/find.rs` (`call` line ~438, `count_for_scope` line ~223, `build_hints` passthrough)

**Interfaces:**
- Changes: `apply_scope(user_filter, scope, ws, current, exclude_worktrees: &[String])` — new final param: forward-slash roots of active worktrees to exclude from the scope clause. Callers pass `&[]` to keep old behavior.
- Behavior: worktree session (`current.main_root.is_some()`) + `Project`/`Repo` scope → `OR(prefix(session root), prefix(main_root))`. Any session: when `exclude_worktrees` is non-empty and a scope clause exists, wrap as `AND(scope, NOT(or_of_prefixes(exclude)))`.

- [ ] **Step 1: Write failing tests** in `scope.rs` tests module (extend the `cp()` helper with a `main_root` variant):

```rust
fn cp_wt(abs_path: &str, git_root: &str, main_root: &str) -> CurrentProject {
    CurrentProject {
        abs_path: abs_path.into(),
        git_root: git_root.into(),
        main_root: Some(main_root.into()),
        umbrella: None,
    }
}

#[test]
fn worktree_project_scope_unions_worktree_and_main_prefixes() {
    let ws = ws(vec![], vec![]);
    let current = cp_wt("/repo/.worktrees/feat", "/repo/.worktrees/feat", "/repo");
    let (f, _) = apply_scope(None, Scope::Project, &ws, Some(&current), &[]).unwrap();
    let s = serde_json::to_string(&f.unwrap()).unwrap();
    assert!(s.contains("/repo/.worktrees/feat/"), "worktree prefix present: {s}");
    assert!(s.contains(r#""prefix":"/repo/""#), "main prefix present: {s}");
}

#[test]
fn exclusion_wraps_scope_with_not_prefix() {
    let ws = ws(vec![], vec![]);
    let current = cp("/repo", "/repo", None);
    let (f, _) = apply_scope(
        None,
        Scope::Project,
        &ws,
        Some(&current),
        &["/repo/.worktrees/feat".to_string()],
    )
    .unwrap();
    let s = serde_json::to_string(&f.unwrap()).unwrap();
    assert!(s.contains(r#""not""#), "NOT clause present: {s}");
    assert!(s.contains("/repo/.worktrees/feat/"), "excluded prefix present: {s}");
}

#[test]
fn no_exclusion_clause_when_list_empty() {
    let ws = ws(vec![], vec![]);
    let current = cp("/repo", "/repo", None);
    let (f, _) = apply_scope(None, Scope::Project, &ws, Some(&current), &[]).unwrap();
    assert!(!serde_json::to_string(&f.unwrap()).unwrap().contains(r#""not""#));
}
```

(The existing `cp()` helper gains `main_root: None`; keep its current signature by updating its body only.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib librarian::tools::scope`
Expected: compile error (arity) then assertion failures

- [ ] **Step 3: Implement.** In `apply_scope`, replace the `Scope::Project` / `Scope::Repo` arms and the combine step:

```rust
let scope_clause = match scope {
    Scope::All => None,
    Scope::Project => {
        let cp = require(current, "project")?;
        Some(match &cp.main_root {
            // Overlay: a worktree session sees its own rows AND the main
            // checkout's rows; shadow-vs-main dedup happens post-query in find.
            Some(main) => FilterNode::Or {
                or: vec![path_prefix_clause(&cp.abs_path), path_prefix_clause(main)],
            },
            None => path_prefix_clause(&cp.abs_path),
        })
    }
    Scope::Repo => {
        let cp = require(current, "repo")?;
        Some(match &cp.main_root {
            Some(main) => FilterNode::Or {
                or: vec![path_prefix_clause(&cp.git_root), path_prefix_clause(main)],
            },
            None => path_prefix_clause(&cp.git_root),
        })
    }
    Scope::Umbrella => { /* unchanged */ }
};

// Shadow rows belong to their worktree's overlay: every other session
// excludes them. (In-repo layouts like <main>/.worktrees/<n> would
// otherwise match the main prefix.)
let scope_clause = match (scope_clause, exclude_worktrees.is_empty()) {
    (Some(sc), false) => Some(FilterNode::And {
        and: vec![
            sc,
            FilterNode::Not {
                not: Box::new(or_of_prefixes(exclude_worktrees)),
            },
        ],
    }),
    (sc, _) => sc,
};
```

Check `or_of_prefixes`'s parameter type (line 147–151) — it takes the umbrella member list; if that is `&[String]` of path strings this composes directly, otherwise add a sibling helper with the same body over `&[String]`.

- [ ] **Step 4: Update callers in `find.rs`.** Before the `apply_scope` call at line ~438, compute the exclusion list (session's own worktree root never excludes itself):

```rust
let exclude_worktrees: Vec<String> = {
    let cat = ctx.catalog.lock();
    let own = current
        .filter(|c| c.main_root.is_some())
        .map(|c| crate::util::fs::RepoPath::from(c.git_root.as_path()).into_string());
    crate::librarian::catalog::worktree::active_roots(&cat)?
        .into_iter()
        .filter(|r| own.as_deref() != Some(r.as_str()))
        .collect()
};
let (scoped_filter, applied) =
    apply_scope(base.clone(), effective_scope, &ctx.workspace, current, &exclude_worktrees)?;
```

`count_for_scope` (line 210–225) gains the same `exclude_worktrees: &[String]` parameter and passes it through; `build_hints` threads it (it already receives `&cat`, so it may compute or receive the list — pass it down from `call` to keep one source of truth).

- [ ] **Step 5: Run tests**

Run: `cargo test --lib librarian::tools`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/scope.rs src/librarian/tools/find.rs
git commit -m "feat(librarian): overlay scope — worktree sessions read main, all sessions exclude foreign shadows

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: fork-on-first-write — `resolve_write_target`

**Files:**
- Create: `src/librarian/tools/worktree.rs`
- Modify: `src/librarian/tools/mod.rs` (add `pub(crate) mod worktree;`)

**Interfaces:**
- Consumes: Task 1 registration CRUD, Task 2 `main_root`.
- Produces: `pub(crate) fn resolve_write_target(cat: &mut Catalog, ctx: &ToolContext, id: &str) -> Result<String>` — returns the id writes must target (shadow id when forked, input id otherwise).
- Produces: `pub(crate) fn ensure_registration(cat: &Catalog, cp: &CurrentProject) -> Result<()>` (no-op for non-worktree sessions).
- Produces: event kind `"worktree_fork"`, link rel `"worktree_of"` (shadow → main).

- [ ] **Step 1: Write failing tests** (in `worktree.rs`; build contexts with `TestToolContextBuilder` + `with_current_project`, per the idiom in `reindex.rs:363-370`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, augmentation, events, links, worktree as reg, Catalog, TestArtifactRowBuilder};
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;
    use std::sync::Arc;

    fn wt_ctx(cat: Catalog) -> crate::librarian::tools::ToolContext {
        TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(CurrentProject {
                abs_path: "/repo/.worktrees/feat".into(),
                git_root: "/repo/.worktrees/feat".into(),
                main_root: Some("/repo".into()),
                umbrella: None,
            }))
            .build()
    }

    fn seed_main_tracker(cat: &Catalog) -> String {
        let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/repo/docs/trackers/t.md"));
        artifact::upsert(cat, &TestArtifactRowBuilder::new(&id)
            .with_abs_path("/repo/docs/trackers/t.md")
            .with_kind("tracker")
            .build()).unwrap();
        augmentation::upsert(cat, &crate::librarian::catalog::AugmentationRow {
            artifact_id: id.clone(),
            prompt: "p".into(),
            params: r#"{"items":[{"id":"F-1","t":"a"}],"note":"base"}"#.into(),
            entry_collection: Some("items".into()),
            ..Default::default()
        }).unwrap();
        id
    }

    #[test]
    fn fork_creates_shadow_with_lineage_registration_and_base() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = { let cat = ctx.catalog.lock(); seed_main_tracker(&cat) };
        let shadow_id = {
            let mut cat = ctx.catalog.lock();
            resolve_write_target(&mut cat, &ctx, &main_id).unwrap()
        };
        assert_ne!(shadow_id, main_id);
        let cat = ctx.catalog.lock();
        let shadow = artifact::get(&cat, &shadow_id).unwrap().unwrap();
        assert_eq!(shadow.abs_path.to_string_lossy(), "/repo/.worktrees/feat/docs/trackers/t.md");
        // params seeded from base
        let aug = augmentation::get(&cat, &shadow_id).unwrap().unwrap();
        assert!(aug.params.contains(r#""F-1""#));
        // lineage link
        let out = links::outgoing(&cat, &shadow_id).unwrap();
        assert!(out.iter().any(|l| l.rel == "worktree_of" && l.dst_id == main_id));
        // fork event with base snapshot
        let ev = events::latest_for_artifact(&cat, &shadow_id).unwrap().unwrap();
        assert_eq!(ev.kind, "worktree_fork");
        assert!(ev.payload.contains(r#""main_id""#) && ev.payload.contains(r#""base_params""#));
        // durable registration
        assert!(reg::get(&cat, "/repo/.worktrees/feat").unwrap().unwrap().status == "active");
    }

    #[test]
    fn fork_is_idempotent() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = { let cat = ctx.catalog.lock(); seed_main_tracker(&cat) };
        let a = { let mut c = ctx.catalog.lock(); resolve_write_target(&mut c, &ctx, &main_id).unwrap() };
        let b = { let mut c = ctx.catalog.lock(); resolve_write_target(&mut c, &ctx, &main_id).unwrap() };
        assert_eq!(a, b);
        let cat = ctx.catalog.lock();
        // exactly one fork event
        let n: i64 = cat.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE artifact_id=?1 AND kind='worktree_fork'",
            [&a], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn passthrough_for_non_worktree_session_and_foreign_targets() {
        // Non-worktree session: id unchanged.
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: "/repo".into(), git_root: "/repo".into(),
                main_root: None, umbrella: None,
            })).build();
        let main_id = { let cat = ctx.catalog.lock(); seed_main_tracker(&cat) };
        let got = { let mut c = ctx.catalog.lock(); resolve_write_target(&mut c, &ctx, &main_id).unwrap() };
        assert_eq!(got, main_id);

        // Worktree session, target OUTSIDE main_root (umbrella peer): unchanged.
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let peer_id = {
            let cat = ctx.catalog.lock();
            let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/other/doc.md"));
            artifact::upsert(&cat, &TestArtifactRowBuilder::new(&id).with_abs_path("/other/doc.md").build()).unwrap();
            id
        };
        let got = { let mut c = ctx.catalog.lock(); resolve_write_target(&mut c, &ctx, &peer_id).unwrap() };
        assert_eq!(got, peer_id);
    }
}
```

(If `AugmentationRow` lacks `Default`, construct it fully — fields at `src/librarian/catalog/augmentation.rs:7-32` — or add `#[derive(Default)]`-compatible builder usage per existing tests at `augmentation.rs:416-432`.)

- [ ] **Step 2: Run to verify failure** — `cargo test --lib librarian::tools::worktree` → compile error.

- [ ] **Step 3: Implement**

```rust
//! Fork-on-first-write for the worktree overlay. See
//! docs/superpowers/specs/2026-07-17-worktree-overlay-design.md §3.
use anyhow::Result;
use serde_json::json;

use super::ToolContext;
use crate::librarian::catalog::{artifact, augmentation, events, links, worktree as reg, Catalog};
use crate::librarian::current_project::CurrentProject;
use crate::librarian::ids;
use crate::util::fs::RepoPath;

pub(crate) const FORK_EVENT_KIND: &str = "worktree_fork";
pub(crate) const LINEAGE_REL: &str = "worktree_of";

fn under(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

/// Best-effort branch name: worktree `.git` file → gitdir → `<gitdir>/HEAD`
/// → `ref: refs/heads/<branch>`. Filesystem-only, like current_project.rs.
fn read_branch(worktree_root: &std::path::Path) -> Option<String> {
    let gitfile = std::fs::read_to_string(worktree_root.join(".git")).ok()?;
    let gitdir = gitfile.strip_prefix("gitdir:")?.trim();
    let head = std::fs::read_to_string(std::path::Path::new(gitdir).join("HEAD")).ok()?;
    head.trim().strip_prefix("ref: refs/heads/").map(String::from)
}

/// Upsert the durable registration for a worktree session. No-op otherwise.
pub(crate) fn ensure_registration(cat: &Catalog, cp: &CurrentProject) -> Result<()> {
    let Some(main_root) = cp.main_root.as_ref() else {
        return Ok(());
    };
    reg::upsert_active(
        cat,
        &RepoPath::from(cp.git_root.as_path()).into_string(),
        &RepoPath::from(main_root.as_path()).into_string(),
        read_branch(&cp.git_root).as_deref(),
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The overlay write gate. A mutating action targeting a MAIN-root artifact
/// from a worktree session forks it: shadow row at the worktree path (seeded
/// from the main row), `worktree_fork` event carrying the base snapshot,
/// `worktree_of` lineage link, durable registration. Returns the id the write
/// must proceed against. Everything else passes through unchanged.
///
/// NOTE (F-2, session log): the shadow's params are a base COPY + the
/// worktree's delta. merge_worktree extracts the delta against the fork
/// event's base — never bare-graft a seeded shadow.
pub(crate) fn resolve_write_target(cat: &mut Catalog, ctx: &ToolContext, id: &str) -> Result<String> {
    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok(id.to_string());
    };
    let Some(main_root) = cp.main_root.as_ref() else {
        return Ok(id.to_string());
    };
    let Some(row) = artifact::get(cat, id)? else {
        return Ok(id.to_string()); // unknown id: let the caller produce its own error
    };
    let main_s = RepoPath::from(main_root.as_path()).into_string();
    let wt_s = RepoPath::from(cp.git_root.as_path()).into_string();
    let row_path = RepoPath::from(row.abs_path.as_path()).into_string();
    if under(&row_path, &wt_s) || !under(&row_path, &main_s) {
        return Ok(id.to_string()); // already shadow, or foreign repo — no isolation (spec non-goal)
    }

    let rel = row_path
        .strip_prefix(&format!("{main_s}/"))
        .unwrap_or(&row_path);
    let shadow_path = format!("{wt_s}/{rel}");
    let shadow_id = ids::artifact_id_from_abs(std::path::Path::new(&shadow_path));
    if artifact::get(cat, &shadow_id)?.is_some() {
        return Ok(shadow_id); // already forked
    }

    let now = chrono::Utc::now().timestamp_millis();
    let base_aug = augmentation::get(cat, id)?;
    let tx = cat.conn.unchecked_transaction()?;
    ensure_registration(cat, cp)?;
    // Seed the shadow from the main row. The FILE at the shadow path is git's
    // checkout copy — identical at fork — so file_mtime/file_sha256 carry over.
    let shadow_row = crate::librarian::catalog::ArtifactRow {
        id: shadow_id.clone(),
        abs_path: std::path::PathBuf::from(&shadow_path),
        created_at: now,
        updated_at: now,
        ..row.clone()
    };
    artifact::upsert(cat, &shadow_row)?;
    if let Some(aug) = &base_aug {
        let mut shadow_aug = aug.clone();
        shadow_aug.artifact_id = shadow_id.clone();
        augmentation::upsert(cat, &shadow_aug)?;
    }
    events::insert(
        cat,
        &crate::librarian::catalog::EventRow {
            id: ulid::Ulid::new().to_string(),
            artifact_id: shadow_id.clone(),
            kind: FORK_EVENT_KIND.into(),
            payload: json!({
                "main_id": id,
                "branch": read_branch(&cp.git_root),
                "base_params": base_aug.as_ref()
                    .and_then(|a| serde_json::from_str::<serde_json::Value>(&a.params).ok()),
                "base_frontmatter": {
                    "status": row.status, "title": row.title, "tags": row.tags,
                    "topic": row.topic, "time_scope": row.time_scope, "owners": row.owners,
                },
            })
            .to_string(),
            anchor_commit: None,
            head_commit: None,
            author: Some("worktree-overlay".into()),
            created_at: now,
        },
    )?;
    links::insert(
        cat,
        &crate::librarian::catalog::LinkRow {
            src_id: shadow_id.clone(),
            dst_id: id.to_string(),
            rel: LINEAGE_REL.into(),
            created_at: now,
        },
    )?;
    tx.commit()?;
    Ok(shadow_id)
}
```

Adjust `events::insert` / `links::insert` argument shapes to their real signatures (`src/librarian/catalog/events.rs:68`, `links.rs:15`) and re-export names (`ArtifactRow`/`EventRow`/`LinkRow`/`AugmentationRow` paths) as the compiler directs — semantics stay exactly as above. If `ArtifactRow` is not `Clone`, derive it.

- [ ] **Step 4: Run** — `cargo test --lib librarian::tools::worktree` → 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/worktree.rs src/librarian/tools/mod.rs src/librarian/catalog
git commit -m "feat(librarian): fork-on-first-write — shadow rows with recorded lineage + base snapshot

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: wire the write gate into every mutating handler

**Files:**
- Modify: `src/librarian/tools/append_entry.rs:20-31`, `update.rs:201`, `event_create.rs:214-215`, `augment.rs` (its `call`), `refresh.rs:26-27`, `link.rs:14-17`, `delete.rs:23-25`, `mv.rs:15-18`, `create.rs` (its `call`, after the row lands)

**Interfaces:**
- Consumes: `worktree::resolve_write_target`, `worktree::ensure_registration` (Task 4).
- Behavior contract: `append_entry`/`update`/`event_create` (field `artifact_id`)/`augment`/`refresh` redirect to the shadow id; `link` redirects `src_id` only (spec open question 3 → v1 decision); `delete`/`mv` refuse main-root targets from worktree sessions; `create` ensures registration.

- [ ] **Step 1: Write failing integration-style tests** (new `tests` additions in `append_entry.rs` and `delete.rs`, reusing Task 4's `wt_ctx`/`seed_main_tracker` idiom — move those helpers to `worktree.rs` `pub(crate) mod test_support` so both files use them):

```rust
// in append_entry.rs tests
#[tokio::test]
async fn append_from_worktree_lands_on_shadow_not_main() {
    let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); crate::librarian::tools::worktree::test_support::seed_main_tracker(&c) };
    let out = call(&ctx, serde_json::json!({
        "id": main_id, "entry_collection": "items", "id_prefix": "F",
        "entry": {"t": "from-worktree"}
    })).await.unwrap();
    assert_eq!(out["id"], "F-2"); // base had F-1
    let c = ctx.catalog.lock();
    let main_aug = augmentation::get(&c, &main_id).unwrap().unwrap();
    assert!(!main_aug.params.contains("from-worktree"), "main untouched");
}

// in delete.rs tests
#[tokio::test]
async fn delete_of_main_artifact_from_worktree_is_refused() {
    let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); crate::librarian::tools::worktree::test_support::seed_main_tracker(&c) };
    let err = call(&ctx, serde_json::json!({"id": main_id})).await.unwrap_err();
    assert!(err.to_string().contains("worktree"), "refusal names the worktree overlay: {err}");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test --lib librarian::tools::append_entry librarian::tools::delete`.

- [ ] **Step 3: Implement the wiring.** Pattern A (redirect) — `append_entry.rs`:

```rust
let a: Args = serde_json::from_value(args)?;
if !a.entry.is_object() { /* unchanged guard */ }
let mut cat = ctx.catalog.lock();
let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
let id = augmentation::append_entry(&mut cat, &target, &a.entry_collection, &a.id_prefix, a.entry)?;
Ok(json!({"id": id, "artifact_id": target}))
```

`update.rs` (before `let cat = ctx.catalog.lock();` at line 202):

```rust
let a = {
    let mut cat = ctx.catalog.lock();
    let id = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    Args { id, ..a }
};
```

Same rebind in `event_create.rs` (`Args { artifact_id, ..a }`), `augment.rs`, `refresh.rs`. In `link.rs`, rebind `src_id` only. Pattern B (refuse) — `delete.rs` and `mv.rs`, after loading the row:

```rust
if let Some(cp) = ctx.current_project.as_deref() {
    if let Some(main_root) = cp.main_root.as_ref() {
        let main_s = crate::util::fs::RepoPath::from(main_root.as_path()).into_string();
        let row_path = crate::util::fs::RepoPath::from(row.abs_path.as_path()).into_string();
        if row_path == main_s || row_path.starts_with(&format!("{main_s}/")) {
            return Err(super::RecoverableError::new(
                "refused from a worktree session: this artifact belongs to the main checkout. \
                 Merge the worktree (librarian action=\"merge_worktree\") or run this from the main checkout.",
            ));
        }
    }
}
```

Pattern C — `create.rs`, after the artifact row is written:

```rust
if let Some(cp) = ctx.current_project.as_deref() {
    super::worktree::ensure_registration(&cat, cp)?;
}
```

- [ ] **Step 4: Run** — `cargo test --lib librarian::tools` → all pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools
git commit -m "feat(librarian): route all mutating artifact actions through the overlay write gate

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: overlay reads — find dedup + get overlay_hint

**Files:**
- Modify: `src/librarian/tools/find.rs` (inside the `let (items, hints, catalog_value) = { ... }` block, after `rows` is built, ~line 493)
- Modify: `src/librarian/tools/get.rs` (after the row load in `call`, ~line 88 onward)

**Interfaces:**
- Consumes: `worktree_of` links (Task 4).
- Produces: `find` items may carry `"overlay": true`; `get` response may carry `"overlay_hint": {"shadow_id": ...}`.

- [ ] **Step 1: Write failing test** in `find.rs` tests:

```rust
#[tokio::test]
async fn worktree_find_shadows_main_twin_and_flags_overlay() {
    use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};
    let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); seed_main_tracker(&c) };
    let shadow_id = {
        let mut c = ctx.catalog.lock();
        crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap()
    };
    let out = call(&ctx, serde_json::json!({"scope": "repo"})).await.unwrap();
    let ids: Vec<&str> = out["items"].as_array().unwrap().iter()
        .map(|i| i["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&shadow_id.as_str()), "shadow visible: {ids:?}");
    assert!(!ids.contains(&main_id.as_str()), "main twin suppressed: {ids:?}");
    let shadow_item = out["items"].as_array().unwrap().iter()
        .find(|i| i["id"] == shadow_id.as_str()).unwrap();
    assert_eq!(shadow_item["overlay"], true);
}
```

(The `wt_ctx` catalog has no workspace root for `/repo` — if `apply_scope`/`find` needs one, extend `test_support::wt_ctx` with `.with_root(Root { ... "/repo" ... })` per the `scope_all_widens_to_workspace` test idiom at `find.rs:729`.)

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement in `find.rs`** (inside the lock block, right after `let rows = ...`):

```rust
// Overlay dedup: a worktree session sees its shadow INSTEAD of the main twin.
let mut overlay_ids: std::collections::HashSet<String> = Default::default();
let mut rows = rows;
if let Some(cp) = current.filter(|c| c.main_root.is_some()) {
    let wt = crate::util::fs::RepoPath::from(cp.git_root.as_path()).into_string();
    let mut stmt = cat.conn.prepare(
        "SELECT l.src_id, l.dst_id FROM artifact_link l \
         JOIN artifact s ON s.id = l.src_id \
         WHERE l.rel = 'worktree_of' AND (s.abs_path = ?1 OR s.abs_path LIKE ?1 || '/%')",
    )?;
    let pairs: Vec<(String, String)> = stmt
        .query_map([&wt], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let shadowed: std::collections::HashSet<&str> =
        pairs.iter().map(|(_, d)| d.as_str()).collect();
    rows.retain(|r| !shadowed.contains(r.id.as_str()));
    overlay_ids = pairs.into_iter().map(|(s, _)| s).collect();
}
```

and in the `items` mapping add the flag:

```rust
let mut item = json!({ /* existing fields unchanged */ });
if overlay_ids.contains(&r.id) {
    item["overlay"] = json!(true);
}
item
```

**In `get.rs`**, after the requested row is loaded (only when it was found and the session is a worktree):

```rust
if let Some(cp) = ctx.current_project.as_deref().filter(|c| c.main_root.is_some()) {
    let wt = crate::util::fs::RepoPath::from(cp.git_root.as_path()).into_string();
    let shadow: Option<String> = cat.conn.query_row(
        "SELECT l.src_id FROM artifact_link l JOIN artifact s ON s.id = l.src_id \
         WHERE l.rel='worktree_of' AND l.dst_id = ?1 \
           AND (s.abs_path = ?2 OR s.abs_path LIKE ?2 || '/%')",
        rusqlite::params![a.id, wt],
        |r| r.get(0),
    ).optional()?;
    if let Some(sid) = shadow {
        response["overlay_hint"] = serde_json::json!({
            "shadow_id": sid,
            "hint": "This session has forked this artifact; reads of the worktree state and all writes use the shadow id.",
        });
    }
}
```

(Adapt variable names to `get.rs`'s actual response-building local; the response JSON object exists before `Ok(...)`.)

- [ ] **Step 4: Run** — `cargo test --lib librarian::tools::find librarian::tools::get` → pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/find.rs src/librarian/tools/get.rs
git commit -m "feat(librarian): overlay reads — shadow-wins dedup in find, overlay_hint in get

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: graft refactor — extract `fold_entries` + `repoint_history`

**Files:**
- Modify: `src/librarian/catalog/graft.rs` (`graft_rows` lines 51–143, `merge_augmentation` lines 179–320)

Pure refactor: behavior identical, existing tests stay green. This is the F-2 mitigation's foundation — merge_worktree (Task 8) must fold a *delta*, not a whole seeded collection, so the fold core becomes callable on an arbitrary incoming slice.

**Interfaces:**
- Produces: `pub(crate) fn fold_entries(into_arr: &[Value], incoming: &[Value], report: &mut GraftReport) -> Vec<Value>` — exactly the current reserved-universe/collision-renumber/near-dup logic operating on arrays.
- Produces: `pub(crate) fn repoint_history(tx: &rusqlite::Transaction<'_>, from_id: &str, into_id: &str, report: &mut GraftReport) -> Result<()>` — the events/observations/links/event_edges UPDATE statements currently inline in `graft_rows`.

- [ ] **Step 1: Extract `fold_entries`.** Move the block of `merge_augmentation` from the `into_ids` HashSet construction through the renumber loop (graft.rs ~lines 245–305) into the new function; `merge_augmentation` calls it with `(&into_arr, &from_arr, report)` and writes the returned `merged` array back. Signature above; no behavior change.

- [ ] **Step 2: Extract `repoint_history`.** Move `graft_rows`'s re-point UPDATEs (events, artifact_observation, artifact_link with collision-drop, event_edges with collision-drop — everything between the row-existence guards and `merge_augmentation`) into the new function taking the open transaction; `graft_rows` calls it.

- [ ] **Step 3: Run the full graft suite**

Run: `cargo test --lib librarian::catalog::graft`
Expected: all 10 existing tests pass unchanged (`graft_renumbers_colliding_incoming_ids_and_reports_remap`, `graft_renumber_avoids_free_incoming_id_no_duplicate`, `graft_flags_near_dup_as_suspicious`, ...)

- [ ] **Step 4: Commit**

```bash
git add src/librarian/catalog/graft.rs
git commit -m "refactor(librarian): extract fold_entries + repoint_history from graft for merge_worktree reuse

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: `librarian(action="merge_worktree")`

**Files:**
- Create: `src/librarian/tools/merge_worktree.rs`
- Modify: `src/librarian/tools/mod.rs` (add `pub mod merge_worktree;`)
- Modify: `src/librarian/tools/librarian.rs` (dispatch at lines 103–114, action enum + description in `input_schema`/`description`)

**Interfaces:**
- Consumes: Tasks 1, 4, 7 (`fold_entries`, `repoint_history`, fork event payload shape, registration CRUD).
- Args: `{ root: String, dry_run?: bool, abandon?: bool }`.
- Produces response: `{ merged: [...], reseated: [...], conflicts: [...], remap: {...}, suspicious: [...], registration: "merged"|"abandoned"|"active(dry_run)" }`.
- Produces: event kind `"worktree_merge"` on each merged main artifact.

- [ ] **Step 1: Write failing tests** (in `merge_worktree.rs`, using Task 4's `test_support`):

```rust
#[tokio::test]
async fn merge_folds_delta_without_duplicating_base_entries() {
    // F-2 regression — THE invariant test of this feature.
    let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); seed_main_tracker(&c) }; // base: items=[F-1]
    // worktree appends F-2 (via the write gate → fork + shadow append)
    let shadow_id = {
        let mut c = ctx.catalog.lock();
        let sid = crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap();
        augmentation::append_entry(&mut c, &sid, "items", "F", serde_json::json!({"t":"wt"})).unwrap();
        sid
    };
    // main concurrently appends its own F-2
    {
        let mut c = ctx.catalog.lock();
        augmentation::append_entry(&mut c, &main_id, "items", "F", serde_json::json!({"t":"main"})).unwrap();
    }
    let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"})).await.unwrap();
    let c = ctx.catalog.lock();
    let params: serde_json::Value =
        serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params).unwrap();
    let ids: Vec<&str> = params["items"].as_array().unwrap().iter()
        .map(|e| e["id"].as_str().unwrap()).collect();
    // base F-1 exactly once; main's F-2 kept; worktree's F-2 renumbered to F-3.
    assert_eq!(ids, vec!["F-1", "F-2", "F-3"], "no duplicates, deterministic renumber: {ids:?}");
    assert_eq!(out["remap"][&format!("{shadow_id}:items:F-2")], "F-3");
    // shadow row gone, its events re-pointed under main
    assert!(artifact::get(&c, &shadow_id).unwrap().is_none());
    let n: i64 = c.conn.query_row(
        "SELECT COUNT(*) FROM events WHERE artifact_id=?1 AND kind='worktree_fork'",
        [&main_id], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "fork event preserved as audit trail under main id");
    // registration closed
    assert_eq!(reg::get(&c, "/repo/.worktrees/feat").unwrap().unwrap().status, "merged");
}

#[tokio::test]
async fn merge_three_ways_scalars_and_reports_conflicts() {
    let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); seed_main_tracker(&c) }; // note:"base"
    let _sid = {
        let mut c = ctx.catalog.lock();
        let sid = crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap();
        // worktree edits scalar `note`
        augmentation::merge_params(&c, &sid, &serde_json::json!({"note": "wt-edit"})).unwrap();
        sid
    };
    // main ALSO edits `note` → both-changed → conflict, main value survives
    { let c = ctx.catalog.lock();
      augmentation::merge_params(&c, &main_id, &serde_json::json!({"note": "main-edit"})).unwrap(); }
    let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"})).await.unwrap();
    let c = ctx.catalog.lock();
    let params: serde_json::Value =
        serde_json::from_str(&augmentation::get(&c, &main_id).unwrap().unwrap().params).unwrap();
    assert_eq!(params["note"], "main-edit", "conflicted field keeps main value");
    let conflicts = out["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["key"], "note");
    assert_eq!(conflicts[0]["worktree"], "wt-edit");
}

#[tokio::test]
async fn new_worktree_artifact_reseats_to_main_path() {
    let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
    { let c = ctx.catalog.lock(); seed_main_tracker(&c); }
    // artifact born in the worktree (no lineage edge)
    let wt_born = {
        let c = ctx.catalog.lock();
        let id = crate::librarian::ids::artifact_id_from_abs(
            std::path::Path::new("/repo/.worktrees/feat/docs/new.md"));
        artifact::upsert(&c, &TestArtifactRowBuilder::new(&id)
            .with_abs_path("/repo/.worktrees/feat/docs/new.md").build()).unwrap();
        crate::librarian::tools::worktree::ensure_registration(&c,
            ctx.current_project.as_deref().unwrap()).unwrap();
        id
    };
    let out = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat"})).await.unwrap();
    let c = ctx.catalog.lock();
    let main_new = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/repo/docs/new.md"));
    assert!(artifact::get(&c, &main_new).unwrap().is_some(), "reseated at main path");
    assert!(artifact::get(&c, &wt_born).unwrap().is_none(), "worktree row gone");
    assert!(out["reseated"].as_array().unwrap().len() == 1);
}

#[tokio::test]
async fn dry_run_writes_nothing_and_abandon_sweeps() {
    let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
    let main_id = { let c = ctx.catalog.lock(); seed_main_tracker(&c) };
    let sid = { let mut c = ctx.catalog.lock();
        crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap() };
    let _ = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat", "dry_run": true})).await.unwrap();
    { let c = ctx.catalog.lock();
      assert!(artifact::get(&c, &sid).unwrap().is_some(), "dry_run left shadow intact");
      assert_eq!(reg::get(&c, "/repo/.worktrees/feat").unwrap().unwrap().status, "active"); }
    let _ = call(&ctx, serde_json::json!({"root": "/repo/.worktrees/feat", "abandon": true})).await.unwrap();
    let c = ctx.catalog.lock();
    assert!(artifact::get(&c, &sid).unwrap().is_none(), "abandon removed shadow");
    assert_eq!(reg::get(&c, "/repo/.worktrees/feat").unwrap().unwrap().status, "abandoned");
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement `merge_worktree.rs`.** Skeleton with the full per-artifact algorithm:

```rust
//! First-class worktree merge: fold every shadow row's DELTA (vs the fork
//! event's base snapshot) onto its main twin, reseat lineage-less rows, close
//! the registration. Per-artifact IMMEDIATE transaction (graft's atomicity
//! contract). NEVER bare-grafts a seeded shadow (F-2, session log).
use anyhow::Result;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::{
    artifact, augmentation, events, graft, worktree as reg, Catalog, GraftReport,
};
use crate::librarian::ids;

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
    let root = crate::util::fs::RepoPath::from(std::path::Path::new(&a.root)).into_string();
    let mut cat = ctx.catalog.lock();
    let Some(registration) = reg::get(&cat, &root)? else {
        return Err(RecoverableError::with_hint(
            format!("no worktree registration for `{root}`"),
            "Unregistered legacy rows: use librarian(action=\"doctor\") + fix=reseat_worktree / artifact(action=\"graft\") instead.",
        ));
    };
    if registration.status != "active" {
        return Err(RecoverableError::new(format!(
            "registration for `{root}` is `{}` — nothing to merge", registration.status
        )));
    }
    let now = chrono::Utc::now().timestamp_millis();

    // Enumerate shadow rows under the root.
    let shadow_ids: Vec<String> = {
        let mut stmt = cat.conn.prepare(
            "SELECT id FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?1 || '/%' ORDER BY abs_path")?;
        stmt.query_map([&root], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };

    if a.abandon {
        if !a.dry_run {
            cat.conn.execute(
                "DELETE FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?1 || '/%'", [&root])?;
            reg::set_status(&cat, &root, "abandoned", now)?;
        }
        return Ok(json!({"abandoned": shadow_ids.len(), "registration": "abandoned", "dry_run": a.dry_run}));
    }

    check_rebase_invariant(&registration)?; // skips silently when the worktree dir is gone

    let mut merged = Vec::new();
    let mut reseated = Vec::new();
    let mut conflicts = Vec::new();
    let mut remap = serde_json::Map::new();
    let mut suspicious = Vec::new();

    for sid in &shadow_ids {
        let lineage: Option<String> = cat.conn.query_row(
            "SELECT dst_id FROM artifact_link WHERE src_id=?1 AND rel='worktree_of'",
            [sid], |r| r.get(0)).optional()?;
        match lineage {
            Some(main_id) => merge_one(
                &mut cat, ctx, sid, &main_id, a.dry_run,
                &mut conflicts, &mut remap, &mut suspicious, now,
            ).map(|_| merged.push(json!({"shadow": sid, "into": main_id})))?,
            None => reseat_one(&mut cat, sid, &root, &registration.main_root, a.dry_run)
                .map(|main_id| reseated.push(json!({"from": sid, "to": main_id})))?,
        }
    }
    if !a.dry_run {
        reg::set_status(&cat, &root, "merged", now)?;
    }
    Ok(json!({
        "merged": merged, "reseated": reseated, "conflicts": conflicts,
        "remap": remap, "suspicious": suspicious,
        "registration": if a.dry_run { "active(dry_run)" } else { "merged" },
        "dry_run": a.dry_run,
        "hint": "Rewrite live-tree citations for any remapped entry ids (see remap).",
    }))
}
```

`merge_one` — the delta algorithm (full logic; per-artifact tx via `cat.conn.unchecked_transaction()`):

1. Load the shadow's `worktree_fork` event (latest by `created_at` with that kind); parse `base_params` + `base_frontmatter` from payload. Missing fork event → push to `conflicts` with `"kind": "missing_fork_event"` and skip (never guess — that's the legacy-doctor path).
2. Load shadow + main augmentation params as `serde_json::Value` objects, and both `ArtifactRow`s.
3. Split params by the augmentation's `entry_collection` (`coll`):
   - `appended` = shadow `coll` entries whose `id` ∉ base `coll` id-set.
   - `edited_base` = shadow entries with `id` ∈ base set but content ≠ base content (compare via `graft::strip_id`-style clone-minus-id, plus id equality).
   - scalar keys = every top-level key except `coll`.
4. Fold `appended` with `graft::fold_entries(&main_arr, &appended, &mut report)`; record `report.remap` entries into the response `remap` as `"{sid}:{coll}:{old}" -> new`, extend `suspicious`.
5. `edited_base` and scalars three-way: `main == base → apply shadow value; else → conflicts.push({artifact: main_id, key, base, main, worktree})` (main value kept).
6. Frontmatter three-way over `status/title/tags/topic/time_scope/owners` from `base_frontmatter` vs main row vs shadow row; apply via `artifact::upsert` of the amended main row.
7. Write merged params: `UPDATE artifact_augmentation SET params=?1 WHERE artifact_id=?2` (main).
8. `graft::repoint_history(&tx, sid, main_id, &mut report)` — moves ALL shadow events (fork event included — audit trail), observations, non-lineage links, event_edges; then delete the `worktree_of` link row explicitly before re-point would collide it (or let the collision-drop discard it — assert one or the other in the test).
9. Insert `worktree_merge` event on `main_id` with payload `{branch, remap, conflicts: <this artifact's>, entries_merged, entries_renumbered}`.
10. `DELETE FROM artifact WHERE id = ?sid` (cascades augmentation). Commit.
11. `dry_run`: run steps 1–5 into the report, skip every write (guard each write on `!dry_run`; simplest: compute-only path that never opens the tx).

`reseat_one`: compute `main_path = main_root + rel(root, shadow_path)`, `id_m = ids::artifact_id_from_abs(main_path)`; if a row exists at `id_m` → push a `"kind": "reseat_collision"` conflict and skip; else seed the row at the main path (same field-copy as doctor's reseat, `doctor.rs:215-266`) and `graft::graft_rows(cat, sid, &id_m)` — safe here precisely because a lineage-less worktree row was never base-seeded (its history is 100% worktree-born).

`check_rebase_invariant`: if `std::path::Path::new(&registration.worktree_root).exists()` and a branch is recorded — run `git -C <main_root> merge-base --is-ancestor <branch> HEAD` OR the reverse; if **neither** direction holds, return `RecoverableError` advising `git rebase` first. Any git failure (no git, detached, branch gone) → skip the check (DB state is self-sufficient); never block on git absence.

- [ ] **Step 4: Wire dispatch in `librarian.rs`** — add to the match (line 103) `"merge_worktree" => super::merge_worktree::call(ctx, args).await,`, extend the action `enum` list + error strings + `description()` text: `"merge_worktree: fold a registered worktree's shadow artifacts onto their main-checkout twins (delta vs recorded base; dry_run / abandon)."` and add `root`/`dry_run`/`abandon` are already generic (`root` exists for doctor; extend its description to mention merge_worktree).

- [ ] **Step 5: Run** — `cargo test --lib librarian::tools::merge_worktree librarian::catalog::graft` → all pass.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/merge_worktree.rs src/librarian/tools/mod.rs src/librarian/tools/librarian.rs
git commit -m "feat(librarian): merge_worktree — delta-fold shadow rows onto main, reseat new rows, close registration

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: doctor reclassification, prune guard, docs + spec sync

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (`scan_worktree_scoped` lines 438–491, `validate_prune_request` lines 148–171, `run_fix`/`reseat_worktree` skip-registered, module docs lines 50–58, description strings in `librarian.rs`)
- Modify: `src/prompts/guides/librarian.md` (new "Worktree overlay" subsection: overlay reads, write gate, merge_worktree, doctor-as-legacy-fallback)
- Modify: `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md` (§Data model: drop `base_event_seq`, note "all shadow events are post-fork by construction")
- Modify: `docs/trackers/worktree-overlay-session-log.md` (flip F-1/F-2 to `fixed-verified` once the F-2 regression test passes)

- [ ] **Step 1: Write failing doctor tests:**

```rust
#[test]
fn worktree_scoped_row_marks_registered_rows_pending_merge() {
    // build a fake worktree row (existing fixture idiom in doctor tests) + an
    // ACTIVE registration covering it; scan must set detail.registered = true
    // and classification unchanged; reseat_worktree must SKIP it.
}

#[test]
fn prune_missing_refuses_root_with_active_registration() {
    // upsert_active for a root that no longer exists on disk; expect
    // RecoverableError mentioning merge_worktree/abandon.
}
```

(Flesh out from the existing reseat test fixture in `doctor.rs` — the test asserting the `wt-row` graft around line 1054 builds exactly the worktree `.git`-file + catalog-row layout these two tests need; copy its setup, then add the registration row via `worktree::upsert_active`.)

- [ ] **Step 2: Implement.**
- `scan_worktree_scoped`: after `worktree_root` resolves, query `SELECT 1 FROM worktree_registration WHERE status='active' AND (?1 = worktree_root OR ?1 LIKE worktree_root || '/%')` with the row's worktree root; set `detail["registered"] = json!(true/false)`. Registered rows: `detail["hint"] = "pending merge — use librarian(action=\"merge_worktree\")"`.
- `reseat_worktree` fix: skip violations with `registered == true` (report them as skipped).
- `validate_prune_request`: before accepting a dead root, `worktree::covering`-style check; refuse with hint naming `merge_worktree` / `abandon=true`.

- [ ] **Step 3: Docs.**
- `src/prompts/guides/librarian.md`: add the overlay subsection (concise — mechanics + one merge example call).
- Spec §Data model / §Merge: apply the `base_event_seq` deviation note.
- Session log: F-1 → `fixed-verified` (fork event carries the cursor; landed Task 4), F-2 → `fixed-verified` (delta fold + regression test; landed Task 8), both with commit SHAs.
- Update memory: `memory(action="write", topic="worktree-merge-catalog-reconciliation", ...)` — overlay flow is now primary (fork-on-first-write + merge_worktree); doctor reseat/graft is the fallback for unregistered legacy rows only.

- [ ] **Step 4: Full gates**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean. Watch specifically: `server::tests::prompt_surfaces_reference_only_real_tools` (guide edits touch a prompt surface) and the doctor suite.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(librarian): doctor overlay-awareness + prune guard; docs + spec sync for worktree overlay

Closes F-1, F-2 (docs/trackers/worktree-overlay-session-log.md).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Verification checklist (post-plan, before ship)

- [ ] `cargo rb` + `/mcp` reconnect, then live smoke: create a real worktree, `append_entry` against a main tracker id from a session in it, verify main untouched, `merge_worktree(dry_run=true)`, then merge, then `librarian(action="doctor")` clean.
- [ ] `git worktree remove` BEFORE merge variant of the smoke test — merge must still succeed from registration + shadows alone.
- [ ] Memory + session-log statuses updated (Task 9 Step 3).
