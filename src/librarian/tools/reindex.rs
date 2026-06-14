use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::librarian::indexer;

use super::ToolContext;

#[derive(Deserialize)]
struct Args {
    repo: Option<String>,
    /// When true, the upsert walk ignores cached file hashes and re-processes
    /// every file (re-classification + re-embedding). Plumbed through
    /// `index_repo_sync` as `force_rewalk` (task #31). Default false — files
    /// matching their stored hash are skipped via the early-return path.
    ///
    /// Historically this also issued a destructive pre-walk DELETE that
    /// cascade-removed augmentations on subsequent failure (bug-tracker #7).
    /// That DELETE was removed in commit `d482ca8a`; force is now a safe
    /// hash-cache-bypass with no destructive side-effect.
    force: Option<bool>,
    /// Scope of the reindex. Defaults to `project` when a current project is
    /// resolved, else `all`. Mirrors the read-tool scope semantics.
    scope: Option<super::scope::Scope>,
}

fn backfill_commits(
    catalog: &crate::librarian::catalog::Catalog,
    repo_path: &std::path::Path,
) -> anyhow::Result<()> {
    use git2::{Repository, Sort};

    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("skipping {}: not a git repo ({})", repo_path.display(), e);
            return Ok(());
        }
    };
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    if let Err(e) = walk.push_head() {
        tracing::debug!(
            "revwalk push_head failed for {}: {}",
            repo_path.display(),
            e
        );
        return Ok(());
    }

    let git_root_str = crate::util::fs::RepoPath::from(repo_path).into_string();
    let rows: anyhow::Result<Vec<_>> = walk
        .enumerate()
        .map(|(order, oid_result)| {
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            Ok(crate::librarian::catalog::commits::CommitRow {
                hash: oid.to_string(),
                git_root: git_root_str.clone(),
                authored_at: Some(commit.time().seconds() * 1000),
                subject: commit.summary().map(String::from),
                topo_order: Some(order as i64),
            })
        })
        .collect();
    crate::librarian::catalog::commits::upsert_many(catalog, &rows?)?;
    Ok(())
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    use super::scope::Scope;

    let a: Args = serde_json::from_value(args)?;

    let effective_scope = a.scope.unwrap_or_else(|| {
        if ctx.current_project.is_some() {
            Scope::Project
        } else {
            Scope::All
        }
    });

    // targets: abs_root paths to walk.
    let targets: Vec<std::path::PathBuf> = match effective_scope {
        Scope::All => {
            if let Some(ref repo_name) = a.repo {
                let root = ctx
                    .workspace
                    .roots
                    .iter()
                    .find(|r| &r.name == repo_name)
                    .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", repo_name))?;
                vec![root.path.clone()]
            } else {
                ctx.workspace.roots.iter().map(|r| r.path.clone()).collect()
            }
        }
        Scope::Repo => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=repo requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            vec![cp.git_root.clone()]
        }
        Scope::Project => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=project requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            vec![cp.abs_path.clone()]
        }
        Scope::Umbrella => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!("scope=umbrella requires a resolved current project")
            })?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella declared for {}",
                    cp.abs_path.display(),
                )
            })?;
            let umbrella = ctx
                .workspace
                .umbrellas
                .iter()
                .find(|u| u.name == umbrella_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("umbrella `{umbrella_name}` not found in workspace config")
                })?;
            umbrella.members.clone()
        }
    };

    // NOTE: previously, `force=true` issued
    // `DELETE FROM artifact WHERE abs_path LIKE <root>/%` here, *before* the
    // re-walk. That was destructive: `artifact_augmentation` is declared
    // `ON DELETE CASCADE` (catalog/schema.sql), so the DELETE cascade-wiped
    // augmentations. When the subsequent embedding INSERT failed (e.g.
    // dim mismatch — bug-tracker #6), the DELETE was already committed.
    // Removed 2026-05-17 per bug-tracker #7 (F-9 in
    // docs/trackers/archive/artifact-code-linkage-session-log.md). `force=true`
    // now means "ignore cached file hashes during the upsert walk"; the
    // walk's own deletion logic still removes rows for files no longer
    // on disk (the `removed` count in the response).

    let mut orphan_removed = 0usize;
    if effective_scope == Scope::All && a.repo.is_none() {
        let cat = ctx.catalog.lock();
        let active: Vec<&std::path::Path> = ctx
            .workspace
            .roots
            .iter()
            .map(|r| r.path.as_path())
            .collect();
        // Bound the orphan sweep to THIS workspace's own roots (scope == the
        // walked roots): the catalog is a single machine-global DB, so an
        // unbounded "delete rows not under the active roots" would wipe other
        // workspaces' rows (3ea49090). Within-workspace file deletions are
        // already handled by the per-file walk above; pruning a de-registered
        // root or a renamed repo is the job of an explicit scoped prune
        // (7ca71bf7), not this reindex side-effect.
        orphan_removed =
            crate::librarian::catalog::artifact::delete_orphan_repos(&cat, &active, &active)?;
    }

    let ignore = crate::librarian::workspace::compile_ignore(&ctx.workspace.ignore)?;

    let mut total_added = 0usize;
    let mut total_updated = 0usize;
    let mut total_removed = 0usize;
    let mut total_unchanged = 0usize;
    let mut all_unknown_ids: Vec<String> = Vec::new();
    let mut backfill_errors: Vec<String> = Vec::new();

    let want_embeddings = ctx.embedding.is_some();

    for abs_root in &targets {
        let (report, embed_queue) = {
            let cat = ctx.catalog.lock();
            indexer::index_repo_sync(
                &cat,
                &ctx.rules,
                abs_root,
                &ignore,
                want_embeddings,
                a.force.unwrap_or(false),
            )?
        };

        total_added += report.added;
        total_updated += report.updated;
        total_removed += report.removed;
        total_unchanged += report.unchanged;
        all_unknown_ids.extend(report.unknown_ids);

        if let Some(ref svc) = ctx.embedding {
            let mut computed: Vec<(String, Vec<f32>)> = Vec::with_capacity(embed_queue.len());
            for (id, title, chunk_text) in &embed_queue {
                let vec = svc.embed_artifact(title.as_deref(), chunk_text).await?;
                computed.push((id.clone(), vec));
            }
            let cat = ctx.catalog.lock();
            indexer::write_embeddings(&cat, &computed)?;
        }

        {
            let cat = ctx.catalog.lock();
            // Derive a git_root for git backfill from the abs_root path.
            if let Err(e) = backfill_commits(&cat, abs_root) {
                // F-5 fix: surface the failure instead of swallowing it. The
                // backfill populates the `commits` table that `state_at(commit=)`
                // depends on; silent failure produced the "commit not indexed"
                // error that misleads callers into running reindex over and over.
                let msg = format!("{}: {}", abs_root.display(), e);
                tracing::warn!("backfill_commits failed for {}", msg);
                backfill_errors.push(msg);
            }
        }
    }

    let unknown_count = all_unknown_ids.len();
    const UNKNOWN_SAMPLE: usize = 20;
    let sample: Vec<&String> = all_unknown_ids.iter().take(UNKNOWN_SAMPLE).collect();
    Ok(json!({
        "added": total_added,
        "updated": total_updated,
        "removed": total_removed,
        "unchanged": total_unchanged,
        "orphans_removed": orphan_removed,
        "unknown_count": unknown_count,
        "unknown_sample": sample,
        "backfill_error_count": backfill_errors.len(),
        "backfill_errors": backfill_errors,
        "unknown_sample_note": if unknown_count > UNKNOWN_SAMPLE {
            format!("showing first {UNKNOWN_SAMPLE} of {unknown_count}; run CLI reindex for full list")
        } else {
            "complete".to_string()
        },
        "scope": match effective_scope {
            Scope::Project => "project",
            Scope::Repo => "repo",
            Scope::Umbrella => "umbrella",
            Scope::All => "all",
        },
        "targets": targets.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::classify::load_rules;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf, rules_toml: &str) -> ToolContext {
        let rules = load_rules(rules_toml).unwrap();
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root,
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(rules),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn indexes_two_files_one_unknown() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 2 fixture .md files
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(
            root.join("docs/specs/auth.md"),
            "---\ntitle: Auth Spec\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "# README\n").unwrap();

        // Rules match only docs/specs/*.md as "spec"; README.md stays "unknown"
        let rules_toml = "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n";
        let ctx = mk_ctx(root.to_path_buf(), rules_toml);

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            v["added"].as_u64().unwrap(),
            2,
            "should index both .md files"
        );
        assert_eq!(
            v["unknown_count"].as_u64().unwrap(),
            1,
            "README.md should be unknown"
        );
    }

    #[tokio::test]
    async fn force_wipes_then_reindexes() {
        // History (kept verbatim — see bug-tracker #7 / F-9):
        //   pre-bug-tracker-#7 → force=true issued a destructive DELETE +
        //     re-INSERT; expected added=1, unchanged=0.
        //   commit d482ca8a → DELETE removed; force=true was a no-op
        //     pending proper plumbing.
        //   task #31 → force_rewalk plumbed through index_repo_sync;
        //     force=true now bypasses the hash-equal early-return, so the
        //     row is re-walked → upsert path → counts as updated (not added).
        //
        // Today's expectation: force=true on an existing-unchanged file →
        // updated=1, added=0, unchanged=0.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/a.md"), "# A\n").unwrap();

        let ctx = mk_ctx(
            root.to_path_buf(),
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        );

        // First index
        call(&ctx, json!({})).await.unwrap();

        // Second index without force → unchanged (hash matches)
        let v2 = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v2["unchanged"].as_u64().unwrap(), 1);
        assert_eq!(v2["added"].as_u64().unwrap(), 0);

        // Third index with force=true → re-walks regardless of hash,
        // re-runs the upsert → counts as updated (id pre-existed).
        let v3 = call(&ctx, json!({"force": true})).await.unwrap();
        assert_eq!(v3["updated"].as_u64().unwrap(), 1);
        assert_eq!(v3["added"].as_u64().unwrap(), 0);
        assert_eq!(v3["unchanged"].as_u64().unwrap(), 0);
    }

    fn mk_ctx_with_project(tmp_root: std::path::PathBuf, project_subdir: &str) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(
                crate::librarian::classify::load_rules(
                    "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
                )
                .unwrap(),
            ),
            embedding: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: tmp_root.join(project_subdir),
                    git_root: tmp_root.clone(),
                    umbrella: None,
                },
            )),
        }
    }

    #[tokio::test]
    async fn project_scope_walks_only_subdir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("p1/docs")).unwrap();
        std::fs::create_dir_all(root.join("p2/docs")).unwrap();
        std::fs::write(root.join("p1/docs/a.md"), "# A\n").unwrap();
        std::fs::write(root.join("p2/docs/b.md"), "# B\n").unwrap();

        let ctx = mk_ctx_with_project(root.to_path_buf(), "p1");

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(v["added"].as_u64().unwrap(), 1, "only p1/docs/a.md indexed");
        assert_eq!(v["scope"].as_str().unwrap(), "project");
        let targets = v["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        let target = targets[0].as_str().unwrap();
        assert!(
            target.ends_with("p1"),
            "target should end with p1, got: {target}"
        );
    }

    #[tokio::test]
    async fn project_scope_force_does_not_nuke_sibling_rows() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("p1/docs")).unwrap();
        std::fs::create_dir_all(root.join("p2/docs")).unwrap();
        std::fs::write(root.join("p1/docs/a.md"), "# A\n").unwrap();
        std::fs::write(root.join("p2/docs/b.md"), "# B\n").unwrap();

        // First, index everything (scope=all)
        let ctx_all = mk_ctx(
            root.to_path_buf(),
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        );
        call(&ctx_all, json!({"scope": "all"})).await.unwrap();
        let total_before: i64 = ctx_all
            .catalog
            .lock()
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_before, 2);

        // Reuse same catalog for project-scoped force reindex from p1
        let ctx_p1 = ToolContext {
            catalog: ctx_all.catalog.clone(),
            workspace: ctx_all.workspace.clone(),
            rules: ctx_all.rules.clone(),
            embedding: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: root.join("p1"),
                    git_root: root.to_path_buf(),
                    umbrella: None,
                },
            )),
        };
        call(&ctx_p1, json!({"force": true})).await.unwrap();

        let total_after: i64 = ctx_p1
            .catalog
            .lock()
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            total_after, 2,
            "p2 row must survive a project-scoped force reindex of p1"
        );

        // Forward-slash LIKE pattern — catalog stores abs_paths in forward-slash
        // form (artifact::upsert via to_forward_slash); a native-separator pattern
        // would not match any rows on Windows.
        let p2_pattern = format!("%{}/p2/%", crate::util::fs::RepoPath::from(root));
        let p2_count: i64 = ctx_p1
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE abs_path LIKE ?1",
                rusqlite::params![p2_pattern],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p2_count, 1);
    }

    #[tokio::test]
    async fn project_scope_errors_without_current_project() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf(), "");
        let err = call(&ctx, json!({"scope": "project"})).await.unwrap_err();
        assert!(err.to_string().contains("scope=project"));
    }

    #[tokio::test]
    async fn defaults_to_all_when_no_current_project() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("a.md"), "# A\n").unwrap();
        let ctx = mk_ctx(root.to_path_buf(), "");
        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["scope"].as_str().unwrap(), "all");
        assert_eq!(v["added"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn reindex_backfills_commits_table() {
        use std::process::Command;

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("r1");
        std::fs::create_dir_all(&repo_path).unwrap();

        let run = |args: &[&str], cwd: &std::path::Path| {
            Command::new("git")
                .args(args)
                .current_dir(cwd)
                .output()
                .unwrap()
        };

        // git init (plain — avoid -b flag for older git compatibility)
        run(&["init", "-q"], &repo_path);
        run(&["config", "user.email", "test@test.com"], &repo_path);
        run(&["config", "user.name", "Test User"], &repo_path);

        // 3 commits
        for i in 1..=3u32 {
            std::fs::write(repo_path.join("f.md"), format!("v{i}")).unwrap();
            run(&["add", "."], &repo_path);
            run(&["commit", "-q", "-m", &format!("c{i}")], &repo_path);
        }

        // Build a ToolContext pointing at this repo as "r1"
        let rules = crate::librarian::classify::load_rules("").unwrap();
        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r1".into(),
                    path: repo_path.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(rules),
            embedding: None,
            current_project: None,
        };

        // Run reindex — this should backfill the commits table
        call(&ctx, json!({})).await.unwrap();

        // Assert 3 rows in commits table for "r1"
        let n: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT COUNT(*) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(n, 3, "should have 3 commit rows");

        // newest commit = highest topo_order = 2 (0-indexed: c1=0, c2=1, c3=2)
        let max_order: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT MAX(topo_order) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(max_order, 2, "newest commit should have topo_order=2");

        // topo_order is monotonically increasing (all distinct 0,1,2)
        let min_order: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT MIN(topo_order) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(min_order, 0, "oldest commit should have topo_order=0");
    }
}
