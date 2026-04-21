use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::indexer;

use super::{Tool, ToolContext};

pub struct LibrarianReindex;

#[derive(Deserialize)]
struct Args {
    repo: Option<String>,
    force: Option<bool>,
}

#[async_trait::async_trait]
impl Tool for LibrarianReindex {
    fn name(&self) -> &'static str {
        "librarian_reindex"
    }

    fn description(&self) -> &'static str {
        "Re-scan repos, classify + upsert. force=true wipes existing rows first."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "repo": {"type": "string"},
                "force": {"type": "boolean"}
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        // Determine target roots
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

        // If force, wipe rows for each target repo before indexing
        if a.force == Some(true) {
            let cat = ctx.catalog.lock();
            for root in &roots {
                cat.conn.execute(
                    "DELETE FROM artifact WHERE repo = ?1",
                    rusqlite::params![root.name],
                )?;
            }
        }

        // Whole-workspace reindex: drop rows for repos no longer in workspace.toml.
        let mut orphan_removed = 0usize;
        if a.repo.is_none() {
            let cat = ctx.catalog.lock();
            let active: Vec<&str> = ctx
                .workspace
                .roots
                .iter()
                .map(|r| r.name.as_str())
                .collect();
            orphan_removed = crate::catalog::artifact::delete_orphan_repos(&cat, &active)?;
        }

        // Compile ignore globs once
        let ignore = crate::workspace::compile_ignore(&ctx.workspace.ignore)?;

        // Aggregate reports
        let mut total_added = 0usize;
        let mut total_updated = 0usize;
        let mut total_removed = 0usize;
        let mut total_unchanged = 0usize;
        let mut all_unknown_ids: Vec<String> = Vec::new();

        let want_embeddings = ctx.embedding.is_some();

        for root in &roots {
            // --- Phase 1: sync walk (lock held, no await) ---
            let (report, embed_queue) = {
                let cat = ctx.catalog.lock();
                indexer::index_repo_sync(
                    &cat,
                    &ctx.rules,
                    &root.name,
                    &root.path,
                    &ignore,
                    want_embeddings,
                )?
                // MutexGuard dropped here
            };

            total_added += report.added;
            total_updated += report.updated;
            total_removed += report.removed;
            total_unchanged += report.unchanged;
            all_unknown_ids.extend(report.unknown_ids);

            // --- Phase 2: async embedding (lock NOT held) ---
            if let Some(ref svc) = ctx.embedding {
                let mut computed: Vec<(String, Vec<f32>)> = Vec::with_capacity(embed_queue.len());
                for (id, title, chunk_text) in &embed_queue {
                    let vec = svc.embed_artifact(title.as_deref(), chunk_text).await?;
                    computed.push((id.clone(), vec));
                }
                // --- Phase 3: sync write (lock held again, no await) ---
                let cat = ctx.catalog.lock();
                indexer::write_embeddings(&cat, &computed)?;
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
            }
        }))
    }
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
            }),
            rules: Arc::new(rules),
            embedding: None,
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

        let v = LibrarianReindex.call(&ctx, json!({})).await.unwrap();

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
        LibrarianReindex.call(&ctx, json!({})).await.unwrap();

        // Second index without force → unchanged
        let v2 = LibrarianReindex.call(&ctx, json!({})).await.unwrap();
        assert_eq!(v2["unchanged"].as_u64().unwrap(), 1);
        assert_eq!(v2["added"].as_u64().unwrap(), 0);

        // Third index with force=true → wipes and re-adds
        let v3 = LibrarianReindex
            .call(&ctx, json!({"force": true}))
            .await
            .unwrap();
        assert_eq!(v3["added"].as_u64().unwrap(), 1);
        assert_eq!(v3["unchanged"].as_u64().unwrap(), 0);
    }
}
