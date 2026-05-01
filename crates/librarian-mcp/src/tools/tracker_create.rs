use crate::catalog::{artifact, augmentation};
use crate::ids::artifact_id;
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct TrackerCreate;

#[derive(Deserialize)]
struct Args {
    repo: String,
    rel_path: String,
    title: String,
    prompt: String,
    params: Option<Value>,
}

#[async_trait]
impl Tool for TrackerCreate {
    fn name(&self) -> &'static str {
        "tracker_create"
    }

    fn description(&self) -> &'static str {
        "Atomically create a tracker artifact (kind=tracker) and attach augmentation \
         in one call. Shorthand for artifact_create + artifact_augment. \
         Returns the new artifact id."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["repo", "rel_path", "title", "prompt"],
            "properties": {
                "repo": {
                    "type": "string",
                    "description": "Repository name (as configured in workspace.toml)"
                },
                "rel_path": {
                    "type": "string",
                    "description": "Relative path for the new file (e.g. 'trackers/features.md')"
                },
                "title": { "type": "string" },
                "prompt": {
                    "type": "string",
                    "description": "Persistent refresh instruction"
                },
                "params": {
                    "type": "object",
                    "description": "Optional gather config"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        if a.rel_path.contains("..") || std::path::Path::new(&a.rel_path).is_absolute() {
            return Err(RecoverableError::new(
                "rel_path must be relative and must not contain '..'",
            ));
        }

        let repo_root = ctx
            .workspace
            .roots
            .iter()
            .find(|r| r.name == a.repo)
            .map(|r| r.path.clone())
            .ok_or_else(|| {
                RecoverableError::new(format!("repo '{}' not found in workspace", a.repo))
            })?;

        let full_path = repo_root.join(&a.rel_path);

        if full_path.exists() {
            return Err(RecoverableError::new(format!(
                "file already exists: {}",
                full_path.display()
            )));
        }

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let frontmatter = format!(
            "---\nkind: tracker\nstatus: active\ntitle: \"{}\"\n---\n\n",
            a.title.replace('"', "\\\"")
        );
        std::fs::write(&full_path, &frontmatter)?;

        let id = artifact_id(&a.repo, &a.rel_path);
        let now = chrono::Utc::now().timestamp_millis();

        let params_str = a
            .params
            .map(|p| serde_json::to_string(&p))
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        {
            let cat = ctx.catalog.lock();

            artifact::upsert(
                &cat,
                &artifact::ArtifactRow {
                    id: id.clone(),
                    repo: a.repo,
                    rel_path: a.rel_path,
                    kind: "tracker".to_string(),
                    status: "active".to_string(),
                    title: Some(a.title),
                    owners: vec![],
                    tags: vec![],
                    topic: None,
                    time_scope: None,
                    source: None,
                    created_at: now,
                    updated_at: now,
                    file_mtime: now,
                    file_sha256: "".to_string(),
                    confidence: 1.0,
                },
            )?;

            let ts = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            augmentation::upsert(
                &cat,
                &augmentation::AugmentationRow {
                    artifact_id: id.clone(),
                    prompt: a.prompt,
                    params: params_str,
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: ts.clone(),
                    updated_at: ts,
                    render_template: None,
                    params_schema: None,
                },
            )?;
        }

        Ok(json!({ "id": id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{augmentation, Catalog};
    use crate::workspace::{Root, WorkspaceConfig};
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp: &TempDir) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "repo".into(),
                    path: tmp.path().to_path_buf(),
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
    async fn creates_file_artifact_and_augmentation() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let result = TrackerCreate
            .call(
                &ctx,
                json!({
                    "repo": "repo",
                    "rel_path": "trackers/features.md",
                    "title": "Feature State",
                    "prompt": "Keep features updated",
                    "params": {"format": "table"}
                }),
            )
            .await
            .unwrap();

        let id = result["id"].as_str().unwrap();
        assert!(!id.is_empty());
        assert!(tmp.path().join("trackers/features.md").exists());

        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, id).unwrap().unwrap();
        assert_eq!(aug.prompt, "Keep features updated");

        let art = crate::catalog::artifact::get(&cat, id).unwrap().unwrap();
        assert_eq!(art.kind, "tracker");
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("exists.md"), "x").unwrap();
        let ctx = mk_ctx(&tmp);
        let err = TrackerCreate
            .call(
                &ctx,
                json!({
                    "repo": "repo",
                    "rel_path": "exists.md",
                    "title": "T",
                    "prompt": "p"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn rejects_dotdot_path() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let err = TrackerCreate
            .call(
                &ctx,
                json!({
                    "repo": "repo",
                    "rel_path": "../escape.md",
                    "title": "T",
                    "prompt": "p"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn rejects_unknown_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let err = TrackerCreate
            .call(
                &ctx,
                json!({
                    "repo": "unknown",
                    "rel_path": "foo.md",
                    "title": "T",
                    "prompt": "p"
                }),
            )
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
