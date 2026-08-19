pub mod db;

use crate::agent::Agent;
use anyhow::Result;
use rmcp::model::Content;
use serde_json::Value;
use std::time::Instant;

pub struct UsageRecorder {
    agent: Agent,
    debug: bool,
    /// This MCP server process's own id.
    session_id: String,
    /// The Claude Code session id, resolved by the server and passed in.
    ///
    /// This used to be read here from `.codescout/cc_session_id` on every write.
    /// That file is per-PROJECT, so two concurrent Claude Code sessions both
    /// recorded under whichever id was written last, and every per-session
    /// figure silently merged them. The server already resolves this correctly
    /// (`CLAUDE_CODE_SESSION_ID` first, which is per-process); taking it from
    /// there gives the value one resolution site instead of two that drifted.
    /// docs/issues/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md
    cc_session_id: String,
}

impl UsageRecorder {
    pub fn new(agent: Agent, debug: bool, session_id: String, cc_session_id: String) -> Self {
        Self {
            agent,
            debug,
            session_id,
            cc_session_id,
        }
    }

    /// Record a tool call's telemetry against the project named by
    /// `workspace_override`, falling back to the session default when `None`.
    /// The pin MUST match the one the tool body itself resolved, or a pinned
    /// call's stats land in the wrong project's `usage.db` (see
    /// `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`,
    /// finding 4).
    pub async fn record_content<F, Fut>(
        &self,
        tool_name: &str,
        input: &Value,
        workspace_override: Option<&std::path::Path>,
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
            .write_content(tool_name, latency_ms, input, workspace_override, &result)
            .await;
        result
    }

