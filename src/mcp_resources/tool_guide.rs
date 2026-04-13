use std::sync::Arc;

use async_trait::async_trait;

use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use crate::tools::Tool;

const URI: &str = "doc://codescout-tool-guide";

pub struct ToolGuideProvider {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolGuideProvider {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools }
    }

    fn render(&self) -> String {
        let mut s = String::from(
            "# Codescout tool guide\n\n\
             Long-form usage notes. Short descriptions live in the MCP tool list; \
             this resource holds examples and 'when to use this vs. that' prose. \
             Fetched on demand via `resources/read doc://codescout-tool-guide`.\n\n",
        );
        for t in &self.tools {
            s.push_str(&format!("## {}\n\n", t.name()));
            if let Some(docs) = t.long_docs() {
                s.push_str(docs);
                s.push_str("\n\n");
            } else {
                // Fall back to the short description if no long_docs provided.
                s.push_str(t.description());
                s.push_str("\n\n");
            }
        }
        s
    }
}

#[async_trait]
impl ResourceProvider for ToolGuideProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![ResourceDescriptor {
            uri: URI.into(),
            name: "codescout-tool-guide".into(),
            description: Some("Extended usage notes for every codescout tool.".into()),
            mime_type: "text/markdown".into(),
        }]
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        if uri != URI {
            return Err(ResourceError::NotFound(uri.into()));
        }
        Ok(ResourceBytes::Text(self.render()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubTool {
        name: &'static str,
        desc: &'static str,
        long: Option<&'static str>,
    }

    #[async_trait::async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            self.desc
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        async fn call(
            &self,
            _i: serde_json::Value,
            _c: &crate::tools::ToolContext,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({}))
        }

        fn long_docs(&self) -> Option<&str> {
            self.long
        }
    }

    #[tokio::test]
    async fn tool_guide_renders_all_tools() {
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(StubTool {
                name: "alpha",
                desc: "short alpha",
                long: Some("long alpha details"),
            }),
            Arc::new(StubTool {
                name: "beta",
                desc: "short beta",
                long: None,
            }),
        ];
        let p = ToolGuideProvider::new(tools);
        let bytes = p.read("doc://codescout-tool-guide").await.unwrap();
        match bytes {
            ResourceBytes::Text(s) => {
                assert!(s.contains("## alpha"));
                assert!(s.contains("long alpha details"));
                assert!(s.contains("## beta"));
                assert!(s.contains("short beta")); // fallback
            }
            _ => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn tool_guide_not_found_for_wrong_uri() {
        let p = ToolGuideProvider::new(vec![]);
        let result = p.read("doc://something-else").await;
        assert!(matches!(result, Err(ResourceError::NotFound(_))));
    }
}
