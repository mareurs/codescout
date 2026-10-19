// Included into `server::tests` (see the `include!` beside `result_text`): these drive the
// REAL dispatch (`call_tool_inner`) and use that module's private `make_server`, `dispatch_tool`
// and `result_text`.
//
// What they guard: a `workspace=` pin that does not name an existing directory is REFUSED, and
// the refusal creates NOTHING on disk. Before 2026-10-18 the pin was kept as a raw (possibly
// relative) root and the first touch built a project there, leaving `<value>/.codescout/` behind.
// docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
//
// Isolation: no test here changes the process cwd in the shared test process. The ones that must
// (a RELATIVE pin resolves against the cwd) run in a child process (`platform::test_child`), the
// documented pattern in docs/conventions/test-env-isolation.md, with the cwd set to a tempdir in
// the CHILD only. A relative pin therefore cannot create `x/` in the repo even if the guard
// is removed.

/// Every path under `root`, relative to it, sorted. "Created nothing" is a statement about the
/// whole tree, so the tests compare listings rather than probing one expected path.
fn tree_listing(root: &std::path::Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if entry.file_type().unwrap().is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// Dispatch without unwrapping the protocol layer: a refused pin must come back as an `Ok`
/// result (a recoverable answer that leaves sibling calls alive), never as a protocol error.
async fn dispatch_pinned(
    server: &CodeScoutServer,
    name: &str,
    mut args: serde_json::Value,
    pin: &str,
) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
    args["workspace"] = serde_json::json!(pin);
    let req = CallToolRequestParams::new(name.to_string())
        .with_arguments(serde_json::from_value(args).unwrap());
    server
        .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
        .await
}

/// The refusal's JSON body: `RecoverableError` is routed to `isError: false` with `ok: false`.
fn assert_pin_refused(label: &str, result: &CallToolResult, pin_text: &str) {
    let text = result_text(result);
    assert_ne!(
        result.is_error,
        Some(true),
        "{label}: a refused pin is recoverable (sibling calls must survive); got: {text}"
    );
    let body: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("{label}: the refusal must be a JSON body; got: {text}"));
    assert_eq!(body["ok"], false, "{label}: must be refused; got: {text}");
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains(pin_text),
        "{label}: the refusal must name the pin {pin_text:?}; got: {text}"
    );
    assert!(
        error.contains("not an existing directory"),
        "{label}: the refusal must say what is wrong with it; got: {text}"
    );
    // Which ARGUMENT is refused: a pin here, not a project root.
    assert!(
        error.starts_with("workspace pin "),
        "{label}: the refusal must name the argument as the workspace pin; got: {text}"
    );
    assert!(
        body["hint"]
            .as_str()
            .is_some_and(|h| h.contains("absolute")),
        "{label}: the refusal must say what to do (pass an absolute path); got: {text}"
    );
}

/// The call that was actually made on 2026-10-18 (a write, `edit_file`), a read (`memory
/// list`, the issue's reproduction), and the other write shapes, each pinned to an ABSOLUTE
/// path that does not exist. A read and a write take different routes before the tool body (the
/// write takes the project's write lock first), so each is its own row.
#[tokio::test]
async fn a_pin_naming_a_missing_absolute_path_is_refused_and_creates_nothing() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");
    let pin = missing.to_string_lossy().to_string();

    let calls = [
        (
            "memory list (read)",
            "memory",
            serde_json::json!({"action": "list"}),
        ),
        (
            "memory write",
            "memory",
            serde_json::json!({"action": "write", "topic": "t", "content": "c"}),
        ),
        (
            "read_file (read)",
            "read_file",
            serde_json::json!({"path": "a.txt"}),
        ),
        (
            "edit_file (the 2026-10-18 call)",
            "edit_file",
            serde_json::json!({"path": "a.txt", "old_string": "a", "new_string": "b"}),
        ),
        (
            "create_file",
            "create_file",
            serde_json::json!({"path": "n.txt", "content": "c"}),
        ),
    ];
    for (label, tool, args) in calls {
        let result = dispatch_pinned(&server, tool, args, &pin)
            .await
            .unwrap_or_else(|e| {
                panic!("{label}: a refused pin must be an Ok result, not a protocol error: {e:?}")
            });
        assert_pin_refused(label, &result, &pin);
    }
    assert!(
        !missing.exists(),
        "the refused pin must not have been created"
    );
    assert_eq!(
        tree_listing(&host_root),
        Vec::<String>::new(),
        "nothing may be created anywhere under the directory the pin pointed into"
    );
}

