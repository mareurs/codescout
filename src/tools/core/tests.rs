use super::*;

#[test]
fn tool_context_has_progress_field() {
    // Compile-only test: ensures the progress field exists and has the right type.
    fn _check_progress_field_type(_ctx: &ToolContext) {
        let _p: &Option<std::sync::Arc<crate::tools::progress::ProgressReporter>> = &_ctx.progress;
    }
}

#[test]
fn parse_bool_param_handles_all_variants() {
    use serde_json::json;
    // Native JSON booleans
    assert!(parse_bool_param(&json!(true)));
    assert!(!parse_bool_param(&json!(false)));
    // String booleans (sent by Claude Code MCP client)
    assert!(parse_bool_param(&json!("true")));
    assert!(!parse_bool_param(&json!("false")));
    // Missing / null / wrong type → false
    assert!(!parse_bool_param(&json!(null)));
    assert!(!parse_bool_param(&json!(42)));
    assert!(!parse_bool_param(&json!("yes")));
}

#[test]
fn optional_bool_param_returns_none_when_absent() {
    use serde_json::json;
    assert_eq!(optional_bool_param(&json!({}), "flag"), None);
    assert_eq!(optional_bool_param(&json!({"flag": null}), "flag"), None);
}

#[test]
fn optional_bool_param_coerces_strings() {
    use serde_json::json;
    assert_eq!(optional_bool_param(&json!({"x": true}), "x"), Some(true));
    assert_eq!(optional_bool_param(&json!({"x": false}), "x"), Some(false));
    assert_eq!(optional_bool_param(&json!({"x": "true"}), "x"), Some(true));
    assert_eq!(
        optional_bool_param(&json!({"x": "false"}), "x"),
        Some(false)
    );
    assert_eq!(optional_bool_param(&json!({"x": "yes"}), "x"), None);
    assert_eq!(optional_bool_param(&json!({"x": 42}), "x"), None);
}

#[test]
fn optional_u64_param_coerces_strings() {
    use serde_json::json;
    assert_eq!(optional_u64_param(&json!({}), "n"), None);
    assert_eq!(optional_u64_param(&json!({"n": null}), "n"), None);
    assert_eq!(optional_u64_param(&json!({"n": 42}), "n"), Some(42));
    assert_eq!(optional_u64_param(&json!({"n": "42"}), "n"), Some(42));
    assert_eq!(optional_u64_param(&json!({"n": " 7 "}), "n"), Some(7));
    assert_eq!(optional_u64_param(&json!({"n": "abc"}), "n"), None);
    assert_eq!(optional_u64_param(&json!({"n": "-1"}), "n"), None);
}

#[test]
fn optional_array_param_returns_none_when_absent() {
    use serde_json::json;
    assert_eq!(optional_array_param(&json!({}), "a"), None);
    assert_eq!(optional_array_param(&json!({"a": null}), "a"), None);
}

#[test]
fn optional_array_param_native_array() {
    use serde_json::json;
    assert_eq!(
        optional_array_param(&json!({"a": [1, 2, 3]}), "a"),
        Some(vec![json!(1), json!(2), json!(3)])
    );
}

#[test]
fn optional_array_param_string_encoded_array() {
    use serde_json::json;
    // String-encoded JSON array of strings
    assert_eq!(
        optional_array_param(&json!({"a": "[\"x\",\"y\"]"}), "a"),
        Some(vec![json!("x"), json!("y")])
    );
    // String-encoded array of objects
    assert_eq!(
        optional_array_param(&json!({"a": "[{\"k\":1},{\"k\":2}]"}), "a"),
        Some(vec![json!({"k": 1}), json!({"k": 2})])
    );
    // Non-array string → None
    assert_eq!(
        optional_array_param(&json!({"a": "not an array"}), "a"),
        None
    );
    // String-encoded non-array JSON → None
    assert_eq!(optional_array_param(&json!({"a": "{}"}), "a"), None);
    // Number → None
    assert_eq!(optional_array_param(&json!({"a": 42}), "a"), None);
}

#[test]
fn recoverable_error_stores_message() {
    let e = RecoverableError::new("path not found");
    assert_eq!(e.message, "path not found");
    assert!(e.hint().is_none());
}

#[test]
fn recoverable_error_stores_hint() {
    let e = RecoverableError::with_hint("path not found", "use tree to explore");
    assert_eq!(e.message, "path not found");
    assert_eq!(e.hint(), Some("use tree to explore"));
}

#[test]
fn recoverable_error_display_shows_message() {
    // BUG-052 regression: Display now surfaces both message AND attached
    // guidance text. Previously only `self.message` was emitted, which
    // hid hint/warning/must_follow content from `to_string()` consumers.
    let e = RecoverableError::with_hint("file missing", "check the path");
    let s = e.to_string();
    assert!(s.contains("file missing"), "must keep message: {s}");
    assert!(s.contains("check the path"), "must surface hint: {s}");
}

#[test]
fn require_u64_param_accepts_integer() {
    let input = serde_json::json!({ "n": 42 });
    assert_eq!(require_u64_param(&input, "n").unwrap(), 42);
}

