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

use crate::librarian::tools::{all_tools, Tool, ToolContext};

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

impl ServerHandler for LibrarianServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Librarian — artifact + tracker + memory management for the active workspace. See `get_guide(\"librarian\")` when used inside codescout, or the tool descriptions for standalone usage.")
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
        Ok(map_tool_result(tool.call(&self.ctx, args).await))
    }
}

/// Map a tool result to an MCP `CallToolResult`. Recoverable errors become
/// `success` results with an `{"error": "...", "hint": "..."}` JSON body so
/// sibling parallel tool calls survive (Claude Code aborts batched calls when
/// it sees `isError: true`). Other errors stay on the `isError: true` path.
pub(crate) fn map_tool_result(r: Result<serde_json::Value>) -> CallToolResult {
    match r {
        Ok(v) => {
            let text = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
            CallToolResult::success(vec![Content::text(text)])
        }
        Err(e) => {
            if let Some(rec) = e.downcast_ref::<crate::librarian::tools::RecoverableError>() {
                let mut body = serde_json::json!({ "error": rec.message });
                if let Some(h) = &rec.hint {
                    body["hint"] = serde_json::Value::String(h.clone());
                }
                let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
                return CallToolResult::success(vec![Content::text(text)]);
            }
            let hint = e
                .downcast_ref::<serde_json::Error>()
                .map(|se| {
                    format!(
                        "input deserialization failed (check types vs tool schema): {}. ",
                        se
                    )
                })
                .unwrap_or_default();
            CallToolResult::error(vec![Content::text(format!("{hint}error: {e:#}"))])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::WorkspaceConfig;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
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
    async fn serde_error_gets_helpful_hint() {
        use crate::librarian::tools::artifact::Artifact;
        use crate::librarian::tools::Tool;
        let ctx = mk_ctx();
        // Pass a string where a bool is expected — serde will reject it.
        let err = Artifact
            .call(
                &ctx,
                serde_json::json!({
                    "action": "get",
                    "id": "x",
                    "include_links": "true"   // should be bool
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

    #[test]
    fn map_tool_result_recoverable_returns_success_with_error_body() {
        use crate::librarian::tools::RecoverableError;
        let err = RecoverableError::with_hint("bad input", "use foo");
        let res = map_tool_result(Err(err));
        // Recoverable → success, no isError flag set
        assert!(!res.is_error.unwrap_or(false));
        let text = match &res.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("expected text"),
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["error"], "bad input");
        assert_eq!(v["hint"], "use foo");
    }

    #[test]
    fn map_tool_result_anyhow_returns_is_error_true() {
        let err = anyhow::anyhow!("boom");
        let res = map_tool_result(Err(err));
        assert_eq!(res.is_error, Some(true));
    }
}
