use anyhow::Result;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo, Tool as McpTool,
    },
    service::RequestContext,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use std::sync::Arc;

use crate::tools::{all_tools, Tool, ToolContext};

#[derive(Clone)]
pub struct LibrarianServer {
    ctx: Arc<ToolContext>,
    tools: Arc<Vec<Arc<dyn Tool>>>,
}

impl LibrarianServer {
    pub fn new(ctx: ToolContext) -> Self {
        Self {
            ctx: Arc::new(ctx),
            tools: Arc::new(all_tools()),
        }
    }

    pub async fn serve_stdio(self) -> Result<()> {
        let (stdin, stdout) = rmcp::transport::stdio();
        self.serve((stdin, stdout)).await?.waiting().await?;
        Ok(())
    }
}

const INSTRUCTIONS: &str = include_str!("prompts/server_instructions.md");

impl ServerHandler for LibrarianServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tools = self
            .tools
            .iter()
            .map(|t| {
                let schema = t.input_schema();
                let schema_obj = schema.as_object().cloned().unwrap_or_default();
                McpTool::new(t.name().to_owned(), t.description().to_owned(), schema_obj)
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == req.name.as_ref())
            .ok_or_else(|| {
                McpError::invalid_params(format!("unknown tool `{}`", req.name), None)
            })?;
        let args = serde_json::Value::Object(req.arguments.unwrap_or_default());
        match tool.call(&self.ctx, args).await {
            Ok(v) => {
                let text = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => {
                let hint = e
                    .downcast_ref::<serde_json::Error>()
                    .map(|se| {
                        format!(
                            "input deserialization failed (check types vs tool schema): {}. ",
                            se
                        )
                    })
                    .unwrap_or_default();
                Ok(CallToolResult::error(vec![Content::text(format!(
                    "{hint}error: {e:#}"
                ))]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
        }
    }

    #[tokio::test]
    async fn serde_error_gets_helpful_hint() {
        use crate::tools::get::ArtifactGet;
        let ctx = mk_ctx();
        // Pass a string where a bool is expected — serde will reject it.
        let err = ArtifactGet
            .call(
                &ctx,
                serde_json::json!({
                    "id": "x",
                    "include_links": "true"
                }),
            )
            .await
            .unwrap_err();
        let s = err.to_string();
        assert!(
            s.to_lowercase().contains("bool") || s.to_lowercase().contains("string"),
            "expected type-hint in error, got: {s}"
        );
    }
}