#[test]
fn require_u64_param_accepts_string_encoded_integer() {
    // LLMs sometimes quote integers — we must tolerate this.
    let input = serde_json::json!({ "n": "11" });
    assert_eq!(require_u64_param(&input, "n").unwrap(), 11);
}

#[test]
fn require_u64_param_rejects_non_numeric_string() {
    let input = serde_json::json!({ "n": "abc" });
    assert!(require_u64_param(&input, "n").is_err());
}

#[test]
fn require_u64_param_rejects_negative_string() {
    let input = serde_json::json!({ "n": "-5" });
    assert!(require_u64_param(&input, "n").is_err());
}

#[test]
fn require_str_param_or_hint_prefers_canonical_over_alias() {
    use serde_json::json;
    let input = json!({ "path": "canonical.rs", "file_path": "alias.rs" });
    let got = require_str_param_or_hint(&input, "path", &["file_path"], "hint").unwrap();
    assert_eq!(got, "canonical.rs");
}

#[test]
fn require_str_param_or_hint_falls_back_to_alias() {
    use serde_json::json;
    let input = json!({ "file_path": "alias.rs" });
    let got = require_str_param_or_hint(&input, "path", &["file_path"], "hint").unwrap();
    assert_eq!(got, "alias.rs");
}

#[test]
fn require_str_param_or_hint_missing_surfaces_custom_hint() {
    use serde_json::json;
    let err = require_str_param_or_hint(&json!({}), "path", &["file_path"], "call it like X")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("call it like X"),
        "custom hint should surface: {err}"
    );
}

#[test]
fn require_str_param_or_hint_non_string_surfaces_custom_hint() {
    use serde_json::json;
    let err = require_str_param_or_hint(&json!({ "path": 42 }), "path", &[], "call it like X")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("call it like X"),
        "custom hint should surface on type error: {err}"
    );
}

/// BL-3 Class B / `docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`.
///
/// Class A put the *requirement* in the schema description. Class B is the other
/// half: when the tool refuses anyway, the refusal must teach the call rather than
/// restate the error. Roughly 13 of 34 live schema errors carried the generic
/// template, which says only what the reader already knows:
///
/// ```text
/// missing 'symbol' parameter — Add the required 'symbol' parameter to the tool call.
/// ```
///
/// The project already ruled on this principle in
/// `docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md`:
/// a bare "not found" was a defect *because the tool was holding the content needed
/// to help*. The server knows the parameter's name, type and purpose at the moment
/// it rejects.
///
/// Runs through the real emitter, so a name that loses its hint fails here.
#[test]
fn a_missing_required_param_teaches_the_call_instead_of_restating_the_error() {
    use serde_json::json;

    for name in [
        "symbol",
        "old_string",
        "command",
        "pattern",
        "query",
        "content",
        "topic",
    ] {
        let err = crate::tools::require_str_param(&json!({}), name)
            .expect_err("an absent required param must be refused")
            .to_string();
        assert!(
            !err.contains("Add the required"),
            "{name}: the generic template restates the error instead of teaching the \
             call — got: {err}"
        );
        assert!(
            err.contains(&format!("{name}=")),
            "{name}: the hint must show a concrete call shape, e.g. `{name}=…` — got: {err}"
        );
    }
}

/// The half a shared name→hint table provably cannot cover, and the reason this
/// fix is not one table lookup.
///
/// `path` is three different things across its three `require_str_param` call
/// sites: a file awaiting approval (`approve_write`), a project **directory**
/// (`workspace(activate)`), and a library root (`library(register)`). A single
/// global hint for `path` would be confidently wrong in two of the three — worse
/// than the generic template it replaces, because it reads as authoritative.
///
/// Asserting the hints are pairwise DISTINCT is what pins that: no shared table
/// entry can satisfy this test.
#[tokio::test]
async fn one_param_name_meaning_three_things_gets_three_hints() {
    use serde_json::json;
    let ctx = bare_ctx().await;

    // `approve_write` runs `guard_worktree_write` and requires an active project
    // *before* it reads `path`, so a bare ctx never reaches the param check — it
    // fails with "No active project" instead, which would pass a laxer assertion
    // for entirely the wrong reason.
    let dir = tempfile::tempdir().unwrap();
    let rooted = rooted_ctx(dir.path()).await;
    let approve = crate::tools::approve_write::ApproveWrite
        .call(json!({}), &rooted)
        .await
        .expect_err("approve_write without `path` must be refused")
        .to_string();
    let activate = crate::tools::config::ActivateProject
        .call(json!({}), &ctx)
        .await
        .expect_err("workspace(activate) without `path` must be refused")
        .to_string();
    let register = crate::tools::library::RegisterLibrary
        .call(json!({}), &ctx)
        .await
        .expect_err("library(register) without `path` must be refused")
        .to_string();

    for (label, err) in [
        ("approve_write", &approve),
        ("activate_project", &activate),
        ("register_library", &register),
    ] {
        // Assert the refusal is the one under test. Without this the row can pass
        // on an unrelated early error — `approve_write` runs `guard_worktree_write`
        // before reading `path`, and did exactly that on the first run.
        assert!(
            err.contains("missing 'path'"),
            "{label}: expected the missing-`path` refusal, got something else: {err}"
        );
        assert!(
            !err.contains("Add the required"),
            "{label}: still emitting the generic template — got: {err}"
        );
    }

    assert_ne!(
        approve, activate,
        "approve_write wants a file, workspace(activate) wants a directory — one hint \
         cannot be right for both"
    );
    assert_ne!(
        activate, register,
        "workspace(activate) wants a project root, library(register) wants a library \
         root — one hint cannot be right for both"
    );
    assert_ne!(approve, register, "these want different things too");
}