/// A pin that names an existing FILE is not a project root either. Guards `is_dir` against
/// `exists`: a file exists, so only the directory test refuses it. Before the fix this was not
/// refused as a pin at all; it failed deep inside residency with an OS error.
#[tokio::test]
async fn a_pin_naming_a_file_is_refused_and_leaves_it_untouched() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let file = host_root.join("not-a-dir.txt");
    std::fs::write(&file, "keep").unwrap();
    let pin = file.to_string_lossy().to_string();

    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &pin,
    )
    .await
    .expect("a refused pin must be an Ok result");
    assert_pin_refused("memory list", &result, &pin);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
    assert_eq!(tree_listing(&host_root), vec!["not-a-dir.txt".to_string()]);
}

/// The reproduction in the issue, as the filing agent ran it: a RELATIVE placeholder pin,
/// `workspace="x"`. Runs in a child process whose cwd is a tempdir, so the pre-fix behaviour
/// (a project built at `<cwd>/x`) lands in that tempdir and never in the repo.
#[cfg(unix)]
#[test]
fn a_relative_pin_naming_a_missing_path_is_refused_and_creates_nothing() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_naming_a_missing_path_is_refused_and_creates_nothing",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            // Child process only: nothing else runs here, so nothing else reads the cwd.
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    for (label, tool, args) in [
                        (
                            "memory list",
                            "memory",
                            serde_json::json!({"action": "list"}),
                        ),
                        (
                            "edit_file",
                            "edit_file",
                            serde_json::json!({"path": "x", "old_string": "a", "new_string": "b"}),
                        ),
                    ] {
                        let result = dispatch_pinned(&server, tool, args, "x")
                            .await
                            .unwrap_or_else(|e| panic!("{label}: protocol error: {e:?}"));
                        assert_pin_refused(label, &result, "x");
                        assert!(
                            result_text(&result).contains("working directory"),
                            "{label}: a relative pin resolves against the server's cwd, and the \
                             refusal must say so; got: {}",
                            result_text(&result)
                        );
                    }
                    assert_eq!(
                        tree_listing(&cwd_root),
                        Vec::<String>::new(),
                        "the refused relative pin must create nothing in the cwd"
                    );
                });
        },
    );
}

/// The behaviour that must NOT change: a relative pin that DOES exist still resolves against
/// the server's cwd (as `workspace(activate)` and `--project` do) and still makes that
/// directory a project. Same child-process isolation as above.
#[cfg(unix)]
#[test]
fn a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_naming_an_existing_directory_still_resolves_against_the_cwd",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::fs::create_dir(cwd_root.join("proj")).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    let result = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "list"}),
                        "proj",
                    )
                    .await
                    .expect("an existing relative pin must be served");
                    // The SUCCESS body, not the absence of one refusal's wording: a refusal of
                    // any other kind is a RecoverableError too (`is_error: false`), and
                    // residency builds `proj/.codescout` before a later step can fail.
                    // Measured: with the root left un-canonicalized the call answered
                    // `{"ok": false, "error": "workspace pin .../proj is not resident"}` and
                    // every assertion that only looked for "not an existing directory" passed.
                    assert_eq!(result_text(&result), "0 topics");
                    // And the pin really targets `proj`: a write through it lands there.
                    let written = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "write", "topic": "t", "content": "c"}),
                        "proj",
                    )
                    .await
                    .expect("an existing relative pin must be writable");
                    assert!(
                        !result_text(&written).contains("\"ok\": false"),
                        "got: {}",
                        result_text(&written)
                    );
                    assert!(
                        cwd_root
                            .join("proj")
                            .join(".codescout/memories/t.md")
                            .is_file(),
                        "the pin resolved against the cwd and made THAT directory the project"
                    );
                    assert_eq!(
                        tree_listing(&cwd_root)
                            .into_iter()
                            .filter(|p| p != "proj" && !p.starts_with("proj/"))
                            .collect::<Vec<_>>(),
                        Vec::<String>::new(),
                        "nothing may be created beside the pinned directory"
                    );
                });
        },
    );
}

/// An empty or whitespace-only `workspace` is "no pin" (the existing contract), NOT a pin
/// naming a missing path: it must keep answering from the session's project and create nothing.
#[cfg(unix)]
#[test]
fn a_blank_pin_is_still_no_pin_and_creates_nothing() {
    crate::platform::test_child::run(
        module_path!(),
        "a_blank_pin_is_still_no_pin_and_creates_nothing",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    for blank in ["", "   "] {
                        let result = dispatch_pinned(
                            &server,
                            "memory",
                            serde_json::json!({"action": "list"}),
                            blank,
                        )
                        .await
                        .expect("a blank pin is no pin");
                        // The default project's own answer, not merely "no refusal text".
                        assert_eq!(
                            result_text(&result),
                            "0 topics",
                            "{blank:?} is no pin and must be answered by the default project"
                        );
                    }
                    assert_eq!(tree_listing(&cwd_root), Vec::<String>::new());
                });
        },
    );
}

