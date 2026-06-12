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
         topics + summaries. The full guide is always returned inline, \
         never buffered to a @ref."
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

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
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
                Some(body) => {
                    // Participate in the per-session `guide_hints_emitted` ledger so
                    // explicit fetches and auto-injected hints share one keyspace:
                    //  - first fetch of `t` marks it emitted, so a later auto-inject of
                    //    the same topic is suppressed (and an auto-inject suppresses a
                    //    later explicit fetch's "first" status);
                    //  - a repeat fetch is flagged so the model re-reads its existing
                    //    copy instead of re-spending context on static content.
                    // The body is NEVER withheld: the ledger is not cleared on `/compact`,
                    // so a legitimate post-compaction re-fetch must still return the guide.
                    // `insert` returns false when the topic was already present.
                    let first_fetch = ctx.guide_hints_emitted.lock().insert(t.to_string());
                    let note = if first_fetch {
                        format!(
                            "This guide is static and now in your context. Don't re-call \
                             get_guide(\"{t}\") this session unless your context was compacted."
                        )
                    } else {
                        format!(
                            "You already fetched get_guide(\"{t}\") earlier this session. This \
                             guide is static — if the earlier copy is still in your context, no \
                             need to re-read it. (Re-fetch is only needed after compaction.)"
                        )
                    };
                    Ok(json!({ "topic": t, "body": *body, "note": note }))
                }
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

    fn force_inline(&self) -> bool {
        // A guide is documentation the agent explicitly asked to READ; handing
        // back a `@tool_*` buffer reference defeats that. Always return the full
        // body inline, regardless of size.
        true
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
    async fn get_guide_large_topic_returns_full_body_inline_not_buffered() {
        // Regression: get_guide must return the ENTIRE guide inline regardless of
        // size — never a `@tool_*` buffer handle. The `librarian` topic is ~14 KB,
        // well above the ~10 KB (MAX_INLINE_TOKENS * 4) inline-buffer threshold, so
        // without GetGuide's `force_inline()` override, call_content's overflow
        // branch would divert it to the output buffer and return only a ref handle.
        let g = GetGuide::new();
        let ctx = ctx().await;

        // Sanity: the body must actually exceed the inline threshold, otherwise
        // this test would still pass even if `force_inline()` were removed.
        let val = g.call(json!({"topic": "librarian"}), &ctx).await.unwrap();
        let json_len = serde_json::to_string(&val).unwrap().len();
        assert!(
            json_len > 10_000,
            "librarian guide must exceed the ~10 KB inline threshold for this \
             test to be meaningful, got {json_len} bytes"
        );

        let content = g
            .call_content(json!({"topic": "librarian"}), &ctx)
            .await
            .unwrap();
        assert_eq!(content.len(), 1, "guide must be a single inline block");
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.contains("@tool_"),
            "guide must NOT be diverted to a @tool_ buffer handle, got: {}",
            &text[..text.len().min(200)]
        );
        assert!(
            text.contains("artifact"),
            "the full librarian guide body must be present inline"
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

    #[tokio::test]
    async fn repeat_fetch_keeps_body_and_flags_static() {
        // Regression (docs/issues/2026-06-11-get-guide-no-session-dedup): get_guide
        // must participate in the guide_hints_emitted ledger. Two fetches of the same
        // topic in one session (SHARED ctx) must (1) both return the full body — never
        // withhold, so post-/compact recovery still works — and (2) carry a note that
        // flips from "don't re-call" on the first fetch to "already fetched" on the repeat.
        let g = GetGuide::new();
        let tc = ctx().await;

        let first = g
            .call(json!({"topic": "tracker-conventions"}), &tc)
            .await
            .unwrap();
        assert!(!first["body"].as_str().unwrap().is_empty());
        let first_note = first["note"].as_str().expect("first fetch has a note");
        assert!(
            first_note.contains("Don't re-call"),
            "first fetch should discourage re-calling, got: {first_note}"
        );

        // The fetch registered the topic in the shared ledger.
        assert!(tc
            .guide_hints_emitted
            .lock()
            .contains("tracker-conventions"));

        let second = g
            .call(json!({"topic": "tracker-conventions"}), &tc)
            .await
            .unwrap();
        // Body is still returned in full on the repeat — never a stub.
        assert_eq!(
            second["body"].as_str(),
            first["body"].as_str(),
            "repeat fetch must return the identical full body, not a stub"
        );
        let second_note = second["note"].as_str().expect("repeat fetch has a note");
        assert!(
            second_note.contains("already fetched"),
            "repeat fetch note should flag the prior fetch, got: {second_note}"
        );
    }
}