/// Caught by live verification, not by the unit test above — which is the point.
///
/// `a_missing_required_param_teaches_the_call_…` exercises `require_str_param`, the
/// shared helper. `memory` does not always reach it: `topic` goes through a private
/// `require_topic_param` with its own hardcoded hint, so the table entry was bypassed
/// and the live server still answered:
///
/// ```text
/// missing 'topic' parameter — Add the required 'topic' parameter to the tool call.
/// ```
///
/// while `query` on the same tool already showed the new hint. A test written to the
/// mechanism cannot see a call site that skips the mechanism. This one drives the
/// real tool, so it can.
#[tokio::test]
async fn memorys_runtime_required_params_teach_the_call_through_the_real_tool() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let ctx = rooted_ctx(dir.path()).await;

    // (action, the param omitted). All are required at runtime but absent from the
    // schema's `required` array — exactly the shape BL-3 is about, and the only one
    // reachable through a schema-validating client.
    for (action, param) in [("read", "topic"), ("recall", "query")] {
        let err = crate::tools::memory::Memory
            .call(json!({ "action": action }), &ctx)
            .await
            .expect_err("a runtime-required param must be refused when absent")
            .to_string();

        assert!(
            err.contains(&format!("missing '{param}'")),
            "action={action}: expected the missing-`{param}` refusal, got something \
             else entirely: {err}"
        );
        assert!(
            !err.contains("Add the required"),
            "action={action}: `{param}` still emits the generic template — a call site \
             that bypasses require_str_param bypasses the hint table with it: {err}"
        );
        assert!(
            err.contains(&format!("{param}=")),
            "action={action}: the hint must show a concrete call shape: {err}"
        );
    }
}

#[test]
fn require_path_param_accepts_unified_aliases() {
    use serde_json::json;
    // Symbol tools (references/edit_code/call_graph/symbol_at) route through this
    // shared helper; it must accept the same unified alias set as the file tools.
    assert_eq!(
        crate::fs::require_path_param(&json!({ "file_path": "src/x.rs" })).unwrap(),
        "src/x.rs"
    );
    assert_eq!(
        crate::fs::require_path_param(&json!({ "relative_path": "src/y.rs" })).unwrap(),
        "src/y.rs"
    );
    // Canonical `path` still wins when both are present.
    assert_eq!(
        crate::fs::require_path_param(&json!({ "path": "canon.rs", "file_path": "alias.rs" }))
            .unwrap(),
        "canon.rs"
    );
}

#[test]
fn recoverable_error_downcasts_from_anyhow() {
    let e: anyhow::Error = RecoverableError::new("test error").into();
    assert!(
        e.downcast_ref::<RecoverableError>().is_some(),
        "must be recoverable via downcast"
    );
}

#[test]
fn recoverable_error_with_warning_stores_warning_variant() {
    let e = RecoverableError::with_warning("too many results", "narrow with path=");
    assert_eq!(e.message, "too many results");
    assert!(matches!(e.guidance, Some(Guidance::Warning(ref s)) if s == "narrow with path="));
}

#[test]
fn recoverable_error_with_must_follow_stores_must_follow_variant() {
    let e = RecoverableError::with_must_follow("heading too large", "IRON LAW #6: use @file_xxx");
    assert_eq!(e.message, "heading too large");
    assert!(
        matches!(e.guidance, Some(Guidance::MustFollow(ref s)) if s == "IRON LAW #6: use @file_xxx")
    );
}

#[test]
fn recoverable_error_with_hint_still_produces_hint_variant() {
    let e = RecoverableError::with_hint("not found", "check path");
    assert!(matches!(e.guidance, Some(Guidance::Hint(ref s)) if s == "check path"));
    assert_eq!(e.hint(), Some("check path"));
}

/// BUG-052: `Display` (i.e. `to_string()`) must surface the attached
/// guidance text, not just `self.message`. Test authors and log readers
/// previously had to downcast and call `hint()`/match `guidance` to see
/// the extra context — easy to miss.
#[test]
fn display_includes_hint_text() {
    let e = RecoverableError::with_hint("not found", "check the path");
    let s = e.to_string();
    assert!(s.contains("not found"), "must keep message: {s}");
    assert!(s.contains("check the path"), "must surface hint text: {s}");
}

#[test]
fn display_includes_warning_text() {
    let e = RecoverableError::with_warning("too many results", "narrow with path=");
    let s = e.to_string();
    assert!(
        s.contains("narrow with path="),
        "must surface warning text: {s}"
    );
}

