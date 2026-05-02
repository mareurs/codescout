use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::indexer;

use super::ToolContext;

#[derive(Deserialize)]
struct Args {
    repo: Option<String>,
    force: Option<bool>,
    /// Scope of the reindex. Defaults to `project` when a current project is
    /// resolved, else `all`. Mirrors the read-tool scope semantics.
    scope: Option<super::scope::Scope>,
}

fn backfill_commits(
    catalog: &crate::catalog::Catalog,
    repo_path: &std::path::Path,
    repo_name: &str,
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
    // TOPOLOGICAL | REVERSE = parents before children = lowest topo_order for oldest commit
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    if let Err(e) = walk.push_head() {
        // Empty repo or detached HEAD with no commits
        tracing::debug!(
            "revwalk push_head failed for {}: {}",
            repo_path.display(),
            e
        );
        return Ok(());
    }

    let rows: anyhow::Result<Vec<_>> = walk
        .enumerate()
        .map(|(order, oid_result)| {
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            Ok(crate::catalog::commits::CommitRow {
                hash: oid.to_string(),
                repo: repo_name.to_string(),
                authored_at: Some(commit.time().seconds() * 1000),
                subject: commit.summary().map(String::from),
                topo_order: Some(order as i64),
            })
        })
        .collect();
    crate::catalog::commits::upsert_many(catalog, &rows?)?;
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

    let targets: Vec<(crate::workspace::Root, Option<String>)> = match effective_scope {
        Scope::All => {
            let roots: Vec<_> = if let Some(ref repo_name) = a.repo {
                let root = ctx
                    .workspace
                    .roots
                    .iter()
                    .find(|r| &r.name == repo_name)
                    .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", repo_name))?;
                vec![root.clone()]
            } else {
                ctx.workspace.roots.clone()
            };
            roots.into_iter().map(|r| (r, None)).collect()
        }
        Scope::Repo => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=repo requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            let root = ctx
                .workspace
                .roots
                .iter()
                .find(|r| r.name == cp.root)
                .ok_or_else(|| anyhow::anyhow!("workspace root `{}` not found", cp.root))?;
            vec![(root.clone(), None)]
        }
        Scope::Project => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=project requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            let root = ctx
                .workspace
                .roots
                .iter()
                .find(|r| r.name == cp.root)
                .ok_or_else(|| anyhow::anyhow!("workspace root `{}` not found", cp.root))?;
            let subdir = if cp.subdir.is_empty() {
                None
            } else {
                Some(cp.subdir.clone())
            };
            vec![(root.clone(), subdir)]
        }
        Scope::Umbrella => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!("scope=umbrella requires a resolved current project")
            })?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella declared for {}/{}",
                    cp.root,
                    cp.subdir
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
            let mut out = Vec::with_capacity(umbrella.members.len());
            for m in &umbrella.members {
                let (root_name, sub) = match m.split_once('/') {
                    Some((r, s)) => (r, Some(s.to_string())),
                    None => (m.as_str(), None),
                };
                let root = ctx
                    .workspace
                    .roots
                    .iter()
                    .find(|r| r.name == root_name)
                    .ok_or_else(|| {
                        anyhow::anyhow!("umbrella member root `{root_name}` not in workspace")
                    })?;
                out.push((root.clone(), sub));
            }
            out
        }
    };

    if a.force == Some(true) {
        let cat = ctx.catalog.lock();
        for (root, subdir) in &targets {
            match subdir {
                Some(s) if !s.is_empty() => {
                    cat.conn.execute(
                        "DELETE FROM artifact WHERE repo = ?1 AND rel_path LIKE ?2",
                        rusqlite::params![root.name, format!("{s}/%")],
                    )?;
                }
                _ => {
                    cat.conn.execute(
                        "DELETE FROM artifact WHERE repo = ?1",
                        rusqlite::params![root.name],
                    )?;
                }
            }
        }
    }

    let mut orphan_removed = 0usize;
    if effective_scope == Scope::All && a.repo.is_none() {
        let cat = ctx.catalog.lock();
        let active: Vec<&str> = ctx
            .workspace
            .roots
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        orphan_removed = crate::catalog::artifact::delete_orphan_repos(&cat, &active)?;
    }

    let ignore = crate::workspace::compile_ignore(&ctx.workspace.ignore)?;

    let mut total_added = 0usize;
    let mut total_updated = 0usize;
    let mut total_removed = 0usize;
    let mut total_unchanged = 0usize;
    let mut all_unknown_ids: Vec<String> = Vec::new();

    let want_embeddings = ctx.embedding.is_some();

    for (root, subdir) in &targets {
        let subdir_ref = subdir.as_deref();
        let (report, embed_queue) = {
            let cat = ctx.catalog.lock();
            indexer::index_repo_sync(
                &cat,
                &ctx.rules,
                &root.name,
                &root.path,
                subdir_ref,
                &ignore,
                want_embeddings,
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
            if let Err(e) = backfill_commits(&cat, &root.path, &root.name) {
                tracing::debug!("backfill_commits skipped for {}: {}", root.name, e);
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
        "targets": targets.iter().map(|(r, s)| json!({
            "repo": r.name,
            "subdir": s,
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::classify::load_rules;
    use crate::workspace::{Root, WorkspaceConfig};
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
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/a.md"), "# A\n").unwrap();

        let ctx = mk_ctx(root.to_path_buf(), "");

        // First index
        call(&ctx, json!({})).await.unwrap();

        // Second index without force → unchanged
        let v2 = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v2["unchanged"].as_u64().unwrap(), 1);
        assert_eq!(v2["added"].as_u64().unwrap(), 0);

        // Third index with force=true → wipes and re-adds
        let v3 = call(&ctx, json!({"force": true})).await.unwrap();
        assert_eq!(v3["added"].as_u64().unwrap(), 1);
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
            rules: Arc::new(crate::classify::load_rules("").unwrap()),
            embedding: None,
            current_project: Some(Arc::new(crate::current_project::CurrentProject {
                root: "r".into(),
                subdir: project_subdir.into(),
                path: tmp_root.join(project_subdir),
                umbrella: None,
            })),
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
        assert_eq!(targets[0]["subdir"].as_str().unwrap(), "p1");
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
        let ctx_all = mk_ctx(root.to_path_buf(), "");
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
            current_project: Some(Arc::new(crate::current_project::CurrentProject {
                root: "r".into(),
                subdir: "p1".into(),
                path: root.join("p1"),
                umbrella: None,
            })),
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

        let p2_count: i64 = ctx_p1
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE rel_path LIKE 'p2/%'",
                [],
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
        let rules = crate::classify::load_rules("").unwrap();
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
                    "SELECT COUNT(*) FROM commits WHERE repo=?1",
                    rusqlite::params!["r1"],
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
                    "SELECT MAX(topo_order) FROM commits WHERE repo=?1",
                    rusqlite::params!["r1"],
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
                    "SELECT MIN(topo_order) FROM commits WHERE repo=?1",
                    rusqlite::params!["r1"],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(min_order, 0, "oldest commit should have topo_order=0");
    }
}