/// A relative pin is resolved against the server's cwd, and the refusal says where that is. If
/// the cwd itself is unreadable (here: deleted under the process, which Linux allows) it must
/// still be refused, and say that the path is relative without inventing a directory. Guards
/// the `current_dir()` failure arm of the message, which no other test can reach.
#[cfg(unix)]
#[test]
fn a_relative_pin_is_refused_even_when_the_cwd_is_gone() {
    crate::platform::test_child::run(
        module_path!(),
        "a_relative_pin_is_refused_even_when_the_cwd_is_gone",
        &[],
        || {
            let cwd = tempdir().unwrap();
            let cwd_root = std::fs::canonicalize(cwd.path()).unwrap();
            std::env::set_current_dir(&cwd_root).unwrap();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    std::fs::remove_dir(&cwd_root).unwrap();
                    assert!(std::env::current_dir().is_err(), "premise: the cwd is gone");
                    let result = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "list"}),
                        "x",
                    )
                    .await
                    .expect("a refused pin must be an Ok result");
                    assert_pin_refused("memory list", &result, "x");
                    let body: serde_json::Value =
                        serde_json::from_str(&result_text(&result)).unwrap();
                    let error = body["error"].as_str().unwrap();
                    assert!(
                        error.ends_with("working directory)"),
                        "with no readable cwd the note must end at the words, not name a path; \
                         got: {error}"
                    );
                });
        },
    );
}

/// The write lock is taken on the PINNED project, not the session default. `call_tool_inner`
/// resolves the pin once and hands it to the lock acquisition; this holds the DEFAULT project's
/// write mutex and requires a pinned write to another project to complete anyway. Handing the
/// lock acquisition no pin (or re-deriving it wrongly) makes the pinned write queue behind the
/// default's lock, and the call never returns.
#[tokio::test]
async fn a_pinned_write_takes_the_pinned_projects_write_lock_not_the_defaults() {
    let (_home, server) = make_server().await;
    let pinned = tempdir().unwrap();
    std::fs::create_dir_all(pinned.path().join(".codescout")).unwrap();
    let pin = std::fs::canonicalize(pinned.path())
        .unwrap()
        .to_string_lossy()
        .to_string();

    let default_lock = server
        .agent
        .with_project(|p| Ok(p.write_lock.clone()))
        .await
        .unwrap();
    let _default_held = default_lock.lock_owned().await;

    let call = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "write", "topic": "lock-target", "content": "x"}),
        &pin,
    );
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), call)
        .await
        .expect("a pinned write must not queue behind the DEFAULT project's write lock")
        .expect("dispatch");
    assert_ne!(result.is_error, Some(true), "got: {}", result_text(&result));
    assert!(
        pinned
            .path()
            .join(".codescout/memories/lock-target.md")
            .exists(),
        "the write landed in the pinned project"
    );
}

// ---------------------------------------------------------------------------------------------
// Review round 2026-10-18 (F2 to F9). Each of these guards a branch the first round left open.
// ---------------------------------------------------------------------------------------------

/// The refusal body of a RECOVERABLE answer: `Ok`, not `is_error`, `ok: false`.
fn recoverable_body(label: &str, result: &CallToolResult) -> serde_json::Value {
    let text = result_text(result);
    assert_ne!(
        result.is_error,
        Some(true),
        "{label}: must be recoverable; got: {text}"
    );
    let body: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("{label}: must be a JSON refusal body; got: {text}"));
    assert_eq!(body["ok"], false, "{label}: must be refused; got: {text}");
    body
}

const WRITE_TOPIC: &str = r#"{"action": "write", "topic": "t", "content": "c"}"#;

/// F9: a `workspace` value that is not a string was silently "no pin", so `workspace: 5` was
/// answered by the DEFAULT project. Absent and `null` stay "no pin".
#[tokio::test]
async fn a_workspace_value_that_is_not_a_string_is_refused() {
    let (_home, server) = make_server().await;
    for (label, kind, value) in [
        ("number", "a number", serde_json::json!(5)),
        ("bool", "a boolean", serde_json::json!(true)),
        ("array", "an array", serde_json::json!(["x"])),
        ("object", "an object", serde_json::json!({"path": "x"})),
    ] {
        let mut args = serde_json::json!({"action": "list"});
        args["workspace"] = value;
        let result = dispatch_tool(&server, "memory", args).await;
        let body = recoverable_body(label, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains("workspace") && error.contains("string") && error.contains(kind),
            "{label}: must say `workspace` has to be a string and what it was; got: {error}"
        );
        assert!(
            body["hint"]
                .as_str()
                .is_some_and(|h| h.contains("absolute")),
            "{label}: must say what to do; got: {body}"
        );
    }
    let mut null_pin = serde_json::json!({"action": "list"});
    null_pin["workspace"] = serde_json::Value::Null;
    assert_eq!(
        result_text(&dispatch_tool(&server, "memory", null_pin).await),
        "0 topics",
        "null is no pin"
    );
    assert_eq!(
        result_text(&dispatch_tool(&server, "memory", serde_json::json!({"action": "list"})).await),
        "0 topics",
        "absent is no pin"
    );
}