#[test]
fn display_includes_must_follow_text() {
    let e = RecoverableError::with_must_follow("heading too large", "IRON LAW #6: use @file_xxx");
    let s = e.to_string();
    assert!(
        s.contains("IRON LAW #6"),
        "must surface must_follow text: {s}"
    );
}

#[test]
fn display_no_guidance_just_message() {
    let e = RecoverableError::new("simple error");
    assert_eq!(
        e.to_string(),
        "simple error",
        "no guidance attached → Display is just the message"
    );
}

#[test]
fn recoverable_error_extra_fields_roundtrip() {
    let mut e = RecoverableError::new("heading too large");
    e.extra
        .insert("file_id".into(), serde_json::json!("@file_abc"));
    e.extra.insert(
        "section_map".into(),
        serde_json::json!([{"level": 2, "text": "## X", "line": 10}]),
    );
    assert_eq!(e.extra["file_id"], "@file_abc");
    assert_eq!(e.extra["section_map"][0]["line"], 10);
}

#[test]
fn is_regex_like_detects_alternation() {
    assert!(is_regex_like("foo|bar"));
    assert!(is_regex_like("foo|bar|baz"));
}

#[test]
fn is_regex_like_detects_wildcards() {
    assert!(is_regex_like("foo.*bar"));
    assert!(is_regex_like("foo.+bar"));
    assert!(is_regex_like("foo.?bar"));
}

#[test]
fn is_regex_like_detects_anchors() {
    assert!(is_regex_like("^main"));
    assert!(is_regex_like("name$"));
}

#[test]
fn is_regex_like_detects_character_classes_with_range() {
    assert!(is_regex_like("[A-Z]foo"));
    assert!(is_regex_like("bar[0-9]"));
}

#[test]
fn is_regex_like_detects_escape_sequences() {
    assert!(is_regex_like(r"\bword"));
    assert!(is_regex_like(r"foo\d+"));
    assert!(is_regex_like(r"\w+bar"));
    assert!(is_regex_like(r"foo\s"));
}

#[test]
fn is_regex_like_detects_grouping() {
    assert!(is_regex_like("(foo|bar)"));
    assert!(is_regex_like("some(thing)"));
}

#[test]
fn is_regex_like_rejects_plain_identifiers() {
    assert!(!is_regex_like("my_function"));
    assert!(!is_regex_like("MyStruct/method"));
    assert!(!is_regex_like("some-name"));
    assert!(!is_regex_like("CamelCase"));
    assert!(!is_regex_like("foo.bar"));
    assert!(!is_regex_like("Vec<String>"));
    assert!(!is_regex_like(""));
}

#[test]
fn is_regex_like_rejects_lone_pipe() {
    assert!(!is_regex_like("|leading"));
    assert!(!is_regex_like("trailing|"));
}

#[test]
fn is_regex_like_rejects_brackets_without_range() {
    assert!(!is_regex_like("[u8]"));
    assert!(!is_regex_like("[i32; 4]"));
}

// ---- call_content auto-buffering tests ----

async fn bare_ctx() -> ToolContext {
    ToolContext {
        agent: crate::agent::Agent::new(None).await.unwrap(),
        lsp: crate::lsp::LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

struct EchoTool {
    result: serde_json::Value,
    user_summary: Option<String>,
}

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo_tool"
    }
    fn description(&self) -> &str {
        "test"
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    async fn call(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(self.result.clone())
    }
    fn format_compact(&self, _result: &serde_json::Value) -> Option<String> {
        self.user_summary.clone()
    }
}

/// Make `root` look like a checkout with one linked worktree, the way
/// `list_git_worktrees` reads it: `.git/worktrees/<n>/gitdir` holds the
/// absolute path of the worktree's own `.git`.
fn seed_linked_worktree(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let wt_root = root.parent().unwrap().join(format!("wt-{name}"));
    std::fs::create_dir_all(&wt_root).unwrap();
    let entry = root.join(".git").join("worktrees").join(name);
    std::fs::create_dir_all(&entry).unwrap();
    std::fs::write(
        entry.join("gitdir"),
        format!("{}/.git\n", wt_root.display()),
    )
    .unwrap();
    wt_root
}

async fn echo_once(ctx: &ToolContext) -> String {
    let tool = EchoTool {
        result: serde_json::json!({"key": "value"}),
        user_summary: None,
    };
    let content = tool.call_content(serde_json::json!({}), ctx).await.unwrap();
    content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md
///
/// `guard_worktree_write` refuses writes on these two facts; reads used to
/// resolve against the main checkout and say nothing. One-shot, or it becomes
/// noise on every call — the failure mode `removed_attributes` was designed
/// around.
#[tokio::test]
async fn a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    let wt = seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;

    let first = echo_once(&ctx).await;
    assert!(
        first.contains("_workspace_notice"),
        "the first read must say which tree it resolved against, got: {first}"
    );
    assert!(
        first.contains(&wt.display().to_string()),
        "the notice must name the worktree the caller might mean, got: {first}"
    );
    assert!(
        first.contains("workspace(action='activate'"),
        "the notice must name a call the caller can actually run, got: {first}"
    );

    let second = echo_once(&ctx).await;
    assert!(
        !second.contains("_workspace_notice"),
        "one-shot: a notice on every call is noise, got: {second}"
    );
}

/// The other half of the pair: once the caller HAS chosen, the notice has
/// nothing to ask for and must not fire at all — not even once.
#[tokio::test]
async fn an_explicitly_activated_project_gets_no_worktree_notice() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;
    ctx.agent.activate(root.clone(), None).await.unwrap();

    let text = echo_once(&ctx).await;
    assert!(
        !text.contains("_workspace_notice"),
        "the caller already made the choice the notice would ask for, got: {text}"
    );
}

