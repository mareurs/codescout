use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::observations::{self, ObservationRow};

pub struct ArtifactObserve;

#[derive(Deserialize)]
struct Args {
    id: String,
    text: String,
    #[serde(default)]
    source: Option<String>,
}

#[async_trait]
impl Tool for ArtifactObserve {
    fn name(&self) -> &'static str {
        "artifact_observe"
    }

    fn description(&self) -> &'static str {
        "Append an observation (free-text note) to an artifact. Useful for recording agent insights, review comments, or status notes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "text"],
            "properties": {
                "id": {"type": "string", "description": "Artifact ID to annotate"},
                "text": {"type": "string", "description": "Observation text"},
                "source": {"type": "string", "description": "Optional source label (e.g. agent name)"}
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        let obs = ObservationRow {
            id: None,
            artifact_id: a.id,
            text: a.text,
            source: a.source,
            created_at: now,
        };
        let observation_id = observations::insert(&cat, &obs)?;
        Ok(json!({"observation_id": observation_id}))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::catalog::observations::list_for_artifact;
    use crate::catalog::Catalog;
    use crate::tools::create::ArtifactCreate;
    use crate::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    pub(crate) fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
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
    async fn insert_observation_list_returns_it() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Create an artifact first so the FK constraint is satisfied.
        let v = ArtifactCreate
            .call(
                &ctx,
                serde_json::json!({
                    "repo": "r", "rel_path": "note.md",
                    "kind": "doc", "title": "N", "body": "body"
                }),
            )
            .await
            .unwrap();
        let artifact_id = v["id"].as_str().unwrap().to_string();

        let result = ArtifactObserve
            .call(
                &ctx,
                serde_json::json!({
                    "id": artifact_id,
                    "text": "looks good",
                    "source": "review-agent"
                }),
            )
            .await
            .unwrap();

        assert!(
            result["observation_id"].is_i64() || result["observation_id"].is_u64(),
            "observation_id should be an integer, got: {:?}",
            result["observation_id"]
        );

        let rows = list_for_artifact(&ctx.catalog.lock(), &artifact_id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "looks good");
        assert_eq!(rows[0].source.as_deref(), Some("review-agent"));
    }
}
