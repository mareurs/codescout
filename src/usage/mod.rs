pub mod db;

use crate::agent::Agent;
use anyhow::Result;
use rmcp::model::Content;
use serde_json::Value;
use std::time::Instant;

pub struct UsageRecorder {
    agent: Agent,
    debug: bool,
    session_id: String,
}

impl UsageRecorder {
    pub fn new(agent: Agent, debug: bool, session_id: String) -> Self {
        Self {
            agent,
            debug,
            session_id,
        }
    }

    pub async fn record_content<F, Fut>(
        &self,
        tool_name: &str,
        input: &Value,
        f: F,
    ) -> Result<Vec<Content>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Content>>>,
    {
        let start = Instant::now();
        let result = f().await;
        let latency_ms = start.elapsed().as_millis() as i64;
        // Best-effort — never let recording fail the tool call
        let _ = self
            .write_content(tool_name, latency_ms, input, &result)
            .await;
        result
    }

    async fn write_content(
        &self,
        tool_name: &str,
        latency_ms: i64,
        input: &Value,
        result: &Result<Vec<Content>>,
    ) -> Result<()> {
        let (project_root, head_sha) = self
            .agent
            .with_project(|p| Ok((p.root.clone(), p.head_sha.clone())))
            .await?;
        let conn = db::open_db(&project_root)?;
        let (outcome, overflowed, error_msg) = classify_content_result(result);

        // Friction fields (Phase 1 of the legibility probe).
        let is_friction = overflowed || outcome != "success";
        let friction_target = if is_friction {
            extract_friction_target(input)
        } else {
            None
        };
        let overflow_tokens = if overflowed {
            extract_overflow_tokens(result)
        } else {
            None
        };
        let err_family = error_msg.as_deref().and_then(db::normalize_err_family);
        let project_root_str = project_root.to_string_lossy().to_string();

        let input_json = if self.debug {
            serde_json::to_string(input).ok()
        } else {
            None
        };

        let output_json = if self.debug {
            match result {
                Ok(blocks) => serde_json::to_string(blocks).ok(),
                Err(e) => Some(serde_json::json!({"error": e.to_string()}).to_string()),
            }
        } else {
            None
        };

        let cc_session_id =
            std::fs::read_to_string(project_root.join(".codescout").join("cc_session_id"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

        db::write_record(
            &conn,
            tool_name,
            latency_ms,
            outcome,
            overflowed,
            error_msg.as_deref(),
            env!("CODESCOUT_GIT_SHA"),
            head_sha.as_deref(),
            &self.session_id,
            input_json.as_deref(),
            output_json.as_deref(),
            cc_session_id.as_deref(),
            friction_target.as_deref(),
            overflow_tokens,
            err_family,
            Some(project_root_str.as_str()),
        )?;
        Ok(())
    }
}

fn classify_content_result(result: &Result<Vec<Content>>) -> (&'static str, bool, Option<String>) {
    match result {
        Err(e) => ("error", false, Some(e.to_string())),
        Ok(blocks) => {
            // Parse the text of the first content block as JSON and inspect it
            // for the same "error" / "overflow" sentinel keys that classify_result uses.
            let text = blocks
                .first()
                .and_then(|c| c.as_text())
                .map(|t| t.text.as_str())
                .unwrap_or("");
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                if let Some(msg) = v.get("error").and_then(Value::as_str) {
                    return ("recoverable_error", false, Some(msg.to_string()));
                }
                if v.get("output_id").is_some() {
                    return ("success", true, None);
                }
            }
            ("success", false, None)
        }
    }
}

/// Token estimate of a buffered (overflowed) result: `buffered_bytes / 4`.
fn extract_overflow_tokens(result: &Result<Vec<Content>>) -> Option<i64> {
    let blocks = result.as_ref().ok()?;
    let text = blocks
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())?;
    let v: Value = serde_json::from_str(text).ok()?;
    let bytes = v.get("buffered_bytes").and_then(Value::as_i64)?;
    Some(bytes / 4)
}