/// The notice must not fire in the overwhelmingly common case — a repo with no
/// linked worktrees at all — or every session in every ordinary checkout pays
/// for it.
#[tokio::test]
async fn a_repo_without_worktrees_gets_no_notice() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let ctx = rooted_ctx(&root).await;

    let text = echo_once(&ctx).await;
    assert!(
        !text.contains("_workspace_notice"),
        "no worktrees means no ambiguity to report, got: {text}"
    );
}

#[tokio::test]
async fn call_content_passthrough_small_output() {
    let ctx = bare_ctx().await;
    let result = serde_json::json!({"key": "value"});
    let tool = EchoTool {
        result: result.clone(),
        user_summary: None,
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    // Small output: no buffering — content should contain the JSON
    assert_eq!(content.len(), 1, "small output should not be buffered");
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(text.contains("key"));
}

#[tokio::test]
async fn call_content_small_output_ignores_format_compact() {
    // Even when format_compact returns Some, call_content must return exactly
    // 1 block with pretty JSON — the compact text is NOT injected into small outputs.
    let ctx = bare_ctx().await;
    let result = serde_json::json!({"key": "value"});
    let tool = EchoTool {
        result: result.clone(),
        user_summary: Some("compact summary".to_string()),
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    assert_eq!(
        content.len(),
        1,
        "small output must produce exactly 1 block, got: {:?}",
        content
    );
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("key"),
        "block must contain the JSON key, got: {}",
        text
    );
    assert!(
        !text.contains("compact summary"),
        "compact summary must NOT appear in small-output block, got: {}",
        text
    );
}

#[tokio::test]
async fn call_content_buffers_large_output() {
    let ctx = bare_ctx().await;
    // Build a Value that serializes to >> 5_000 bytes (well above the buffer threshold)
    let big_array: Vec<serde_json::Value> = (0..500)
        .map(|i| {
            serde_json::json!({
                "index": i,
                "name": format!("symbol_{}", i),
                "file": "src/tools/symbol.rs"
            })
        })
        .collect();
    let result = serde_json::json!({ "symbols": big_array });
    let tool = EchoTool {
        result,
        user_summary: None,
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    // Must return exactly 1 Content item
    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    // Contains a @tool_ ref handle
    assert!(text.contains("@tool_"), "expected @tool_ ref in: {}", text);
}

#[tokio::test]
async fn call_content_uses_format_compact_in_buffer_summary() {
    let ctx = bare_ctx().await;
    let big_array: Vec<serde_json::Value> = (0..500)
        .map(|i| {
            serde_json::json!({
                "index": i,
                "name": format!("symbol_{}", i)
            })
        })
        .collect();
    let result = serde_json::json!({ "symbols": big_array });
    let tool = EchoTool {
        result,
        user_summary: Some("Found 500 symbols".to_string()),
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("Found 500 symbols"),
        "expected summary in: {}",
        text
    );
    assert!(text.contains("@tool_"), "expected ref handle in: {}", text);
}

#[tokio::test]
async fn call_content_generic_fallback_without_format_compact() {
    let ctx = bare_ctx().await;
    let big_array: Vec<serde_json::Value> = (0..500)
        .map(|i| {
            serde_json::json!({
                "index": i,
                "name": format!("symbol_{}", i)
            })
        })
        .collect();
    let result = serde_json::json!({ "symbols": big_array });
    let tool = EchoTool {
        result,
        user_summary: None,
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    // No format_compact → generic fallback message with byte count and ref
    assert!(
        text.contains("bytes") || text.contains("stored"),
        "expected fallback in: {}",
        text
    );
    assert!(text.contains("@tool_"), "expected ref handle in: {}", text);
}

// ---- threshold + summary-cap tests ----

#[tokio::test]
async fn call_content_buffers_at_token_threshold() {
    // Build a Value whose JSON is ~12 KB — above MAX_INLINE_TOKENS (2500 tokens ≈ 10 KB).
    let ctx = bare_ctx().await;
    let items: Vec<serde_json::Value> = (0..150)
        .map(|i| {
            serde_json::json!({
                "file": format!("src/tools/file_{}.rs", i),
                "line": i,
                "content": format!("let x_{} = some_function_call_{};\n", i, i)
            })
        })
        .collect();
    let result = serde_json::json!({ "matches": items, "total": items.len() });

    // Sanity: confirm the JSON exceeds the token-based threshold (~10 KB)
    let json_len = serde_json::to_string(&result).unwrap().len();
    assert!(
        json_len > MAX_INLINE_TOKENS * 4,
        "test data must exceed token threshold ({} bytes), got {} bytes",
        MAX_INLINE_TOKENS * 4,
        json_len
    );

    let tool = EchoTool {
        result,
        user_summary: Some("150 matches".to_string()),
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("@tool_"),
        "output exceeding token limit must be buffered, got: {}",
        &text[..text.len().min(200)]
    );
}

#[tokio::test]
async fn call_content_does_not_buffer_under_token_limit() {
    // ~2 KB result — well under MAX_INLINE_TOKENS, must stay inline (no @tool_ ref)
    let ctx = bare_ctx().await;
    let items: Vec<serde_json::Value> = (0..30)
        .map(|i| serde_json::json!({ "file": format!("src/a_{}.rs", i), "line": i }))
        .collect();
    let result = serde_json::json!({ "matches": items });

    let json_len = serde_json::to_string(&result).unwrap().len();
    assert!(
        json_len < 5_000,
        "test data must be < 5 KB, got {} bytes",
        json_len
    );

    let tool = EchoTool {
        result,
        user_summary: Some("30 matches".to_string()),
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        !text.contains("@tool_"),
        "small output must not be buffered, got: {}",
        &text[..text.len().min(200)]
    );
}

#[tokio::test]
async fn call_content_caps_compact_summary() {
    // format_compact returns a 4 KB summary — must be truncated to ≤ 3 KB (hard max)
    let ctx = bare_ctx().await;
    let items: Vec<serde_json::Value> = (0..200)
        .map(|i| serde_json::json!({ "idx": i, "name": "x".repeat(50) }))
        .collect();
    let result = serde_json::json!({ "items": items });

    // Summary deliberately larger than hard cap
    let big_summary = format!("{}\n", "summary line ".repeat(300)); // ~3.9 KB
    assert!(
        big_summary.len() > 3_000,
        "summary must be > hard cap for this test"
    );

    let tool = EchoTool {
        result,
        user_summary: Some(big_summary),
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    // Output is now a JSON object — parse it to check individual fields
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("call_content must return valid JSON");
    assert!(
        parsed["output_id"]
            .as_str()
            .unwrap_or("")
            .starts_with("@tool_"),
        "must have output_id: {parsed}"
    );
    // The summary field must be capped. truncate_compact appends "\n… (truncated)"
    // (~15 bytes) after the hard-max boundary, so allow a small suffix slack.
    let summary = parsed["summary"].as_str().unwrap_or("");
    assert!(
        summary.len() <= COMPACT_SUMMARY_HARD_MAX_BYTES + 20,
        "summary must be capped; got {} bytes",
        summary.len()
    );
    assert!(
        summary.contains("truncated"),
        "must include truncation note: {}",
        &summary[..summary.len().min(200)]
    );
    // hint must be present and reference the output_id
    let hint = parsed["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains("@tool_"),
        "hint must reference the output_id: {hint}"
    );
}

// ---- buffered_bytes field tests ----

#[tokio::test]
async fn overflow_envelope_carries_buffered_bytes() {
    let ctx = bare_ctx().await;
    let items: Vec<serde_json::Value> = (0..200)
        .map(|i| serde_json::json!({ "idx": i, "name": "x".repeat(50) }))
        .collect();
    let tool = EchoTool {
        result: serde_json::json!({ "items": items }),
        user_summary: None,
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("overflow must return a JSON envelope");
    assert!(
        parsed["output_id"]
            .as_str()
            .unwrap_or("")
            .starts_with("@tool_"),
        "result must overflow: {parsed}"
    );
    let bytes = parsed["buffered_bytes"]
        .as_u64()
        .expect("envelope must carry buffered_bytes");
    assert!(bytes > 0, "buffered_bytes must be positive: {parsed}");
}

// ---- truncate_compact tests ----

#[test]
fn truncate_compact_under_soft_cap_returns_verbatim() {
    let text = "line1\nline2\nline3";
    assert_eq!(truncate_compact(text, 2_000, 3_000), text);
}

#[test]
fn truncate_compact_exact_soft_cap_returns_verbatim() {
    // Exactly at the soft cap — no truncation
    let text = "x".repeat(2_000);
    assert_eq!(truncate_compact(&text, 2_000, 3_000), text);
}

#[test]
fn truncate_compact_at_line_boundary() {
    // Line 1 is 1,800 bytes; line 2 is 600 bytes → total 2,401 (> soft_max=2_000)
    // Last '\n' is at byte 1,800, which is ≤ hard_max=3_000 → truncate there
    let line1 = "a".repeat(1_800);
    let line2 = "b".repeat(600);
    let text = format!("{}\n{}", line1, line2);

    let result = truncate_compact(&text, 2_000, 3_000);

    assert!(result.starts_with(&line1), "should keep line1 intact");
    assert!(!result.contains(&line2), "should drop line2");
    assert!(
        result.contains("… (truncated)"),
        "should append truncation note"
    );
}

#[test]
fn truncate_compact_no_newlines_uses_hard_cap() {
    // Single 5,000-byte line — no '\n' → hard-cap at 3,000 bytes
    let text = "x".repeat(5_000);
    let result = truncate_compact(&text, 2_000, 3_000);

    assert!(
        result.starts_with(&"x".repeat(3_000)),
        "should keep first 3,000 bytes"
    );
    assert!(result.ends_with("… (truncated)"), "should append note");
    // Sanity check: result is not longer than hard_max + note
    assert!(result.len() <= 3_000 + 20);
}

#[test]
fn truncate_compact_preserves_text_exactly_at_hard_cap() {
    // Text is 2,500 bytes (> soft) with a single newline at position 2,400.
    // Line boundary (2,400) is between soft (2,000) and hard (3,000) — use it.
    let line1 = "a".repeat(2_400);
    let line2 = "b".repeat(99);
    let text = format!("{}\n{}", line1, line2);

    let result = truncate_compact(&text, 2_000, 3_000);

    assert!(result.starts_with(&line1), "should keep line1");
    assert!(!result.contains(&line2), "should not include line2");
    assert!(result.contains("… (truncated)"));
}

#[test]
fn truncate_compact_unicode_does_not_panic() {
    // Regression test for the read_file crash on docs/ARCHITECTURE.md.
    // Box-drawing chars (─, │, ┌, etc.) are 3 bytes each in UTF-8.
    // A hard_max that lands mid-char must NOT cause a panic.
    let box_line: String = "─".repeat(700); // 2100 bytes
    let prefix = "x".repeat(100);
    let text = format!("{}\n{}", prefix, box_line); // >2000 bytes, no newline after 101

    // Must not panic regardless of where hard_max falls inside multi-byte chars.
    let result = truncate_compact(&text, 2_000, 3_000);
    assert!(result.contains("… (truncated)"), "should be truncated");
    // Result must be valid UTF-8 (no mid-char slices)
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn floor_char_boundary_lands_on_boundary() {
    let s = "ab─cd"; // 'a'=1, 'b'=1, '─'=3 bytes (E2 94 80), 'c'=1, 'd'=1
                     // bytes: 0='a', 1='b', 2-4='─', 5='c', 6='d'
    assert_eq!(floor_char_boundary(s, 0), 0);
    assert_eq!(floor_char_boundary(s, 2), 2); // before '─'
    assert_eq!(floor_char_boundary(s, 3), 2); // inside '─' → back to 2
    assert_eq!(floor_char_boundary(s, 4), 2); // inside '─' → back to 2
    assert_eq!(floor_char_boundary(s, 5), 5); // after '─'
    assert_eq!(floor_char_boundary(s, 6), 6);
    assert_eq!(floor_char_boundary(s, 100), s.len()); // clamp to len
}

#[test]
fn safe_truncate_avoids_mid_char_split() {
    let s = "ab\u{2500}cd"; // 'a'=1, 'b'=1, '\u{2500}'=3 bytes, 'c'=1, 'd'=1
    assert_eq!(safe_truncate(s, 0), "");
    assert_eq!(safe_truncate(s, 2), "ab");
    assert_eq!(safe_truncate(s, 3), "ab"); // inside 3-byte char → round down
    assert_eq!(safe_truncate(s, 4), "ab"); // still inside
    assert_eq!(safe_truncate(s, 5), "ab\u{2500}");
    assert_eq!(safe_truncate(s, 100), s); // clamp to len
}

// ---- elicitation tests ----

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct TestConfirm {
    confirm: bool,
}
rmcp::elicit_safe!(TestConfirm);

#[tokio::test]
async fn elicit_returns_none_when_no_peer() {
    let ctx = bare_ctx().await;
    let result = ctx.elicit::<TestConfirm>("Test?").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn elicit_user_declined_is_recoverable_error() {
    // UserDeclined must produce a RecoverableError (isError: false at MCP level),
    // not a plain anyhow error (isError: true). We verify this by constructing the
    // error the same way the elicit() match arm does and checking the downcast.
    let e: anyhow::Error = RecoverableError::with_hint(
        "User declined the elicitation request",
        "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
    )
    .into();
    assert!(
        e.downcast_ref::<RecoverableError>().is_some(),
        "UserDeclined must be a RecoverableError so it routes to isError:false"
    );
}

#[test]
fn elicit_user_cancelled_is_recoverable_error() {
    // UserCancelled must produce a RecoverableError (isError: false at MCP level),
    // not a plain anyhow error (isError: true).
    let e: anyhow::Error = RecoverableError::with_hint(
        "User cancelled the elicitation request",
        "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
    )
    .into();
    assert!(
        e.downcast_ref::<RecoverableError>().is_some(),
        "UserCancelled must be a RecoverableError so it routes to isError:false"
    );
}

// ---- availability tests ----

mod availability_tests {
    use super::*;

    struct AlwaysTool;
    #[async_trait::async_trait]
    impl Tool for AlwaysTool {
        fn name(&self) -> &str {
            "always"
        }
        fn description(&self) -> &str {
            ""
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(
            &self,
            _i: serde_json::Value,
            _c: &ToolContext,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({}))
        }
    }

    #[test]
    fn default_availability_is_always() {
        let t = AlwaysTool;
        let caps = ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false,
        };
        assert!(t.availability(&caps).is_available(&ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false
        }));
        assert!(matches!(t.availability(&caps), Availability::Always));
    }

    #[test]
    fn availability_gates_toggle_correctly() {
        let off = ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false,
        };
        let on = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            has_libraries: true,
        };
        assert!(!Availability::RequiresLsp.is_available(&off));
        assert!(Availability::RequiresLsp.is_available(&on));
        assert!(Availability::Always.is_available(&off));
    }
}

async fn rooted_ctx(root: &std::path::Path) -> ToolContext {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    ToolContext {
        agent: crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap(),
        lsp: crate::lsp::LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

/// Like `EchoTool`, but its compact summary is DERIVED from the result it is
/// handed. `EchoTool::format_compact` ignores its `_result` argument and
/// returns a stored string, so it cannot detect whether the value was stripped
/// before or after the summary was built — which is exactly the ordering this
/// test exists to pin.
struct SummarizingEchoTool {
    result: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for SummarizingEchoTool {
    fn name(&self) -> &str {
        "summarizing_echo_tool"
    }
    fn description(&self) -> &str {
        "test"
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    async fn call(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(self.result.clone())
    }
    fn format_compact(&self, result: &serde_json::Value) -> Option<String> {
        let first = result["matches"][0]["file"].as_str().unwrap_or("?");
        Some(format!("200 matches\n\nfirst: {first}"))
    }
}

#[tokio::test]
async fn call_content_relativizes_path_keys_but_not_content() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = rooted_ctx(&root).await;
    let root_fwd = crate::util::fs::to_forward_slash(&root);

    let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
    let tool = EchoTool {
        result: serde_json::json!({
            "file": format!("{root_fwd}/src/lib.rs"),
            "content": literal,
            "project_root": root_fwd,
        }),
        user_summary: None,
    };

    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    assert!(
        text.contains("\"src/lib.rs\""),
        "path key must relativize: {text}"
    );
    // Substring-match against the raw rendered text won't work here: `literal`
    // contains embedded `"` characters, and JSON string encoding always
    // backslash-escapes those in the serialized text, so the raw bytes of
    // `literal` never appear verbatim in `text` even when the value is
    // perfectly preserved. Parse back and compare the Value instead — that's
    // what "byte-identical content" actually means for a JSON-carried string.
    let parsed: serde_json::Value = serde_json::from_str(text).expect("primary block must be JSON");
    assert_eq!(
        parsed["content"].as_str(),
        Some(literal.as_str()),
        "file CONTENT must survive byte-identical: {text}"
    );
    assert!(
        text.contains(&format!("\"project_root\": \"{root_fwd}\"")),
        "root-valued field must stay absolute, never \"\": {text}"
    );
}

#[tokio::test]
async fn call_content_buffered_summary_is_built_from_the_stripped_value() {
    // Pins the ordering invariant: strip runs BEFORE format_compact builds the
    // buffer summary. Reversed, absolute paths escape through the serialized
    // envelope — 85% of the leaks measured before this change.
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = rooted_ctx(&root).await;
    let root_fwd = crate::util::fs::to_forward_slash(&root);

    // 500, not the 200 an unstripped-size estimate would suggest: the whole
    // point of this test is that `exceeds_inline_limit` now runs on the
    // ALREADY-STRIPPED json, so the item count must be big enough to clear
    // the 10 KB threshold using short relative paths, not long absolute ones
    // (a tmp dir under a short `/tmp` mount makes 200 fall back under budget).
    let items: Vec<serde_json::Value> = (0..500)
        .map(|i| serde_json::json!({ "file": format!("{root_fwd}/src/f_{i}.rs"), "line": i }))
        .collect();
    let tool = SummarizingEchoTool {
        result: serde_json::json!({ "matches": items }),
    };

    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    assert!(
        text.contains("@tool_"),
        "test data must exceed the inline budget: {text}"
    );
    assert!(
        text.contains("first: src/f_0.rs"),
        "format_compact must have observed the STRIPPED value: {text}"
    );
    assert!(
        !text.contains(&format!("{root_fwd}/src/")),
        "no absolute project path may survive into the buffered envelope: {text}"
    );

    // The envelope alone isn't enough: a mutation that strips the summary but
    // leaves the *stored* @tool_* payload unstripped would pass every
    // assertion above, since the payload itself never appears in `text`. Read
    // the buffer back and check the payload directly — this is the one
    // surface the legacy `server::post_process` text-level strip never
    // covered, so it has no fallback if this wiring regresses.
    let parsed: serde_json::Value = serde_json::from_str(text).expect("envelope must be JSON");
    let output_id = parsed["output_id"]
        .as_str()
        .expect("envelope must carry output_id");
    let buffered_payload = ctx
        .output_buffer
        .get(output_id)
        .expect("output_id must resolve in the buffer")
        .stdout;
    assert!(
        !buffered_payload.contains(&format!("{root_fwd}/")),
        "the stored @tool_* payload must itself be stripped, not just the envelope: {buffered_payload}"
    );
}
