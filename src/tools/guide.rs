//! `get_guide(topic)` tool — returns deep guidance text as the tool result.
//!
//! Topics are content files embedded at build time. See
//! `docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md`
//! for the design.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::tools::core::{RecoverableError, Tool, ToolContext};

pub struct GetGuide {
    topics: BTreeMap<&'static str, &'static str>,
}

impl GetGuide {
    pub fn new() -> Self {
        let mut topics: BTreeMap<&'static str, &'static str> = BTreeMap::new();
        topics.insert("librarian", include_str!("../prompts/guides/librarian.md"));
        topics.insert(
            "tracker-conventions",
            include_str!("../prompts/guides/tracker-conventions.md"),
        );
        topics.insert(
            "progressive-disclosure",
            include_str!("../prompts/guides/progressive-disclosure.md"),
        );
        topics.insert(
            "error-handling",
            include_str!("../prompts/guides/error-handling.md"),
        );
        Self { topics }
    }
}

impl Default for GetGuide {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for GetGuide {
    fn name(&self) -> &str {
        "get_guide"
    }

    fn description(&self) -> &str {
        "Fetch deep guidance for a topic. Returns full text (large topics \
         overflow to @tool_* buffer). Use when the system prompt points \
         here (e.g. \"see get_guide('librarian')\"). Topics: librarian | \
         tracker-conventions | progressive-disclosure | error-handling. \
         No args = list topics + summaries."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "Topic to fetch. Omit to list available topics.",
                    "enum": ["librarian", "tracker-conventions",
                             "progressive-disclosure", "error-handling"]
                }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let topic = input.get("topic").and_then(|v| v.as_str());
        match topic {
            None => Ok(json!({
                "topics": self.topics.keys().collect::<Vec<_>>(),
                "summaries": {
                    "librarian": "artifact model, filter syntax, trackers, augmentations",
                    "tracker-conventions": "frontmatter, archive flow, status vocabulary",
                    "progressive-disclosure": "MAX_INLINE_TOKENS, @ref buffer, overflow patterns",
                    "error-handling": "RecoverableError vs anyhow::bail, is_error routing"
                }
            })),
            Some(t) => match self.topics.get(t) {
                Some(body) => Ok(json!({ "topic": t, "body": *body })),
                None => {
                    let available = self.topics.keys().cloned().collect::<Vec<_>>().join(", ");
                    Err(RecoverableError::with_hint(
                        format!("unknown topic '{t}'"),
                        format!("available topics: {available}"),
                    )
                    .into())
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn ctx() -> ToolContext {
        ToolContext {
            agent: crate::agent::Agent::new(None).await.unwrap(),
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        }
    }

    #[tokio::test]
    async fn get_guide_lists_topics_with_no_arg() {
        let g = GetGuide::new();
        let result = g.call(json!({}), &ctx().await).await.unwrap();
        let topics = result["topics"].as_array().unwrap();
        let names: Vec<&str> = topics.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"librarian"));
        assert!(names.contains(&"tracker-conventions"));
        assert!(names.contains(&"progressive-disclosure"));
        assert!(names.contains(&"error-handling"));
        assert_eq!(names.len(), 4);
    }

    #[tokio::test]
    async fn get_guide_returns_librarian_body() {
        let g = GetGuide::new();
        let result = g
            .call(json!({"topic": "librarian"}), &ctx().await)
            .await
            .unwrap();
        assert_eq!(result["topic"].as_str(), Some("librarian"));
        let body = result["body"].as_str().unwrap();
        assert!(!body.is_empty());
        assert!(
            body.contains("artifact"),
            "should mention artifact in librarian guide"
        );
    }

    #[tokio::test]
    async fn get_guide_unknown_topic_is_recoverable() {
        let g = GetGuide::new();
        let err = g
            .call(json!({"topic": "nonexistent"}), &ctx().await)
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<RecoverableError>()
            .expect("should be RecoverableError");
        assert!(rec.message.contains("unknown topic"));
        assert!(rec.hint().unwrap().contains("librarian"));
    }
}
