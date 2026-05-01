//! `call_graph` — transitive call graph for a symbol (stub).

use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};

pub struct CallGraph;

#[async_trait::async_trait]
impl Tool for CallGraph {
    fn name(&self) -> &str {
        "call_graph"
    }

    fn description(&self) -> &str {
        "Transitive call graph for a symbol. Direction `callers` (blast radius) \
         or `callees` (flow) or `both`. NOT YET IMPLEMENTED — see \
         docs/socraticode-borrow-tracker.md (item A)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol":    { "type": "string" },
                "direction": { "enum": ["callers", "callees", "both"], "default": "callers" },
                "max_depth": { "type": "integer", "default": 3 }
            },
            "required": ["symbol"]
        })
    }

    async fn call(&self, _input: Value, _ctx: &ToolContext) -> Result<Value> {
        Err(RecoverableError::new(
            "call_graph is not yet implemented. \
             Tracked in docs/socraticode-borrow-tracker.md (item A). \
             Use `references` for one-hop call sites in the meantime.",
        )
        .into())
    }

    fn format_compact(&self, _result: &Value) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lsp() -> std::sync::Arc<dyn crate::lsp::LspProvider> {
        crate::lsp::LspManager::new_arc()
    }

    fn buf() -> std::sync::Arc<crate::tools::output_buffer::OutputBuffer> {
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20))
    }

    async fn minimal_ctx() -> ToolContext {
        use crate::agent::Agent;
        use std::sync::Arc;

        let agent = Agent::new(None).await.unwrap();
        ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: buf(),
            progress: None,
            peer: None,
            section_coverage: Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        }
    }

    #[tokio::test]
    async fn call_graph_stub_returns_recoverable_error() {
        let ctx = minimal_ctx().await;
        let err = CallGraph
            .call(json!({ "symbol": "foo" }), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }
}