/// The symbol/path a call addressed, for friction attribution. Priority order:
/// the most specific address first (name_path/symbol), then name, then path/query/pattern.
fn extract_friction_target(input: &Value) -> Option<String> {
    const KEYS: [&str; 6] = ["name_path", "symbol", "name", "query", "path", "pattern"];
    for k in KEYS {
        if let Some(s) = input.get(k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod content_tests {
    use super::*;
    use rmcp::model::Content;

    #[test]
    fn classify_content_error_result() {
        let r: anyhow::Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("boom"));
    }

    #[test]
    fn classify_content_recoverable_error() {
        let text = serde_json::json!({"error": "path not found"}).to_string();
        let r: anyhow::Result<Vec<Content>> = Ok(vec![Content::text(text)]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "recoverable_error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn classify_detects_overflow_by_output_id_not_legacy_key() {
        // real overflow envelope marker
        let real = Ok(vec![Content::text(
            r#"{"output_id":"@tool_abc","summary":"...","buffered_bytes":12000}"#.to_string(),
        )]);
        let (_outcome, overflowed, _) = classify_content_result(&real);
        assert!(overflowed, "output_id envelope must set overflowed=true");

        // legacy key must NOT trigger (guards the exact wrong-key regression)
        let legacy = Ok(vec![Content::text(r#"{"overflow":true}"#.to_string())]);
        let (_o2, overflowed_legacy, _) = classify_content_result(&legacy);
        assert!(
            !overflowed_legacy,
            "legacy 'overflow' key must not be treated as overflow"
        );

        // normal result
        let normal = Ok(vec![Content::text(r#"{"result":"ok"}"#.to_string())]);
        let (_o3, overflowed_normal, _) = classify_content_result(&normal);
        assert!(!overflowed_normal);
    }

    #[test]
    fn classify_content_clean_success() {
        let r: anyhow::Result<Vec<Content>> = Ok(vec![Content::text("plain text output")]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "success");
        assert!(!overflowed);
        assert!(msg.is_none());
    }

    #[test]
    fn classify_content_empty_blocks() {
        let r: anyhow::Result<Vec<Content>> = Ok(vec![]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "success");
        assert!(!overflowed);
        assert!(msg.is_none());
    }

    #[test]
    fn extract_overflow_tokens_reads_buffered_bytes_over_four() {
        let env = Ok(vec![Content::text(
            r#"{"output_id":"@tool_x","buffered_bytes":10000}"#.to_string(),
        )]);
        assert_eq!(extract_overflow_tokens(&env), Some(2500));

        let no_bytes = Ok(vec![Content::text(
            r#"{"output_id":"@tool_x"}"#.to_string(),
        )]);
        assert_eq!(extract_overflow_tokens(&no_bytes), None);

        let err: Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
        assert_eq!(extract_overflow_tokens(&err), None);
    }

    #[test]
    fn extract_friction_target_coalesces_input_keys() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"name_path": "A/b", "path": "src/x.rs"})),
            Some("A/b".to_string()),
            "name_path wins over path"
        );
        assert_eq!(
            extract_friction_target(&json!({"symbol": "Foo/bar"})),
            Some("Foo/bar".to_string())
        );
        assert_eq!(
            extract_friction_target(&json!({"path": "src/lib.rs"})),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(extract_friction_target(&json!({"unrelated": 1})), None);
    }

    #[tokio::test]
    async fn record_content_stores_input_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(agent.clone(), true, "test-session".to_string());
        let input = json!({"query": "test_symbol", "path": "src/lib.rs"});

        let _ = recorder
            .record_content("symbols", &input, || async {
                Ok(vec![Content::text("found it")])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, out, sid, cs): (Option<String>, Option<String>, String, String) = conn
            .query_row(
                "SELECT input_json, output_json, session_id, codescout_sha FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();

        assert!(
            inp.is_some(),
            "input_json should be populated in debug mode"
        );
        assert!(inp.unwrap().contains("test_symbol"));
        assert!(
            out.is_some(),
            "output_json should be populated in debug mode for all outcomes"
        );
        assert!(out.unwrap().contains("found it"));
        assert_eq!(sid, "test-session");
        assert!(!cs.is_empty(), "codescout_sha should be set");
    }

    #[tokio::test]
    async fn record_content_stores_output_for_errors_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(agent.clone(), true, "test-session".to_string());
        let input = json!({"path": "/bad/path"});

        let _ = recorder
            .record_content("read_file", &input, || async {
                Err(anyhow::anyhow!("file not found"))
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, out): (Option<String>, Option<String>) = conn
            .query_row("SELECT input_json, output_json FROM tool_calls", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();

        assert!(inp.is_some(), "input_json should be populated");
        assert!(out.is_some(), "output_json should be populated for errors");
        assert!(out.unwrap().contains("file not found"));
    }

    #[tokio::test]
    async fn record_content_no_input_in_normal_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(agent.clone(), false, "test-session".to_string());
        let input = json!({"query": "test_symbol"});

        let _ = recorder
            .record_content("symbols", &input, || async {
                Ok(vec![Content::text("found it")])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, sid, cs): (Option<String>, String, String) = conn
            .query_row(
                "SELECT input_json, session_id, codescout_sha FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert!(inp.is_none(), "input_json should be None in normal mode");
        assert_eq!(sid, "test-session", "session_id should always be set");
        assert!(!cs.is_empty(), "codescout_sha should always be set");
    }

    #[tokio::test]
    async fn record_content_populates_friction_fields_on_overflow() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(agent.clone(), false, "test-session".to_string());
        let input = json!({"name_path": "LspManager/get_or_start", "path": "src/lsp/manager.rs"});

        let _ = recorder
            .record_content("symbols", &input, || async {
                Ok(vec![Content::text(
                    r#"{"output_id":"@tool_x","summary":"...","buffered_bytes":10000}"#.to_string(),
                )])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (overflowed, ft, tok, pr): (i64, Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT overflowed, friction_target, overflow_tokens, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(overflowed, 1, "output_id envelope -> overflowed");
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(2500), "10000 bytes / 4");
        assert!(pr.is_some(), "project_root always set");
    }
}