/// F3 (M4): the pin refusal is the answer, not the write gate's. A default project that is
/// read-only refuses every write; a write pinned at a missing path must still be told about
/// the PIN, because the write gate's text would send the caller to the wrong remedy.
#[tokio::test]
async fn a_bad_pin_is_refused_before_the_write_gate_speaks() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let armed = dispatch_tool(
        &server,
        "workspace",
        serde_json::json!({
            "action": "activate", "path": home_root.to_string_lossy(), "read_only": true
        }),
    )
    .await;
    assert_ne!(
        armed.is_error,
        Some(true),
        "precondition: {}",
        result_text(&armed)
    );
    let control = dispatch_tool(
        &server,
        "memory",
        serde_json::from_str(WRITE_TOPIC).unwrap(),
    )
    .await;
    assert_eq!(
        control.is_error,
        Some(true),
        "control: an unpinned write is refused by the read-only gate; got: {}",
        result_text(&control)
    );

    let host = tempdir().unwrap();
    let missing = std::fs::canonicalize(host.path())
        .unwrap()
        .join("does-not-exist");
    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::from_str(WRITE_TOPIC).unwrap(),
        &missing.to_string_lossy(),
    )
    .await
    .expect("a refused pin is an Ok result");
    assert_pin_refused(
        "pinned write under read-only",
        &result,
        &missing.to_string_lossy(),
    );
    assert!(
        !result_text(&result).contains("disabled"),
        "the write gate must not have spoken; got: {}",
        result_text(&result)
    );
}

/// F5: a refused pin leaves a usage record, against the session's DEFAULT project, and builds
/// nothing at the pinned path. The incident that found this bug was diagnosed from exactly
/// such a row; before this, a refused call vanished without one.
#[tokio::test]
async fn a_refused_pin_is_recorded_against_the_default_project_and_nowhere_else() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &missing.to_string_lossy(),
    )
    .await
    .expect("a refused pin is an Ok result");
    assert_pin_refused("memory list", &result, &missing.to_string_lossy());

    wait_for_usage_rows(&home_root, 1).await;
    let conn = crate::usage::db::open_db(&home_root).unwrap();
    let rows: Vec<(String, String, String)> = conn
        .prepare("SELECT tool_name, outcome, project_root FROM tool_calls")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        vec![(
            "memory".to_string(),
            "recoverable_error".to_string(),
            home_root.to_string_lossy().to_string()
        )],
        "the refused call is one recoverable row in the DEFAULT project's usage.db"
    );
    assert_eq!(
        tree_listing(&host_root),
        Vec::<String>::new(),
        "recording must not build anything at the pinned path"
    );
}

/// F2 (route 1): a pin accepted at dispatch can stop being usable before the write lock is
/// taken (the directory vanishes in between). That is a recoverable refusal, not a protocol
/// error, and it must not re-create the directory. Driven at the lock acquisition itself,
/// because the window is too narrow to hit through the full dispatch.
#[tokio::test]
async fn a_pin_that_vanishes_before_the_write_lock_is_a_recoverable_refusal() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let gone = host_root.join("vanished");

    let refusal = match server
        .acquire_write_guard_if_writing(
            "memory",
            &serde_json::from_str(WRITE_TOPIC).unwrap(),
            Some(&gone),
        )
        .await
    {
        Err(e) => panic!("must not be a protocol error: {e:?}"),
        Ok(Err(refusal)) => refusal,
        Ok(Ok(_)) => panic!("a missing pin must not yield a write guard"),
    };
    let body = recoverable_body("write gate", &refusal);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains(&gone.to_string_lossy().to_string())),
        "must name the pin; got: {body}"
    );
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
}

