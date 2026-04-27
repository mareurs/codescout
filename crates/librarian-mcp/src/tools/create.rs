use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::artifact::{self, ArtifactRow};
use crate::frontmatter::Frontmatter;
use crate::ids::artifact_id;

pub struct ArtifactCreate;

fn validate_rel_path(rel: &str) -> Result<()> {
    use std::path::{Component, Path};
    let p = Path::new(rel);
    if p.is_absolute() {
        bail!("rel_path must be relative: {}", rel);
    }
    for c in p.components() {
        match c {
            Component::ParentDir => bail!("rel_path must not contain `..`: {}", rel),
            Component::Prefix(_) | Component::RootDir => {
                bail!("rel_path must be relative: {}", rel)
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct Args {
    repo: String,
    rel_path: String,
    kind: String,
    title: String,
    body: String,
    #[serde(default)]
    owners: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[async_trait]
impl Tool for ArtifactCreate {
    fn name(&self) -> &'static str {
        "artifact_create"
    }

    fn description(&self) -> &'static str {
        "Create a new artifact. Writes frontmatter + body to the file. Fails if path exists."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["repo", "rel_path", "kind", "title", "body"],
            "properties": {
                "repo": {"type": "string"},
                "rel_path": {"type": "string"},
                "kind": {"type": "string"},
                "title": {"type": "string"},
                "body": {"type": "string"},
                "owners": {"type": "array", "items": {"type": "string"}},
                "tags": {"type": "array", "items": {"type": "string"}}
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let mut a: Args = serde_json::from_value(args)?;
        let root = ctx
            .workspace
            .roots
            .iter()
            .find(|r| r.name == a.repo)
            .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", a.repo))?;
        validate_rel_path(&a.rel_path)?;
        a.rel_path = crate::util::normalize_rel_path(&a.rel_path);
        let full = root.path.join(&a.rel_path);
        if full.exists() {
            bail!("path exists: {}", full.display());
        }
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let id = artifact_id(&a.repo, &a.rel_path);
        let fm = Frontmatter {
            id: Some(id.clone()),
            kind: Some(a.kind.clone()),
            status: Some("draft".into()),
            title: Some(a.title.clone()),
            owners: a.owners.clone(),
            tags: a.tags.clone(),
            topic: None,
            time_scope: None,
        };
        let content = crate::frontmatter::write(&fm, &format!("\n{}\n", a.body));
        std::fs::write(&full, &content)?;
        let now = chrono::Utc::now().timestamp_millis();
        let row = ArtifactRow {
            id: id.clone(),
            repo: a.repo,
            rel_path: a.rel_path,
            kind: a.kind,
            status: "draft".into(),
            title: Some(a.title),
            owners: a.owners,
            tags: a.tags,
            topic: None,
            time_scope: None,
            source: Some("generated".into()),
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: crate::util::sha_of_bytes(content.as_bytes()),
            confidence: 1.0,
        };
        artifact::upsert(&ctx.catalog.lock(), &row)?;
        Ok(json!({"id": id, "repo": row.repo, "rel_path": row.rel_path}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
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
    async fn creates_file_and_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = ArtifactCreate
            .call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": "docs/specs/x.md",
                    "kind": "spec", "title": "X", "body": "hello"
                }),
            )
            .await
            .unwrap();
        let path = tmp.path().join("docs/specs/x.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: X"));
        let id = v["id"].as_str().unwrap();
        assert!(artifact::get(&ctx.catalog.lock(), id).unwrap().is_some());
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/x.md"), "").unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = ArtifactCreate
            .call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": "docs/x.md",
                    "kind": "doc", "title": "X", "body": "hi"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("path exists"));
    }

    #[tokio::test]
    async fn rejects_parent_dir_traversal() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = ArtifactCreate
            .call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": "../escape.md",
                    "kind": "doc", "title": "X", "body": "hi"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains(".."), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = ArtifactCreate
            .call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": "/etc/passwd",
                    "kind": "doc", "title": "X", "body": "hi"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("relative"), "got: {err}");
    }
}
