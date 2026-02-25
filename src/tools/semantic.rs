//! Semantic search tools backed by the embedding index.

use super::{Tool, ToolContext};
use serde_json::{json, Value};

pub struct SemanticSearch;
pub struct IndexProject;
pub struct IndexStatus;

#[async_trait::async_trait]
impl Tool for SemanticSearch {
    fn name(&self) -> &str {
        "semantic_search"
    }
    fn description(&self) -> &str {
        "Find code by natural language description or code snippet. \
         Returns ranked chunks with file path, line range, and similarity score."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language description or code snippet to search for"
                },
                "limit": { "type": "integer", "default": 10 }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'query' parameter"))?;
        let limit = input["limit"].as_u64().unwrap_or(10) as usize;

        let (root, model) = {
            let inner = ctx.agent.inner.read().await;
            let p = inner
                .active_project
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No active project. Use activate_project first."))?;
            (p.root.clone(), p.config.embeddings.model.clone())
        };

        let conn = crate::embed::index::open_db(&root)?;
        let embedder = crate::embed::create_embedder(&model).await?;
        let query_embedding = crate::embed::embed_one(embedder.as_ref(), query).await?;
        let results = crate::embed::index::search(&conn, &query_embedding, limit)?;

        Ok(json!({
            "results": results.iter().map(|r| json!({
                "file_path": r.file_path,
                "language": r.language,
                "content": r.content,
                "start_line": r.start_line,
                "end_line": r.end_line,
                "score": r.score,
            })).collect::<Vec<_>>(),
            "total": results.len(),
        }))
    }
}

#[async_trait::async_trait]
impl Tool for IndexProject {
    fn name(&self) -> &str {
        "index_project"
    }
    fn description(&self) -> &str {
        "Build or incrementally update the semantic search index for the active project."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": { "type": "boolean", "default": false,
                    "description": "Force full reindex, ignoring cached file hashes" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let force = input["force"].as_bool().unwrap_or(false);
        let root = ctx.agent.require_project_root().await?;

        crate::embed::index::build_index(&root, force).await?;

        let conn = crate::embed::index::open_db(&root)?;
        let stats = crate::embed::index::index_stats(&conn)?;

        Ok(json!({
            "status": "ok",
            "files_indexed": stats.file_count,
            "chunks": stats.chunk_count,
        }))
    }
}

#[async_trait::async_trait]
impl Tool for IndexStatus {
    fn name(&self) -> &str {
        "index_status"
    }
    fn description(&self) -> &str {
        "Show index stats: file count, chunk count, model, last update."
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let (root, model) = {
            let inner = ctx.agent.inner.read().await;
            let p = inner
                .active_project
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No active project. Use activate_project first."))?;
            (p.root.clone(), p.config.embeddings.model.clone())
        };

        let db_path = crate::embed::index::db_path(&root);
        if !db_path.exists() {
            return Ok(json!({
                "indexed": false,
                "message": "No index found. Run index_project first.",
            }));
        }

        let conn = crate::embed::index::open_db(&root)?;
        let stats = crate::embed::index::index_stats(&conn)?;

        Ok(json!({
            "indexed": true,
            "model": model,
            "file_count": stats.file_count,
            "chunk_count": stats.chunk_count,
            "embedding_count": stats.embedding_count,
            "db_path": db_path.display().to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::embed::index;
    use crate::lsp::LspManager;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".code-explorer")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: Arc::new(LspManager::new()),
            },
        )
    }

    #[tokio::test]
    async fn index_status_no_index() {
        let (_dir, ctx) = project_ctx().await;
        let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["indexed"], false);
    }

    #[tokio::test]
    async fn index_status_with_data() {
        let (dir, ctx) = project_ctx().await;
        // Create the DB and insert some data
        let conn = index::open_db(dir.path()).unwrap();
        let chunk = crate::embed::schema::CodeChunk {
            id: None,
            file_path: "test.rs".to_string(),
            language: "rust".to_string(),
            content: "fn test() {}".to_string(),
            start_line: 1,
            end_line: 1,
            file_hash: "abc".to_string(),
        };
        index::insert_chunk(&conn, &chunk, &[0.1, 0.2, 0.3]).unwrap();
        index::upsert_file_hash(&conn, "test.rs", "abc").unwrap();
        drop(conn);

        let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["indexed"], true);
        assert_eq!(result["file_count"], 1);
        assert_eq!(result["chunk_count"], 1);
        assert_eq!(result["embedding_count"], 1);
    }

    #[tokio::test]
    async fn tools_error_without_project() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: Arc::new(LspManager::new()),
        };
        assert!(SemanticSearch
            .call(json!({ "query": "test" }), &ctx)
            .await
            .is_err());
        assert!(IndexProject.call(json!({}), &ctx).await.is_err());
        assert!(IndexStatus.call(json!({}), &ctx).await.is_err());
    }

    #[tokio::test]
    async fn index_stats_function() {
        let dir = tempdir().unwrap();
        let conn = index::open_db(dir.path()).unwrap();
        let stats = index::index_stats(&conn).unwrap();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.chunk_count, 0);
        assert_eq!(stats.embedding_count, 0);
    }
}