/// F2 (route 3), the case the existing project.toml hint is FOR: an unparseable config. The
/// write was a protocol error here (`write gate: parsing .../project.toml`); both calls must
/// be recoverable and both must point at project.toml.
#[tokio::test]
async fn a_pin_to_an_unparseable_project_toml_is_recoverable_and_names_project_toml() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let broken = std::fs::canonicalize(host.path()).unwrap().join("broken");
    std::fs::create_dir_all(broken.join(".codescout")).unwrap();
    std::fs::write(
        broken.join(".codescout/project.toml"),
        "this is = = not toml [[",
    )
    .unwrap();
    let pin = broken.to_string_lossy().to_string();

    for (label, args) in [
        (
            "memory write",
            serde_json::from_str::<serde_json::Value>(WRITE_TOPIC).unwrap(),
        ),
        ("memory list", serde_json::json!({"action": "list"})),
    ] {
        let result = dispatch_pinned(&server, "memory", args, &pin)
            .await
            .unwrap_or_else(|e| panic!("{label}: must not be a protocol error: {e:?}"));
        let body = recoverable_body(label, &result);
        assert!(
            body["error"].as_str().is_some_and(|e| e.contains(&pin)),
            "{label}: must name the pin; got: {body}"
        );
        assert!(
            body["hint"]
                .as_str()
                .is_some_and(|h| h.contains("project.toml")),
            "{label}: an unparseable config must point at project.toml; got: {body}"
        );
    }
}

/// F5, attribution: the refused call's row names the same caller an ordinary call of that
/// caller would. A stamped principal (`<session>/<agent>`) must reach `cc_session_id` and
/// `agent_id`, because that is how a subagent's refused call is told from its parent's.
#[tokio::test]
async fn a_refused_pin_is_attributed_to_its_caller_like_an_ordinary_call() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let host = tempdir().unwrap();
    let missing = std::fs::canonicalize(host.path())
        .unwrap()
        .join("does-not-exist");
    let pin = missing.to_string_lossy().to_string();
    let stamped = || {
        let mut args = serde_json::json!({"action": "list"});
        args[crate::tools::session_key::PRINCIPAL_ARG_KEY] = serde_json::json!("conv-A/testagent");
        args
    };

    dispatch_tool(&server, "memory", serde_json::json!({"action": "list"})).await;
    dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &pin,
    )
    .await
    .unwrap();
    dispatch_tool(&server, "memory", stamped()).await;
    dispatch_pinned(&server, "memory", stamped(), &pin)
        .await
        .unwrap();

    wait_for_usage_rows(&home_root, 4).await;
    let conn = crate::usage::db::open_db(&home_root).unwrap();
    let rows: Vec<(String, Option<String>, Option<String>)> = conn
        .prepare("SELECT outcome, cc_session_id, agent_id FROM tool_calls ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 4, "one row per call; got {rows:?}");
    assert_eq!(rows[1].0, "recoverable_error");
    assert_eq!(rows[3].0, "recoverable_error");
    assert_eq!(
        (&rows[1].1, &rows[1].2),
        (&rows[0].1, &rows[0].2),
        "unstamped: the refused call is attributed like the ordinary one; got {rows:?}"
    );
    assert_eq!(
        (&rows[3].1, &rows[3].2),
        (&rows[2].1, &rows[2].2),
        "stamped: the refused call is attributed like the ordinary one; got {rows:?}"
    );
    assert_eq!(
        rows[3].1.as_deref(),
        Some("conv-A/testagent"),
        "got {rows:?}"
    );
    assert_eq!(rows[3].2.as_deref(), Some("testagent"), "got {rows:?}");
}

/// F2, the branch that did NOT change: with no pin, a write gate that cannot resolve the
/// session's own default project is still the protocol error it always was. Only a pin is the
/// caller's input; the default project is nobody's choice, so a failure there is the server's.
#[tokio::test]
async fn an_unpinned_write_with_no_project_keeps_its_protocol_error() {
    let (_dir, server) = test_support::make_server_no_project().await;
    let req = CallToolRequestParams::new("memory".to_string())
        .with_arguments(serde_json::from_str(WRITE_TOPIC).unwrap());
    let err = server
        .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
        .await
        .expect_err("the unpinned write gate must keep failing as a protocol error");
    assert!(
        err.message.starts_with("write gate: "),
        "got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------------------------
// Second review round (H1, M1, L1 to L3). Refusals must not leak, must name the pin, and must
// cost the caller nothing.
// ---------------------------------------------------------------------------------------------

/// A server built with `debug = true`, which stores every call's input and output in the usage
/// row, so a leak into the telemetry is visible to a test. Mirrors `make_server`.
async fn make_debug_server() -> (tempfile::TempDir, CodeScoutServer) {
    let dir = tempdir().unwrap();
    let codescout_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    let ws_path = codescout_dir.join("librarian-workspace.toml");
    std::fs::write(&ws_path, "").unwrap();
    #[cfg(feature = "librarian")]
    let env = ServerEnv {
        librarian: crate::librarian::LibrarianEnv {
            workspace: Some(ws_path),
            db: Some(codescout_dir.join("librarian.db")),
            ..Default::default()
        },
        ..test_support::test_env(dir.path())
    };
    #[cfg(not(feature = "librarian"))]
    let env = test_support::test_env(dir.path());
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let server =
        CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), true, env).await;
    (dir, server)
}