    async fn write_content(
        &self,
        tool_name: &str,
        latency_ms: i64,
        input: &Value,
        workspace_override: Option<&std::path::Path>,
        result: &Result<Vec<Content>>,
    ) -> Result<()> {
        let (project_root, head_sha) = self
            .agent
            .with_project_at(workspace_override, |p| {
                Ok((p.root.clone(), p.head_sha.clone()))
            })
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
        let err_family = error_msg
            .as_deref()
            .and_then(|m| db::normalize_err_family(tool_name, m));
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

        // Resolved once by the server, not re-derived here — see the field doc.
        let cc_session_id = Some(self.cc_session_id.as_str()).filter(|s| !s.is_empty());

        db::write_record(
            &conn,
            tool_name,
            latency_ms,
            outcome,
            overflowed,
            error_msg.as_deref(),
            // The sha AND its dirty bit, as one value. Passing the bare sha env var here
            // is what BL-24 was: a lone sha is not an identity, and a `&str` routes
            // through `From<&str>`, which assumes the tree was clean. Pinned by
            // `db::tests::the_recorder_never_assumes_a_clean_build`, which scans this
            // file — so do not name that env var here, even in a comment.
            db::BuildProvenance::current(),
            head_sha.as_deref(),
            &self.session_id,
            input_json.as_deref(),
            output_json.as_deref(),
            cc_session_id,
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
///
/// `file_path` and `rel_path` are included because they are documented **aliases** of
/// `path` on the tools that accept them, not separate concepts — so they sit immediately
/// after it, and the canonical spelling still wins if a caller sends both. Omitting them
/// cost 57 error rows their target on this project alone (measured 2026-08-20; all 51
/// `file_path` rows carried no `path` at all), each one a file
/// `legibility::recorder_lane` could not join to a candidate.
///
/// Deliberately absent: `command`. It is the largest target-less population by volume
/// (438 rows) and is still not a target — see
/// `extract_friction_target_ignores_shell_commands` for the reasoning, which is a
/// decision rather than an oversight.
fn extract_friction_target(input: &Value) -> Option<String> {
    const KEYS: [&str; 8] = [
        "name_path",
        "symbol",
        "name",
        "query",
        "path",
        "file_path",
        "rel_path",
        "pattern",
    ];
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

    /// `file_path` and `rel_path` are documented ALIASES of `path` on the tools that
    /// accept them (`read_file`, `edit_file`, `grep`, `create_file`, `read_markdown`,
    /// `edit_markdown`, and `artifact`'s `rel_path`). Extracting only `path` means a call
    /// that spelled it the other way records no target at all.
    ///
    /// Measured 2026-08-20 on this project's own `usage.db`: **51 error rows carried
    /// `file_path` and NONE of them also carried `path`** — `read_file` 31, `edit_file` 10,
    /// `read_markdown` 4, `edit_markdown` 4, `edit_code` 2 — plus 6 `artifact` rows
    /// carrying `rel_path`. Every one is a file target that `legibility::recorder_lane`
    /// should have been able to join to a `rel_file` candidate and could not.
    #[test]
    fn extract_friction_target_reads_the_documented_path_aliases() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"file_path": "src/x.rs"})),
            Some("src/x.rs".to_string()),
            "file_path is a documented alias of path and must yield a target"
        );
        assert_eq!(
            extract_friction_target(&json!({"rel_path": "docs/trackers/foo.md"})),
            Some("docs/trackers/foo.md".to_string()),
            "rel_path is artifact's spelling of the same concept"
        );
        // An alias must not outrank the more specific keys, or a symbol-addressed call
        // that also names a file would be attributed to the file.
        assert_eq!(
            extract_friction_target(&json!({"name_path": "A/b", "file_path": "src/x.rs"})),
            Some("A/b".to_string()),
            "name_path still wins over an alias"
        );
        // `path` and `file_path` are the same concept, so which one wins cannot matter for
        // correctness — but pin it so the order is a decision rather than an accident.
        assert_eq!(
            extract_friction_target(&json!({"path": "a.rs", "file_path": "b.rs"})),
            Some("a.rs".to_string()),
            "canonical `path` is preferred when a caller sends both spellings"
        );
    }

    /// `command` is deliberately NOT a friction target, and this test is the record of
    /// that decision rather than an oversight.
    ///
    /// `run_command` accounts for 438 of the 596 target-less error rows (2026-08-20), so
    /// adding `command` would close most of the gap by volume. It is still wrong:
    ///
    /// * The field is documented as *the symbol/path a call addressed*. A shell command is
    ///   neither, and the sole consumer — `legibility::score_and_rank` — looks friction up
    ///   by `name_path` then `rel_file`, so a command string is inert there: never matched,
    ///   never surfaced, pure storage.
    /// * A whole command varies by flags, so it groups badly; the executable name groups
    ///   well but discards what was addressed. Neither is *the target*.
    /// * `input_json` is populated on ~99% of rows, so the command is already recoverable
    ///   at query time. Storing a derived form buys nothing and makes one column mean two
    ///   things.
    ///
    /// If a consumer ever needs per-command grouping, give it its own column rather than
    /// widening this one's contract.
    #[test]
    fn extract_friction_target_ignores_shell_commands() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"command": "cargo test --lib"})),
            None,
            "a shell command is not a symbol or a path — see this test's doc comment"
        );
        // ...but a run_command that DOES name a cwd path still yields nothing, because cwd
        // is the directory the command ran in, not the thing it addressed.
        assert_eq!(
            extract_friction_target(&json!({"command": "ls", "cwd": "src/"})),
            None,
            "cwd is where the call ran, not what it addressed"
        );
    }

    /// Regression for
    /// docs/issues/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md:
    /// the recorder used to read `.codescout/cc_session_id` itself. That file is
    /// per-PROJECT, so with two Claude Code sessions open on one repo both wrote
    /// rows under whichever id the file held last and per-session figures merged
    /// them silently. The id the server resolved must win over the file.
    #[tokio::test]
    async fn record_content_uses_the_passed_cc_session_id_not_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        // A file holding a DIFFERENT session's id — the concurrent-session case.
        std::fs::write(
            dir.path().join(".codescout").join("cc_session_id"),
            "other-session-from-the-file",
        )
        .unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "mcp-session".to_string(),
            "my-cc-session".to_string(),
        );

        let _ = recorder
            .record_content(
                "symbols",
                &serde_json::json!({"query": "x"}),
                None,
                || async { Ok(vec![Content::text("ok")]) },
            )
            .await;

        let db = dir.path().join(".codescout").join("usage.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        let got: String = conn
            .query_row(
                "SELECT cc_session_id FROM tool_calls ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            got, "my-cc-session",
            "the server-resolved id must win; reading the shared per-project file \
             is what merged concurrent sessions into one"
        );
    }

    #[tokio::test]
    async fn record_content_stores_input_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            true,
            "test-session".to_string(),
            "cc-test".to_string(),
        );
        let input = json!({"query": "test_symbol", "path": "src/lib.rs"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
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
    async fn record_content_honors_workspace_override_pin() {
        // BUG (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md,
        // finding 4): write_content resolved the usage-db root via the plain,
        // unpinned with_project. call_tool_inner already computed the pin and
        // threaded it into check_tool_access/timeout_secs/the write guard — but
        // not into the recorder, so EVERY pinned call's telemetry silently
        // landed in the session-default project's usage.db.
        use serde_json::json;

        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let canon_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B; pin THIS call to A.
        let agent = crate::agent::Agent::new(Some(dir_b.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent,
            false,
            "pin-session".to_string(),
            "cc-pin".to_string(),
        );
        let input = json!({"query": "x"});

        let _ = recorder
            .record_content("symbols", &input, Some(&canon_a), || async {
                Ok(vec![Content::text("ok")])
            })
            .await;

        let rows_in = |root: &std::path::Path| -> i64 {
            let conn = crate::usage::db::open_db(root).unwrap();
            conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(
            rows_in(&canon_a),
            1,
            "the pinned call's telemetry must land in workspace A's usage.db"
        );
        assert_eq!(
            rows_in(dir_b.path()),
            0,
            "it must NOT land in the session-default workspace B's usage.db"
        );
    }

    #[tokio::test]
    async fn record_content_stores_output_for_errors_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            true,
            "test-session".to_string(),
            "cc-test".to_string(),
        );
        let input = json!({"path": "/bad/path"});

        let _ = recorder
            .record_content("read_file", &input, None, || async {
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
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
        );
        let input = json!({"query": "test_symbol"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
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
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
        );
        let input = json!({"name_path": "LspManager/get_or_start", "path": "src/lsp/manager.rs"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
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
