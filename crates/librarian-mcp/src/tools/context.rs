use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::catalog::{artifact, links};

use super::{Tool, ToolContext};

pub struct LibrarianContext;

#[derive(Deserialize)]
struct Args {
    topic: Option<String>,
    anchor_id: Option<String>,
    max_tokens: Option<usize>,
}

const DEFAULT_MAX_TOKENS: usize = 4000;

#[async_trait::async_trait]
impl Tool for LibrarianContext {
    fn name(&self) -> &'static str {
        "librarian_context"
    }

    fn description(&self) -> &'static str {
        "Build a packed markdown context bundle around a topic or anchor artifact."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {"type": "string"},
                "anchor_id": {"type": "string"},
                "max_tokens": {"type": "integer"}
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        use crate::catalog::find::{find, FindOpts};
        use std::collections::HashMap;

        let a: Args = serde_json::from_value(args)?;
        let max_tokens = a.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        let char_cap = max_tokens * 4;

        // If topic + embedder: compute the embedding vector *before* locking the catalog.
        let topic_vec: Option<Vec<f32>> =
            if let (Some(ref topic), Some(ref svc)) = (&a.topic, &ctx.embedding) {
                Some(svc.embedder.embed_query(topic).await?)
            } else {
                None
            };

        // Collect candidate artifact IDs in order (all sync, lock held only briefly).
        let candidate_ids: Vec<String> = {
            let cat = ctx.catalog.lock();
            if let Some(ref anchor_id) = a.anchor_id {
                // Start with anchor, then depth-1 neighbors (dedupe)
                let mut ids: Vec<String> = vec![anchor_id.clone()];
                let out = links::outgoing(&cat, anchor_id)?;
                let inc = links::incoming(&cat, anchor_id)?;
                for link in out {
                    if !ids.contains(&link.dst_id) {
                        ids.push(link.dst_id);
                    }
                }
                for link in inc {
                    if !ids.contains(&link.src_id) {
                        ids.push(link.src_id);
                    }
                }
                ids
            } else if a.topic.is_some() {
                if let Some(vec) = topic_vec {
                    // Semantic search path
                    let rows = find(
                        &cat,
                        &FindOpts {
                            filter: None,
                            limit: 50,
                            offset: 0,
                            semantic: Some(vec),
                        },
                    )?;
                    rows.into_iter().map(|r| r.id).collect()
                } else {
                    // LIKE fallback when no embedder
                    let topic = a.topic.as_deref().unwrap_or("");
                    let pattern = format!("%{topic}%");
                    let mut stmt = cat.conn.prepare(
                        "SELECT id FROM artifact \
                         WHERE title LIKE ?1 OR topic LIKE ?1 \
                         ORDER BY updated_at DESC LIMIT 50",
                    )?;
                    let ids: Vec<String> = stmt
                        .query_map(rusqlite::params![pattern], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect();
                    ids
                }
            } else {
                return Ok(json!({"markdown": "", "included_ids": []}));
            }
            // MutexGuard dropped here
        };

        // Batch-fetch all candidate rows in a single query.
        let rows_map: HashMap<String, artifact::ArtifactRow> = {
            let cat = ctx.catalog.lock();
            if candidate_ids.is_empty() {
                HashMap::new()
            } else {
                let placeholders = (0..candidate_ids.len())
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "SELECT id, repo, rel_path, kind, status, title, owners, tags, topic, \
                     time_scope, source, created_at, updated_at, file_mtime, \
                     file_sha256, confidence FROM artifact WHERE id IN ({placeholders})"
                );
                let mut stmt = cat.conn.prepare(&sql)?;
                let params = rusqlite::params_from_iter(candidate_ids.iter());
                let rows: Vec<artifact::ArtifactRow> = stmt
                    .query_map(params, artifact::row_from_sql)?
                    .collect::<Result<_, _>>()?;
                rows.into_iter().map(|r| (r.id.clone(), r)).collect()
            }
        };

        // Build root lookup: repo name → root path
        let root_map: std::collections::HashMap<String, std::path::PathBuf> = ctx
            .workspace
            .roots
            .iter()
            .map(|r| (r.name.clone(), r.path.clone()))
            .collect();

        let mut markdown = String::new();
        let mut included_ids: Vec<String> = Vec::new();

        for id in &candidate_ids {
            // Look up row from the batch result, preserving candidate order.
            let row = match rows_map.get(id) {
                Some(r) => r,
                None => continue,
            };

            // Read file content
            let repo_root = match root_map.get(&row.repo) {
                Some(p) => p,
                None => continue,
            };
            let full_path = repo_root.join(&row.rel_path);
            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Extract body (skip frontmatter)
            let body = match crate::frontmatter::parse(&content) {
                Ok((_, body)) => body.to_string(),
                Err(_) => content.clone(),
            };

            // First 30 lines of body
            let first_30: String = body.lines().take(30).collect::<Vec<_>>().join("\n");

            // Render section
            let title = row.title.as_deref().unwrap_or("(untitled)");
            let section = format!(
                "## {}  — {}/{}  ({}/{})\n{}\n\n",
                title, row.kind, row.status, row.repo, row.rel_path, first_30
            );

            // Check token budget (chars / 4 approximation)
            if !markdown.is_empty() && (markdown.len() + section.len()) > char_cap {
                break;
            }

            markdown.push_str(&section);
            included_ids.push(id.clone());

            if markdown.len() >= char_cap {
                break;
            }
        }

        Ok(json!({
            "markdown": markdown,
            "included_ids": included_ids
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact::ArtifactRow, Catalog};
    use crate::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn sample_row(
        id: &str,
        repo: &str,
        rel_path: &str,
        title: &str,
        topic: Option<&str>,
    ) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.into(),
            repo: repo.into(),
            rel_path: rel_path.into(),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(title.into()),
            owners: vec![],
            tags: vec![],
            topic: topic.map(|s| s.into()),
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "abc".into(),
            confidence: 1.0,
        }
    }

    fn mk_ctx(tmp_root: std::path::PathBuf, cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root,
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn topic_search_returns_matching_artifacts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 3 real .md files
        std::fs::write(root.join("auth_login.md"), "# Auth Login\nsome body\n").unwrap();
        std::fs::write(root.join("auth_signup.md"), "# Auth Signup\nsome body\n").unwrap();
        std::fs::write(root.join("billing.md"), "# Billing\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_login.md", "r", "auth_login.md", "Auth Login", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row(
                "r/auth_signup.md",
                "r",
                "auth_signup.md",
                "Auth Signup",
                None,
            ),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/billing.md", "r", "billing.md", "Billing", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = LibrarianContext
            .call(&ctx, json!({"topic": "auth"}))
            .await
            .unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2, "only auth artifacts should be included");

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("Auth Login"),
            "markdown should contain Auth Login title"
        );
        assert!(
            md.contains("Auth Signup"),
            "markdown should contain Auth Signup title"
        );
        assert!(
            !md.contains("Billing"),
            "markdown should not contain Billing"
        );
    }

    #[tokio::test]
    async fn max_tokens_caps_inclusion() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 2 auth files
        std::fs::write(root.join("auth_a.md"), "# Auth A\n".repeat(5)).unwrap();
        std::fs::write(root.join("auth_b.md"), "# Auth B\n".repeat(5)).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_a.md", "r", "auth_a.md", "Auth A", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_b.md", "r", "auth_b.md", "Auth B", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        // max_tokens=1 means char_cap=4 — way too small for any full section, but first
        // artifact is always included (budget check only triggers on subsequent artifacts).
        // Use a slightly larger budget that fits exactly 1 section.
        // Each section header is ~50+ chars; set max_tokens=15 (60 chars) → fits 1, not 2.
        let v = LibrarianContext
            .call(&ctx, json!({"topic": "auth", "max_tokens": 15}))
            .await
            .unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(
            ids.len(),
            1,
            "max_tokens should cap inclusion to 1 artifact"
        );
    }

    #[tokio::test]
    async fn no_args_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

        let v = LibrarianContext.call(&ctx, json!({})).await.unwrap();

        assert_eq!(v["markdown"].as_str().unwrap(), "");
        assert_eq!(v["included_ids"].as_array().unwrap().len(), 0);
    }
}