/// Every text block of a result, joined: a leak in ANY block counts.
fn all_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// H1: the refusal for a pin whose `project.toml` cannot be loaded used to print the whole
/// error chain, and the TOML error QUOTES the offending source line. A project.toml that is a
/// link to a secret file therefore returned that file's text. Neither a syntax error nor a type
/// error (which quotes the VALUE) may reach any response or any stored usage row, on any tool.
#[cfg(unix)]
#[tokio::test]
async fn a_config_failure_never_quotes_the_file_in_a_response_or_a_usage_row() {
    const MARKER: &str = "SECRET_MARKER_QX7";
    for (label, contents) in [
        ("syntax", format!("{MARKER}_parse hunter2")),
        ("type", format!("project = \"{MARKER}_type\"\n")),
    ] {
        let (home, server) = make_debug_server().await;
        let home_root = std::fs::canonicalize(home.path()).unwrap();
        // The database exists before any refusal's recorder can race to create it.
        crate::usage::db::open_db(&home_root).unwrap();
        let host = tempdir().unwrap();
        let host_root = std::fs::canonicalize(host.path()).unwrap();
        let secret = host_root.join("secret.txt");
        std::fs::write(&secret, contents).unwrap();
        let proj = host_root.join("proj");
        std::fs::create_dir_all(proj.join(".codescout")).unwrap();
        std::os::unix::fs::symlink(&secret, proj.join(".codescout/project.toml")).unwrap();
        let pin = proj.to_string_lossy().to_string();

        for (tool, args) in [
            ("memory", serde_json::json!({"action": "list"})),
            ("memory", serde_json::from_str(WRITE_TOPIC).unwrap()),
            (
                "create_file",
                serde_json::json!({"path": "n.txt", "content": "c"}),
            ),
            ("read_file", serde_json::json!({"path": "a.txt"})),
            ("tree", serde_json::json!({})),
        ] {
            let result = dispatch_pinned(&server, tool, args, &pin)
                .await
                .unwrap_or_else(|e| panic!("{label} {tool}: protocol error {e:?}"));
            let text = all_text(&result);
            assert!(
                !text.contains(MARKER),
                "{label} {tool}: the file's text reached the response: {text}"
            );
            let body = recoverable_body(&format!("{label} {tool}"), &result);
            assert!(
                body["error"]
                    .as_str()
                    .is_some_and(|e| e.contains("project.toml")),
                "{label} {tool}: must name the file that failed; got: {body}"
            );
        }

        wait_for_usage_rows(&home_root, 5).await;
        let conn = crate::usage::db::open_db(&home_root).unwrap();
        let (rows, stored): (i64, String) = conn
            .query_row(
                "SELECT count(*), coalesce(group_concat(coalesce(error_msg,'') || '|' || \
                 coalesce(input_json,'') || '|' || coalesce(output_json,'')), '') FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(rows > 0, "{label}: premise: the refusals were recorded");
        assert!(
            stored.contains(&pin),
            "{label}: premise: debug storage holds the call, so a leak could land there"
        );
        assert!(
            !stored.contains(MARKER),
            "{label}: the file's text reached the usage rows"
        );
    }
}

/// L1: a pin that dispatch accepts but that cannot be made resident (here: `.codescout` is a
/// regular file, which fails for root too) was refused only by the tools that happen to resolve
/// it, and the others answered with a wrong cause ("requires an active project") or a bare
/// error. It is now refused ONCE, at dispatch, before any tool body, for EVERY tool.
#[tokio::test]
async fn every_tool_refuses_an_unresidentable_pin_and_names_it() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let proj = std::fs::canonicalize(host.path()).unwrap().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(proj.join(".codescout"), "not a directory").unwrap();
    let pin = proj.to_string_lossy().to_string();

    let names: Vec<String> = server.tools.iter().map(|t| t.name().to_string()).collect();
    assert!(
        names.len() >= 10,
        "premise: the table covers the registry: {names:?}"
    );
    for name in names {
        let result = dispatch_pinned(&server, &name, serde_json::json!({}), &pin)
            .await
            .unwrap_or_else(|e| panic!("{name}: protocol error {e:?}"));
        let body = recoverable_body(&name, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains(&pin) && !error.contains("active project"),
            "{name}: must name the pin and the real cause; got: {body}"
        );
    }
}

