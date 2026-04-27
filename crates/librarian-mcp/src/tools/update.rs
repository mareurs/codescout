use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::artifact;

pub struct ArtifactUpdate;

#[derive(Deserialize, Default)]
struct UpdatePatch {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    owners: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize)]
struct Args {
    id: String,
    patch: UpdatePatch,
}

#[async_trait]
impl Tool for ArtifactUpdate {
    fn name(&self) -> &'static str {
        "artifact_update"
    }

    fn description(&self) -> &'static str {
        "Update an existing artifact's frontmatter fields and/or body. Only provided fields are changed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "patch"],
            "properties": {
                "id": {"type": "string"},
                "patch": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string"},
                        "title": {"type": "string"},
                        "owners": {"type": "array", "items": {"type": "string"}},
                        "tags": {"type": "array", "items": {"type": "string"}},
                        "topic": {"type": "string"},
                        "body": {"type": "string"}
                    }
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let row =
            artifact::get(&cat, &a.id)?.ok_or_else(|| anyhow::anyhow!("unknown id `{}`", a.id))?;

        let root = ctx
            .workspace
            .roots
            .iter()
            .find(|r| r.name == row.repo)
            .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", row.repo))?;
        let full = root.path.join(&row.rel_path);

        let original = std::fs::read_to_string(&full)?;
        let patch = &a.patch;

        let new_content = if let Some(new_body) = &patch.body {
            // Re-parse frontmatter and rebuild with new body
            let (fm_opt, _old_body) = crate::frontmatter::parse(&original)?;
            let mut fm = fm_opt.unwrap_or_default();
            if let Some(v) = &patch.status {
                fm.status = Some(v.clone());
            }
            if let Some(v) = &patch.title {
                fm.title = Some(v.clone());
            }
            if let Some(v) = &patch.owners {
                fm.owners = v.clone();
            }
            if let Some(v) = &patch.tags {
                fm.tags = v.clone();
            }
            if let Some(v) = &patch.topic {
                fm.topic = Some(v.clone());
            }
            crate::frontmatter::write(&fm, &format!("\n{}\n", new_body))
        } else {
            crate::frontmatter::update_in_place(&original, |fm| {
                if let Some(v) = &patch.status {
                    fm.status = Some(v.clone());
                }
                if let Some(v) = &patch.title {
                    fm.title = Some(v.clone());
                }
                if let Some(v) = &patch.owners {
                    fm.owners = v.clone();
                }
                if let Some(v) = &patch.tags {
                    fm.tags = v.clone();
                }
                if let Some(v) = &patch.topic {
                    fm.topic = Some(v.clone());
                }
            })?
        };

        std::fs::write(&full, &new_content)?;

        let now = chrono::Utc::now().timestamp_millis();
        let file_mtime = std::fs::metadata(&full)
            .ok()
            .and_then(|m| {
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_millis() as i64)
                })
            })
            .unwrap_or(now);

        // Build updated row from original, applying patch
        let updated_row = crate::catalog::artifact::ArtifactRow {
            id: row.id.clone(),
            repo: row.repo.clone(),
            rel_path: row.rel_path.clone(),
            kind: row.kind.clone(),
            status: patch.status.clone().unwrap_or(row.status),
            title: patch.title.clone().or(row.title),
            owners: patch.owners.clone().unwrap_or(row.owners),
            tags: patch.tags.clone().unwrap_or(row.tags),
            topic: patch.topic.clone().or(row.topic),
            time_scope: row.time_scope,
            source: row.source,
            created_at: row.created_at,
            updated_at: now,
            file_mtime,
            file_sha256: crate::util::sha_of_bytes(new_content.as_bytes()),
            confidence: row.confidence,
        };
        artifact::upsert(&cat, &updated_row)?;

        Ok(json!({"id": a.id, "updated": true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact;
    use crate::catalog::Catalog;
    use crate::tools::create::ArtifactCreate;
    use crate::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
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
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn update_title_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = ArtifactCreate
            .call(
                &ctx,
                serde_json::json!({
                    "repo": "r", "rel_path": "doc.md",
                    "kind": "spec", "title": "Old", "body": "content"
                }),
            )
            .await
            .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        ArtifactUpdate
            .call(
                &ctx,
                serde_json::json!({"id": id, "patch": {"title": "New"}}),
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("doc.md")).unwrap();
        assert!(content.contains("title: New"), "file should have new title");
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.title.as_deref(), Some("New"));
    }

    #[tokio::test]
    async fn update_status_archived_persisted() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = ArtifactCreate
            .call(
                &ctx,
                serde_json::json!({
                    "repo": "r", "rel_path": "doc2.md",
                    "kind": "spec", "title": "T", "body": "b"
                }),
            )
            .await
            .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        ArtifactUpdate
            .call(
                &ctx,
                serde_json::json!({"id": id, "patch": {"status": "archived"}}),
            )
            .await
            .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.status, "archived");
    }

    #[tokio::test]
    async fn missing_id_errors() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = ArtifactUpdate
            .call(
                &ctx,
                serde_json::json!({"id": "nonexistent", "patch": {"title": "X"}}),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown id"));
    }

    #[tokio::test]
    async fn body_patch_preserves_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = ArtifactCreate
            .call(
                &ctx,
                serde_json::json!({
                    "repo": "r", "rel_path": "doc3.md",
                    "kind": "spec", "title": "Keep", "body": "old body"
                }),
            )
            .await
            .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        ArtifactUpdate
            .call(
                &ctx,
                serde_json::json!({"id": id, "patch": {"body": "brand new"}}),
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("doc3.md")).unwrap();
        assert!(content.starts_with("---\n"), "frontmatter must be present");
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(
            row.title.as_deref(),
            Some("Keep"),
            "title should be unchanged"
        );
    }
}
