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
    // Read the notice out of the JSON rather than substring-matching the serialized
    // text: `first` is a JSON document, so on Windows every separator in a path is
    // escaped to `\\` there while `wt.display()` yields single backslashes, and the
    // check could never match. On Linux the two forms coincide, which is why this
    // passed for a year and failed on all three Windows lanes.
    let parsed: serde_json::Value = serde_json::from_str(&first)
        .unwrap_or_else(|e| panic!("echo output is JSON: {e}: {first}"));
    let notice = parsed["_workspace_notice"].as_str().unwrap_or_default();
    assert!(
        notice.contains(&wt.display().to_string()),
        "the notice must name the worktree the caller might mean, got: {notice}"
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

/// The sibling `_workspace_notice` field sits next to a plausible `stdout`
/// answer and loses attention to it — measured twice in one session in
/// docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md.
/// `run_command`'s response is the one shape with a top-level `stdout`
/// string, so the notice must also land inside it, unmissably.
#[tokio::test]
async fn a_worktree_notice_is_prefixed_into_stdout_when_the_response_carries_one() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;

    let tool = EchoTool {
        result: serde_json::json!({"stdout": "3ecb8730 some commit subject", "exit_code": 0}),
        user_summary: None,
    };
    let content = tool
        .call_content(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let text = content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();

    assert!(
        text.contains("_workspace_notice"),
        "the sibling field must still be present: {text}"
    );
    let stdout_field: String = serde_json::from_str::<serde_json::Value>(&text).unwrap()["stdout"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        stdout_field.starts_with('⚠'),
        "the notice must be prefixed into stdout, not just live in a sibling field: {stdout_field}"
    );
    assert!(
        stdout_field.ends_with("3ecb8730 some commit subject"),
        "the original stdout must be preserved verbatim after the prefix: {stdout_field}"
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

/// docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md
///
/// The discriminating case the bug named: a project resolved only through
/// `Agent::new(Some(root))` (the startup/cwd-fallback path) is NOT a choice,
/// so a write with worktrees present must be refused even though
/// `is_project_explicitly_activated` is true for that same agent.
#[tokio::test]
async fn guard_worktree_write_refuses_when_only_resolved_at_startup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;

    assert!(
        ctx.agent.is_project_explicitly_activated().await,
        "startup resolution still sets the legacy flag"
    );
    let result = guard_worktree_write(&ctx).await;
    assert!(
        result.is_err(),
        "startup resolution is not a choice; the write must be refused"
    );
}

/// The other half: once the caller has actually called `activate`, the write
/// must go through.
#[tokio::test]
async fn guard_worktree_write_allows_after_explicit_activate() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;
    ctx.agent.activate(root.clone(), None).await.unwrap();

    let result = guard_worktree_write(&ctx).await;
    assert!(
        result.is_ok(),
        "the caller chose; the write must be allowed"
    );
}

/// The overwhelmingly common case — no linked worktrees at all — must never
/// start refusing writes just because `activate` was never called.
#[tokio::test]
async fn guard_worktree_write_allows_when_no_worktrees_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let ctx = rooted_ctx(&root).await;

    let result = guard_worktree_write(&ctx).await;
    assert!(
        result.is_ok(),
        "no worktrees means no ambiguity; the write must be allowed"
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
            shell_enabled: false,
        };
        assert!(t.availability(&caps).is_available(&ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false,
            shell_enabled: false
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
            shell_enabled: false,
        };
        let on = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            has_libraries: true,
            shell_enabled: true,
        };
        assert!(!Availability::RequiresLsp.is_available(&off));
        assert!(Availability::RequiresLsp.is_available(&on));
        assert!(Availability::Always.is_available(&off));
        assert!(!Availability::RequiresShell.is_available(&off));
        assert!(Availability::RequiresShell.is_available(&on));
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

// ---- `wrote_to`: naming the checkout an unpinned write reached ----
//
// docs/issues/archive/2026-08-27-edit-code-writes-to-session-default-not-pinned-workspace.md
//
// Measured over 195,126 tool calls: a pin-armed REFUSAL would have blocked
// 9,914 writes (30.3% of the corpus) to catch at most 18 real misroutes. So
// this annotation refuses nothing — it only says which tree an ambiguous write
// actually reached.

/// `EchoTool` is a read, and the annotation is gated on `is_write`. `name` is a
/// field because the exemption list is keyed on it.
struct WriteEchoTool {
    name: &'static str,
    result: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for WriteEchoTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "test write"
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn is_write(&self, _input: &serde_json::Value) -> bool {
        true
    }
    async fn call(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(self.result.clone())
    }
}

/// Drive a write through the real `call_content` path and parse its answer back.
/// Going through `call_content` rather than calling the helper directly is the
/// point: it is the wiring — the pin check, the `is_write` capture before
/// `input` is consumed, the ordering against path-stripping — that can break.
async fn write_echo(
    ctx: &ToolContext,
    name: &'static str,
    result: serde_json::Value,
) -> serde_json::Value {
    let tool = WriteEchoTool { name, result };
    let content = tool.call_content(serde_json::json!({}), ctx).await.unwrap();
    let text = content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    serde_json::from_str(&text).expect("a write result must round-trip as JSON")
}

/// Stub tool exercising the operator-rules routing path in `call_content`.
///
/// **Historical note, kept because it is why this stub exists.** When this was
/// written, no production tool overrode `selector_key` except
/// `LibrarianAdapter` — `Memory` in particular did not — so OP-3
/// (`serves: memory.write`) could not route on a real
/// `memory(action="write", ...)` call, even though it is the rule Task 6
/// expects that call to surface. That is no longer true: `Memory` opts in as of
/// the fix to
/// docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md,
/// and `the_real_memory_tool_supplies_a_selector_key_for_op_3` below asserts it
/// against the real tool rather than this stand-in.
///
/// The stub is retained deliberately: it exercises the router against a
/// synthetic corpus without depending on `Memory`'s call path or its on-disk
/// state, which is what lets the routing filters be tested in isolation. Read
/// it as a test double, no longer as a paper-over. This stub
/// projects `{tool}.{action}` the same way `LibrarianAdapter::selector_key`
/// does, so the router path in `call_content` can be exercised regardless.
///
/// **Keep this stub's own `selector_key` override, even though `Tool`'s inverted
/// default (2026-09-01) now produces an identical key.** Deleting it as redundant
/// would make every router test in this file depend on the default under test, so a
/// regression to `None` would red them all and bury the one signal that names the
/// cause. Independence from the default is the property, not the projection.
struct RoutedEchoTool {
    name: &'static str,
    result: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for RoutedEchoTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "test routed"
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
    fn selector_key(&self, input: &serde_json::Value) -> Option<String> {
        match input.get("action").and_then(serde_json::Value::as_str) {
            Some(action) => Some(format!("{}.{}", self.name(), action)),
            None => Some(self.name().to_string()),
        }
    }
}

/// Joins every text block in a `call_content` result, in order, for substring
/// assertions that don't care which block carried the text.
fn joined_text(content: &[rmcp::model::Content]) -> String {
    content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// OP-3 (`triggered`, serves `memory.write`) must arrive once per session for
/// a call shape it serves, and not again on a repeat of the same shape — the
/// same once-per-session contract the guide ledger already gives topics, now
/// asserted for the `op:OP-3` key. `RoutedEchoTool` stands in for `memory` (see its
/// doc comment) so the router filters can be exercised without depending on `Memory`'s
/// call path, its on-disk state, or the trait default this file also tests. The real
/// tool has supplied a key since `2447f709`, and supplies it via the inverted default
/// as of 2026-09-01 — `crate::tools::memory::tests::a_real_memory_write_call_delivers_op_3`
/// is the end-to-end assertion against the real tool.
///
/// Also checks the delivered payload rather than just the marker (a mutation
/// emitting the `operator-rule OP-3` comment with an empty or wrong body
/// would otherwise ship green), and that a present-but-non-matching selector
/// — `memory.read`, absent from OP-3's `**Serves:**` — routes nothing.
#[tokio::test]
async fn a_triggered_operator_rule_is_delivered_once_per_session() {
    let ctx = bare_ctx().await;
    let tool = RoutedEchoTool {
        name: "memory",
        result: serde_json::json!({"ok": true}),
    };
    let input = serde_json::json!({"action": "write"});

    let first = tool.call_content(input.clone(), &ctx).await.unwrap();
    let first_text = joined_text(&first);
    assert!(
        first_text.contains("operator-rule OP-3"),
        "expected an OP-3 operator-rule block on the first memory.write call, got: {first_text}"
    );
    assert!(
        first_text.contains("codescout memory or a tracker"),
        "expected OP-3's imperative payload, not just its marker, got: {first_text}"
    );

    let second = tool.call_content(input, &ctx).await.unwrap();
    let second_text = joined_text(&second);
    assert!(
        !second_text.contains("operator-rule OP-3"),
        "OP-3 must be delivered once per session for this call shape, got a repeat: {second_text}"
    );

    // A present-but-non-matching selector: `memory.read` isn't in OP-3's
    // `**Serves:**` (`memory.write` only), so it must route nothing.
    let non_matching = serde_json::json!({"action": "read"});
    let third = tool.call_content(non_matching, &ctx).await.unwrap();
    let third_text = joined_text(&third);
    assert!(
        !third_text.contains("operator-rule "),
        "a non-matching action must not deliver any operator rule, got: {third_text}"
    );
}

/// The REAL `Memory` tool must supply a selector key, or `OP-3`
/// (`**Serves:** memory.write`) can never route on a real call.
///
/// This is the assertion `RoutedEchoTool` has been standing in for, and the
/// substitution is the whole defect: the routing tests above are green against
/// a stub *named* `"memory"` that projects `{tool}.{action}`, while the
/// production `Memory` took the trait default and returned `None`. So the suite
/// proved the router works and said nothing about whether any real call reaches
/// it — a green suite and a dead feature were consistent with each other for as
/// long as the stub was the only caller.
///
/// Mutation that must kill this: make `Tool::selector_key`'s default return `None`
/// (`src/tools/core/types.rs`), which `Shape::matches` treats as "cannot match".
///
/// *(Updated 2026-09-01. This read "delete `Memory`'s `selector_key` override". That
/// mutation no longer exists: the default was inverted to opt every tool in, and
/// `Memory`'s now-redundant override was deleted, so there is nothing left to remove.
/// A stated kill-mutation that can no longer be applied credits a test with coverage it
/// does not have — and reads entirely plausibly while doing so, which is why this moved
/// rather than being left alone.)*
#[test]
fn the_real_memory_tool_supplies_a_selector_key_for_op_3() {
    let tool = crate::tools::memory::Memory;
    assert_eq!(
        tool.selector_key(&serde_json::json!({
            "action": "write",
            "topic": "t",
            "content": "c"
        }))
        .as_deref(),
        Some("memory.write"),
        "OP-3 declares `Serves: memory.write`; without this key route() is never \
         consulted for a real memory write, however correct the rule and matcher are"
    );
}

/// `OP-4` declares `**Serves:** edit_file(path~/.claude), create_file(path~/.claude)`,
/// so both write tools must supply a selector key or `route()` is never consulted
/// for either.
///
/// Neither takes an `action`, so the key is the bare tool name — the tool-only
/// shape `Shape::matches` already supports. `route.rs` currently hands itself
/// `Some("edit_file")` as an explicitly *synthetic* selector for this reason,
/// its own comment noting the string "never actually reaches `route()` on a
/// real call".
///
/// **This does not make `OP-4` fire, and that is deliberate.** Its `path~`
/// predicate is evaluated against the tool's *response*, and write tools return
/// no path by the no-echo convention
/// (`docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`, whose own
/// mutations show that widening to `wrote_to` still does not fire while a real
/// `abs_path` does). That is a second, independent defect. This closes only the
/// routing precondition — without it, fixing the predicate would change nothing.
///
/// Mutation that must kill this: make `Tool::selector_key`'s default return `None`
/// (`src/tools/core/types.rs`), which `Shape::matches` treats as "cannot match".
/// Updated 2026-09-01 from "drop either override": both overrides were deleted as
/// redundant when the default was inverted, so neither is available to mutate.
#[test]
fn the_real_write_tools_supply_selector_keys_for_op_4() {
    let input = serde_json::json!({"path": "/home/u/.claude/settings.json"});
    assert_eq!(
        crate::tools::edit_file::EditFile
            .selector_key(&input)
            .as_deref(),
        Some("edit_file"),
        "OP-4 serves edit_file(path~/.claude); without the key route() never runs"
    );
    assert_eq!(
        crate::tools::create_file::CreateFile
            .selector_key(&input)
            .as_deref(),
        Some("create_file"),
        "OP-4 serves create_file(path~/.claude) in the same breath — covering only \
         one of the pair would leave the rule half-routable, which is harder to \
         notice than not routable at all"
    );
}

/// OP-1 is `always`-bound, so `route()` must never surface it through
/// `call_content` — `always` rules are resident in the profile, not routed
/// just-in-time.
///
/// This documents intent at the delivery layer, but it is not what proves
/// the `Binding::Triggered` filter load-bearing: OP-1 carries no
/// `**Serves:**` entries at all (Gate 6 in `operator_rules::validate`
/// forbids an `always` rule from having any), so the selector-match filter
/// inside `route_in` already excludes it on its own — deleting the binding
/// filter leaves this test green. That was observed directly, not
/// inferred: RED, run before the `op_content` block existed in
/// `call_content`, passed vacuously — nothing was emitted for any tool
/// yet, so `OP-1` was trivially absent. GREEN, run after `op_content`
/// landed, passed again, but for the different reason above rather than
/// because the binding filter was actually exercised. The mutation is
/// caught one layer down by
/// `operator_rules::route::tests::route_in_excludes_an_always_rule_even_when_its_selector_matches`,
/// which builds a synthetic `always` rule with a matching `serves` — a
/// combination `validate` forbids the real ledger from ever holding.
///
/// Reuses the same `memory.write` call as the test above so this at least
/// asserts absence in a call already known to deliver something (OP-3),
/// rather than in a call that delivers nothing at all.
#[tokio::test]
async fn an_always_operator_rule_is_never_delivered_by_the_router() {
    let ctx = bare_ctx().await;
    let tool = RoutedEchoTool {
        name: "memory",
        result: serde_json::json!({"ok": true}),
    };
    let content = tool
        .call_content(serde_json::json!({"action": "write"}), &ctx)
        .await
        .unwrap();
    let text = joined_text(&content);
    assert!(
        !text.contains("operator-rule OP-1"),
        "OP-1 is `always`-bound and must never arrive via the triggered-rule router: {text}"
    );
}

/// A worktree-bearing checkout, plus a ctx rooted at it.
async fn worktree_repo_ctx(tmp: &tempfile::TempDir) -> (std::path::PathBuf, ToolContext) {
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    seed_linked_worktree(&root, "feat");
    let ctx = rooted_ctx(&root).await;
    (root, ctx)
}

/// The leak this closes: a write that resolved against the session default
/// answered with a bare `status: ok`, indistinguishable from the calls that
/// landed in the tree the caller meant.
#[tokio::test]
async fn an_unpinned_write_names_the_checkout_it_reached() {
    let tmp = tempfile::tempdir().unwrap();
    let (_root, ctx) = worktree_repo_ctx(&tmp).await;
    let expected = ctx.agent.project_root_for(None).await.unwrap();
    let expected = expected.display().to_string();

    let val = write_echo(&ctx, "edit_code", serde_json::json!("ok")).await;

    assert_eq!(
        val.get("wrote_to").and_then(|v| v.as_str()),
        Some(expected.as_str()),
        "an unpinned write in a repo with linked worktrees must name the tree it \
         reached, absolutely; got {val}"
    );
    assert_eq!(
        val.get("status").and_then(|v| v.as_str()),
        Some("ok"),
        "promoting the bare `\"ok\"` must preserve it as `status`, not discard \
         the tool's own answer; got {val}"
    );
}

/// A write must name the **path it wrote**, not only the checkout root.
///
/// `OP-4` declares `**Serves:** edit_file(path~/.claude)`, and its predicate runs
/// through `names_path_containing`, which scans the tool's *response*. Writes answer
/// `"ok"` under the no-echo convention, promoted here to `{"status":"ok","wrote_to":…}`
/// — and `wrote_to` is the project ROOT, not the file. So the rule could never match a
/// write to `~/.claude/…`: the one field present names the wrong thing.
/// `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` records exactly this,
/// its Mutation 1 showing that widening the scan to `wrote_to` still does not fire while
/// a response carrying a real `abs_path` does.
///
/// The path is captured from `&input` before `self.call` consumes it — the same
/// pre-consumption capture `selector` and `annotate_root` already use — and clones one
/// string rather than the input, which for `edit_file`/`create_file` carries whole file
/// bodies.
///
/// Deliberately asserts a path OUTSIDE the project root: that is `OP-4`'s actual target,
/// and it also survives the path-stripper, so the assertion cannot pass by accident on a
/// relativised value.
///
/// Mutation that must kill this: drop the `abs_path` insertion and the response carries
/// `wrote_to` alone, which is the current behaviour.
#[tokio::test]
async fn a_write_response_names_the_path_it_wrote_so_op_4_can_match() {
    let tmp = tempfile::tempdir().unwrap();
    let (_root, ctx) = worktree_repo_ctx(&tmp).await;
    let tool = WriteEchoTool {
        name: "edit_file",
        result: serde_json::json!("ok"),
    };
    // Absolute ON THIS PLATFORM, and outside the project root on both.
    //
    // The assertion below needs `abs_path` to echo this literal back, which only happens
    // when the input is already absolute. `/home/u/.claude/settings.json` is absolute on
    // POSIX and is NOT on Windows — no drive letter, no UNC — so the write resolved it
    // against the temp project root instead and the response carried no `abs_path` at
    // all. The assertion then read `None` on the three Windows lanes and wine while
    // passing on Linux and macOS (CI run 33433055755).
    //
    // "Outside the project root" is the property the test actually depends on (it is
    // OP-4's target, and it survives the path-stripper). Keep that true of both arms if
    // either literal changes.
    let target = if cfg!(windows) {
        r"C:\Users\u\.claude\settings.json"
    } else {
        "/home/u/.claude/settings.json"
    };
    let content = tool
        .call_content(serde_json::json!({ "path": target }), &ctx)
        .await
        .unwrap();
    let text = content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("a write result must round-trip as JSON");

    assert_eq!(
        val.get("abs_path").and_then(|v| v.as_str()),
        Some(target),
        "a write must name the path it wrote, or a path~ predicate has nothing to \
         match on; got {val}"
    );
    assert_eq!(
        val.get("status").and_then(|v| v.as_str()),
        Some("ok"),
        "adding the path must not discard the tool's own answer; got {val}"
    );
}

/// `OP-4` now routes on a write response the pipeline actually produced.
///
/// This replaces `op_4s_path_predicate_cannot_fire_against_a_write_response_today`
/// in `operator_rules::route`, which pinned the defect and told its reader
/// *"when this test starts failing, that is the fix landing"*. It never failed.
/// Its fixture was a hand-written `json!({"status":"ok","wrote_to":…})` bound to a
/// variable named `observed`, so it asserted against a response no tool returns —
/// and could not notice the fix any more than it could have noticed the bug. Same
/// shape as `RoutedEchoTool` supplying the `selector_key` the real tool lacked.
///
/// So the response here is not written by hand: it comes back out of
/// `call_content`, past the path-stripper and both annotations, and is fed to
/// `route` unmodified. The selector is supplied explicitly, which is honest
/// because `the_real_write_tools_supply_selector_keys_for_op_4` proves the real
/// tool produces exactly that string — the two tests meet at a value both check.
///
/// Mutation that must kill this: drop the `abs_path` insertion in `call_content`
/// and the response falls back to `wrote_to` alone, which names the project root
/// and matches no `~/.claude` needle.
#[tokio::test]
async fn op_4_routes_on_a_write_response_the_pipeline_produced() {
    let tmp = tempfile::tempdir().unwrap();
    let (_root, ctx) = worktree_repo_ctx(&tmp).await;
    let tool = WriteEchoTool {
        name: "edit_file",
        result: serde_json::json!("ok"),
    };
    let content = tool
        .call_content(
            serde_json::json!({"path": "/home/u/.claude/settings.json"}),
            &ctx,
        )
        .await
        .unwrap();
    let text = content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let produced: serde_json::Value =
        serde_json::from_str(&text).expect("a write result must round-trip as JSON");

    let hit = crate::operator_rules::route::route(Some("edit_file"), &produced);
    assert!(
        hit.iter().any(|r| r.id == "OP-4"),
        "OP-4 must route on a real write response naming a ~/.claude path; \
         produced={produced}, routed={:?}",
        hit.iter().map(|r| &r.id).collect::<Vec<_>>()
    );

    // Guard the guard: a write to a path OUTSIDE ~/.claude must NOT route, or the
    // assertion above would be equally satisfied by a predicate that matches
    // everything — which is the failure mode a `path~` predicate exists to avoid.
    let elsewhere = tool
        .call_content(serde_json::json!({"path": "/home/u/work/notes.md"}), &ctx)
        .await
        .unwrap();
    let text = elsewhere[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let other: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        !crate::operator_rules::route::route(Some("edit_file"), &other)
            .iter()
            .any(|r| r.id == "OP-4"),
        "a write outside ~/.claude must not route OP-4: {other}"
    );
}

/// `OP-4` delivered end-to-end by the REAL `EditFile`, in one call.
///
/// Closes the gap the OP-4 fix named in its own `unverified:` field and left open:
/// *"no test performs an actual `edit_file` write to a `~/.claude` path and asserts OP-4
/// arrives; what is asserted is that the pipeline produces a matching response and that the
/// real `EditFile` supplies the selector, which is two tests meeting at a value both check
/// rather than one call proving the whole."* The sibling test above is that pipeline half; this
/// is the one call.
///
/// **Two changes made this writable, and neither was available when the sibling was written.**
/// `a6b4fc35` gave write responses a path (`annotate_write_path`), and `30b6fc41` inverted
/// `Tool::selector_key`'s default — so `EditFile` supplies its own selector and `call_content`
/// runs the router itself. The sibling's explicit `route(Some("edit_file"), …)` is no longer
/// the only way to reach the router, which is what lets this assert on the delivered block
/// instead of on a value fed back in by hand.
///
/// **The negative control runs FIRST, and that order is load-bearing.** OP-4 is delivered once
/// per session (ledger key `op:OP-4`), so a non-matching call made *after* the matching one
/// would show no OP-4 block whatever the predicate did — a control that passes just as happily
/// against a match-everything predicate. Running it first is what makes its silence mean "the
/// predicate declined this path" rather than "the ledger was already spent".
///
/// **Both paths must be ABSOLUTE.** `annotate_write_path` keys by absoluteness, so a relative
/// `.claude/settings.json` is filed under `rel_path` as `.claude/settings.json` — which does
/// not contain the `/.claude` needle. Passing a relative path here would fail this test for a
/// reason that has nothing to do with routing.
///
/// **Mutation, and what it does and does not establish.** Making `annotate_write_path` insert
/// nothing kills this test — and also kills
/// `a_write_response_names_the_path_it_wrote_so_op_4_can_match` and the sibling above, so it
/// establishes that the annotation matters and NOT that this test is uniquely necessary
/// (`reconnaissance-patterns:R-164`). What this test uniquely adds is coverage of the real
/// `EditFile` body plus delivery in one call — a named gap closed, not a mutation nothing else
/// catches.
///
/// **A mutation that does NOT kill it, recorded because I predicted it would.** Forcing the key
/// to `rel_path` regardless of absoluteness leaves this test green: `names_path_containing`
/// scans `abs_path` AND `rel_path`, so an absolute path misfiled under the relative key still
/// matches the needle. Only `a_write_response_names_the_path_it_wrote_so_op_4_can_match` dies,
/// which is correct — the key choice is load-bearing for honesty, not for matching, exactly as
/// `annotate_write_path`'s own doc says ("a lie the matcher happens not to notice"). My first
/// draft of this comment named that swap as a kill-mutation; it was measured and it is not one.
#[tokio::test]
async fn a_real_edit_file_write_under_dot_claude_delivers_op_4() {
    let tmp = tempfile::tempdir().unwrap();
    // `rooted_ctx`, deliberately not `worktree_repo_ctx`: that helper seeds a linked
    // worktree, which is what its own callers are testing, and a write into an
    // ambiguous checkout is refused before it can reach the router.
    let root = tmp.path().join("main");
    std::fs::create_dir_all(&root).unwrap();
    let ctx = rooted_ctx(&root).await;

    // Negative control, FIRST — see the doc comment on why the order is load-bearing.
    // Not a `.md` file: IL-5 routes markdown to `edit_markdown`, and the refusal would
    // fail this control for a reason unrelated to the predicate under test.
    let outside = root.join("notes.toml");
    std::fs::write(&outside, "alpha = 1\n").unwrap();
    let control = crate::tools::edit_file::EditFile
        .call_content(
            serde_json::json!({
                "path": outside.display().to_string(),
                "old_string": "alpha = 1",
                "new_string": "alpha = 2",
            }),
            &ctx,
        )
        .await
        .expect("the control write must SUCCEED, or its silence about OP-4 proves nothing");
    let control_text = joined_text(&control);
    assert!(
        !control_text.contains("operator-rule OP-4"),
        "a write outside ~/.claude must not deliver OP-4 — without this the assertion below \
         is equally satisfied by a predicate matching every path: {control_text}"
    );

    // The matching write.
    let dot_claude = root.join(".claude");
    std::fs::create_dir_all(&dot_claude).unwrap();
    let target = dot_claude.join("settings.json");
    std::fs::write(&target, "{\"model\": \"opus\"}\n").unwrap();
    let hit = crate::tools::edit_file::EditFile
        .call_content(
            serde_json::json!({
                "path": target.display().to_string(),
                "old_string": "opus",
                "new_string": "sonnet",
            }),
            &ctx,
        )
        .await
        .expect("the write must succeed — call_content's `?` would skip the router entirely");
    let text = joined_text(&hit);
    assert!(
        text.contains("operator-rule OP-4"),
        "a real edit_file write under .claude must deliver OP-4 through call_content in ONE \
         call — this is the composition the two sibling tests could not establish. Got: {text}"
    );
    // The marker is not the rule: a mutation emitting the comment with an empty or wrong body
    // would otherwise ship green. Same reasoning as
    // `a_triggered_operator_rule_is_delivered_once_per_session`.
    assert!(
        text.contains("all three profiles"),
        "the OP-4 block arrived without its imperative — marker present, rule text absent: \
         {text}"
    );
}

/// The no-echo write convention, guarded. Single-checkout repos — every test
/// tempdir, and the overwhelmingly common real case — must be byte-identical to
/// before this change. This is the test that makes the conditional shape change
/// safe to ship.
#[tokio::test]
async fn a_write_without_worktrees_keeps_the_bare_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("main");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let ctx = rooted_ctx(&root).await;

    let val = write_echo(&ctx, "edit_code", serde_json::json!("ok")).await;

    assert_eq!(
        val,
        serde_json::json!("ok"),
        "no worktrees means no ambiguity: the response shape must not change"
    );
}

/// A pinned call already named its target in the call. Annotating it would be
/// noise on exactly the calls that were never at risk.
#[tokio::test]
async fn a_pinned_write_is_not_annotated() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, mut ctx) = worktree_repo_ctx(&tmp).await;
    ctx.workspace_override = Some(root);

    let val = write_echo(&ctx, "edit_code", serde_json::json!("ok")).await;

    assert_eq!(
        val,
        serde_json::json!("ok"),
        "a pinned write is unambiguous by construction; got {val}"
    );
}

/// The annotation must not cost the tool its own answer.
#[tokio::test]
async fn annotating_an_object_result_preserves_its_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let (_root, ctx) = worktree_repo_ctx(&tmp).await;

    let val = write_echo(
        &ctx,
        "edit_code",
        serde_json::json!({"status": "ok", "inserted_at_line": 4}),
    )
    .await;

    assert_eq!(
        val.get("inserted_at_line").and_then(|v| v.as_u64()),
        Some(4),
        "the tool's own fields must survive annotation; got {val}"
    );
    assert!(
        val.get("wrote_to").is_some(),
        "object results must still be annotated; got {val}"
    );
}

/// `approve_write` grants a write scope; it writes no file. Naming a checkout
/// there would describe something that did not happen.
#[tokio::test]
async fn an_exempt_tool_is_not_annotated() {
    let tmp = tempfile::tempdir().unwrap();
    let (_root, ctx) = worktree_repo_ctx(&tmp).await;

    let val = write_echo(&ctx, "approve_write", serde_json::json!("ok")).await;

    assert_eq!(
        val,
        serde_json::json!("ok"),
        "a tool that writes no project-relative path must not claim a checkout; got {val}"
    );
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