/// L3: the pin is echoed into the response and into the stored usage row, so it is capped, and
/// control characters (which would forge lines or colour in a log) are escaped.
#[tokio::test]
async fn a_refused_pin_is_echoed_capped_and_escaped() {
    let (home, server) = make_server().await;
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    // Create the usage database up front: several refusals' recorders can overlap, and
    // two connections that both CREATE the schema race its migrations (`duplicate column name`).
    crate::usage::db::open_db(&home_root).unwrap();

    let long = format!("/nonexistent/{}", "a".repeat(200_000));
    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        &long,
    )
    .await
    .unwrap();
    let body = recoverable_body("long pin", &result);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.len() < 2_000,
        "the echo must be capped; got {} bytes",
        error.len()
    );
    assert!(error.contains("truncated"), "and say so; got: {error}");

    let rough = "/nonexistent/a\nb\u{1b}[31mc";
    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        rough,
    )
    .await
    .unwrap();
    let body = recoverable_body("control pin", &result);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        !error.contains('\n') && !error.contains('\u{1b}'),
        "control characters must not reach the message raw; got: {error:?}"
    );
    assert!(
        error.contains("\\u{a}") && error.contains("\\u{1b}"),
        "got: {error:?}"
    );

    wait_for_usage_rows(&home_root, 2).await;
    let conn = crate::usage::db::open_db(&home_root).unwrap();
    let longest: i64 = conn
        .query_row("SELECT max(length(error_msg)) FROM tool_calls", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        longest < 2_000,
        "the stored error is capped too; got {longest} chars"
    );
}

/// L3: telemetry never re-creates a project that is gone. The session's default root was
/// deleted; a refused pin must not bring `<root>/.codescout/usage.db` (and the root) back.
#[tokio::test]
async fn a_refused_pin_does_not_recreate_a_deleted_default_root() {
    let (home, server) = make_server().await;
    let root = std::fs::canonicalize(home.path()).unwrap();
    std::fs::remove_dir_all(&root).unwrap();

    let result = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        "/nonexistent/pin",
    )
    .await
    .unwrap();
    recoverable_body("refused", &result);
    assert!(
        !root.exists(),
        "recording must not re-create the deleted default root"
    );
}

/// L3: recording the refusal is best-effort and bounded. With `usage.db` exclusively locked, the
/// recorder waits out SQLite's 5 s busy timeout; the caller must not.
#[tokio::test]
async fn a_refused_pin_does_not_wait_for_a_locked_usage_db() {
    let (home, server) = make_server().await;
    let root = std::fs::canonicalize(home.path()).unwrap();
    let first = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        "/nonexistent/pin",
    )
    .await
    .unwrap();
    recoverable_body("first", &first);

    wait_for_usage_rows(&root, 1).await;
    let conn = crate::usage::db::open_db(&root).unwrap();
    conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let started = std::time::Instant::now();
    let second = dispatch_pinned(
        &server,
        "memory",
        serde_json::json!({"action": "list"}),
        "/nonexistent/pin",
    )
    .await
    .unwrap();
    let waited = started.elapsed();
    conn.execute_batch("ROLLBACK").unwrap();
    recoverable_body("second", &second);
    assert!(
        waited < std::time::Duration::from_secs(3),
        "the refusal waited {waited:?} on a locked usage.db"
    );
}

/// F2 (route 2) and F4, root-proof: a project that cannot be opened for a reason that is NOT
/// its configuration. `.codescout` is a regular file, so residency fails with `Not a
/// directory` for every user, root included (the earlier fixture, a mode-000 directory, bound
/// nobody who may read anything). Reads and writes alike must be recoverable and must not
/// blame project.toml.
#[tokio::test]
async fn a_pin_whose_project_cannot_be_opened_is_recoverable_and_does_not_blame_project_toml() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let proj = std::fs::canonicalize(host.path()).unwrap().join("proj");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(proj.join(".codescout"), "not a directory").unwrap();
    let pin = proj.to_string_lossy().to_string();

    for (label, args) in [
        ("memory list", serde_json::json!({"action": "list"})),
        (
            "memory write",
            serde_json::from_str::<serde_json::Value>(WRITE_TOPIC).unwrap(),
        ),
    ] {
        let result = dispatch_pinned(&server, "memory", args, &pin)
            .await
            .unwrap_or_else(|e| panic!("{label}: must not be a protocol error: {e:?}"));
        let body = recoverable_body(label, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains(&pin) && error.to_lowercase().contains("not a directory"),
            "{label}: must name the pin and the real cause; got: {body}"
        );
        assert!(
            !body.to_string().contains("project.toml"),
            "{label}: an unopenable directory is not a project.toml problem; got: {body}"
        );
    }
}

