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
        for &topic in crate::prompts::GUIDE_TOPICS {
            if let Some(body) = crate::prompts::topic_body(topic) {
                topics.insert(topic, body);
            }
        }
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
        "Deep guidance for a topic. Use when the system prompt points here. \
         Topics: librarian | librarian-runtime | tracker-conventions | progressive-disclosure | \
         error-handling | workspace-state | iron-laws-detail | \
         symbol-navigation. No args = list \
         topics + summaries. Large bodies overflow to @tool_* buffer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "Topic to fetch. Omit to list available topics.",
                    "enum": crate::prompts::GUIDE_TOPICS
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
                    "librarian-runtime": "caps, scope hints, SQL filter semantics, gather sources, catalog DB location, classifier overrides, event-authorship",
                    "tracker-conventions": "frontmatter, archive flow, status vocabulary",
                    "progressive-disclosure": "MAX_INLINE_TOKENS, @ref buffer, overflow patterns",
                    "error-handling": "RecoverableError vs anyhow::bail, is_error routing",
                    "workspace-state": "activate_project semantics, home/foreign, per-session reset, subagent inheritance",
                    "iron-laws-detail": "per-law gate text, exceptions, edge cases for Iron Laws 1-6",
                    "symbol-navigation": "per-language symbols/references/call_graph nav tips"
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
            workspace_override: None,
        }
    }

    #[tokio::test]
    async fn get_guide_lists_topics_with_no_arg() {
        let g = GetGuide::new();
        let result = g.call(json!({}), &ctx().await).await.unwrap();
        let topics = result["topics"].as_array().unwrap();
        let names: Vec<&str> = topics.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"librarian"));
        assert!(names.contains(&"librarian-runtime"));
        assert!(names.contains(&"tracker-conventions"));
        assert!(names.contains(&"progressive-disclosure"));
        assert!(names.contains(&"error-handling"));
        assert!(names.contains(&"workspace-state"));
        assert!(names.contains(&"iron-laws-detail"));
        assert!(names.contains(&"symbol-navigation"));
        assert_eq!(names.len(), 8);
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

    #[tokio::test]
    async fn every_topic_has_non_empty_body() {
        // Drift guard: every topic registered in GetGuide::new() must point at
        // an include_str! that yields a non-empty (substantive) body. Catches
        // the "add a topic, point it at the wrong/empty file" mistake at test
        // time rather than at session time when an LLM gets back "".
        let g = GetGuide::new();
        let list = g.call(json!({}), &ctx().await).await.unwrap();
        let topics = list["topics"]
            .as_array()
            .expect("topics array in no-arg response");
        assert!(
            !topics.is_empty(),
            "GetGuide must register at least one topic"
        );

        for topic in topics {
            let name = topic.as_str().unwrap();
            let result = g
                .call(json!({"topic": name}), &ctx().await)
                .await
                .unwrap_or_else(|e| panic!("topic '{name}' failed: {e}"));
            let body = result["body"]
                .as_str()
                .unwrap_or_else(|| panic!("topic '{name}' returned no body field"));
            assert!(
                body.len() > 100,
                "topic '{name}' body suspiciously short ({} bytes) — likely empty or wrong include_str! target",
                body.len()
            );
        }
    }

    #[tokio::test]
    async fn schema_enum_matches_registered_topics() {
        // Drift guard: the input_schema's `topic` enum must list exactly the
        // topics in GetGuide::topics. Otherwise a new topic works at runtime
        // but isn't advertised in the schema (silent invisibility to clients
        // that validate against the schema), or vice versa.
        use std::collections::BTreeSet;
        let g = GetGuide::new();

        let schema = g.input_schema();
        let enum_arr = schema["properties"]["topic"]["enum"]
            .as_array()
            .expect("schema must have properties.topic.enum");
        let schema_topics: BTreeSet<String> = enum_arr
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        let list = g.call(json!({}), &ctx().await).await.unwrap();
        let registered_topics: BTreeSet<String> = list["topics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert_eq!(
            schema_topics, registered_topics,
            "input_schema enum drifted from GetGuide::topics map — add the new topic to both or neither"
        );
    }
}