/// M1 end to end, through the REAL loader and the real dispatch: each way a project.toml can
/// fail to load comes back as one recoverable refusal naming that file, with the project.toml
/// hint, and nothing from inside the file. The cases are the loader's four failure kinds.
#[tokio::test]
async fn each_way_a_project_toml_fails_to_load_is_refused_naming_the_file() {
    let (_home, server) = make_server().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let oversize = format!("# SECRET_MARKER_QX7\n{}", "#".repeat(1024 * 1024 + 1));
    for (label, contents) in [
        ("syntax", "SECRET_MARKER_QX7 hunter2".to_string()),
        ("type", "project = \"SECRET_MARKER_QX7\"\n".to_string()),
        ("oversize", oversize),
        (
            "list limit",
            "[project]\nname = \"x\"\n[security]\nextra_write_roots = [\"SECRET_MARKER_QX7\"]\n"
                .to_string(),
        ),
    ] {
        let proj = host_root.join(label.replace(' ', "-"));
        std::fs::create_dir_all(proj.join(".codescout")).unwrap();
        let file = proj.join(".codescout/project.toml");
        std::fs::write(&file, contents).unwrap();
        let pin = proj.to_string_lossy().to_string();

        let result = dispatch_pinned(
            &server,
            "memory",
            serde_json::json!({"action": "list"}),
            &pin,
        )
        .await
        .unwrap();
        assert!(
            !all_text(&result).contains("SECRET_MARKER_QX7"),
            "{label}: quoted the file: {}",
            all_text(&result)
        );
        let body = recoverable_body(label, &result);
        let error = body["error"].as_str().unwrap_or_default();
        assert!(
            error.contains(&format!(
                "project config '{}' could not be loaded",
                file.display()
            )),
            "{label}: must name the file that failed; got: {body}"
        );
        assert!(
            body["hint"]
                .as_str()
                .is_some_and(|h| h.contains("project's .codescout/project.toml")),
            "{label}: must point at project.toml; got: {body}"
        );
    }
}

/// M1, the broken GLOBAL config: it fails every project, so it is named as the global file and
/// the hint must not send the caller to the project's own project.toml. The global directory is
/// reached through `XDG_CONFIG_HOME`, set for a CHILD process only (the shared suite never
/// mutates its environment). The file is broken AFTER the server is built, since a broken
/// global config stops the default project from loading at all.
#[cfg(unix)]
#[test]
fn a_broken_global_config_is_named_as_the_global_file_not_the_projects() {
    let xdg = tempdir().unwrap();
    let xdg_root = std::fs::canonicalize(xdg.path()).unwrap();
    crate::platform::test_child::run(
        module_path!(),
        "a_broken_global_config_is_named_as_the_global_file_not_the_projects",
        &[("XDG_CONFIG_HOME", Some(xdg_root.to_str().unwrap()))],
        || {
            let xdg_root = std::path::PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap());
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (_home, server) = make_server().await;
                    let global = xdg_root.join("codescout");
                    std::fs::create_dir_all(&global).unwrap();
                    std::fs::write(global.join("config.toml"), "SECRET_MARKER_QX7 hunter2")
                        .unwrap();
                    let host = tempdir().unwrap();
                    let proj = std::fs::canonicalize(host.path()).unwrap().join("proj");
                    std::fs::create_dir_all(proj.join(".codescout")).unwrap();
                    std::fs::write(
                        proj.join(".codescout/project.toml"),
                        "[project]\nname = \"fine\"\n",
                    )
                    .unwrap();

                    let result = dispatch_pinned(
                        &server,
                        "memory",
                        serde_json::json!({"action": "list"}),
                        &proj.to_string_lossy(),
                    )
                    .await
                    .unwrap();
                    assert!(!all_text(&result).contains("SECRET_MARKER_QX7"));
                    let body = recoverable_body("global", &result);
                    let error = body["error"].as_str().unwrap_or_default();
                    assert!(
                        error.contains(&format!(
                            "global config '{}' could not be loaded",
                            global.join("config.toml").display()
                        )),
                        "must name the global file; got: {body}"
                    );
                    let hint = body["hint"].as_str().unwrap_or_default();
                    assert!(
                        hint.contains("global config") && !hint.contains("project.toml"),
                        "must not send the caller to the project's file; got: {body}"
                    );
                });
        },
    );
}

/// Waits (up to 10 s) until the default project's usage database holds `at_least` rows. A
/// refused pin's row is written under a short budget on the blocking pool, so on a loaded
/// machine it may land just after the answer: the answer is what is bounded, not the row.
async fn wait_for_usage_rows(root: &std::path::Path, at_least: i64) {
    // Read-only and without `open_db`: that runs the schema migrations, and a migration racing
    // the recorder's own open fails with `duplicate column name`.
    let path = root.join(".codescout/usage.db");
    for _ in 0..200 {
        let count = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .and_then(|c| {
            c.query_row("SELECT count(*) FROM tool_calls", [], |r| {
                r.get::<_, i64>(0)
            })
        })
        .unwrap_or(0);
        if count >= at_least {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the usage database never reached {at_least} rows");
}
